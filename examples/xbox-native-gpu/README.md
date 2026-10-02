# GPUI Kit Xbox prototype

This example ports GPUI Kit itself into a native Xbox UWP host. The interface is
rendered by GPUI's WGPU renderer using DirectX 12 and a real CoreWindow surface.
The existing native Direct3D 11 matrix workload from
[`richrobber2/xbox-native-gpu`](https://github.com/richrobber2/xbox-native-gpu)
remains responsible for compute and numerical verification.

This experimental platform backend was launched on Xbox Series X on 2026-10-02.
The actual GPUI interface rendered through DirectX 12, and the native D3D11
256×256 workload verified all 65,536 outputs. Physical controller input and
suspension/resume still need separate verification.

## Build

Run **Actions → Build GPUI Kit Xbox prototype** in this repository, or use a
Windows machine with Visual Studio's UWP C++ v142 workload and Windows SDK
22621 or newer:

```powershell
rustup toolchain install nightly-2026-09-30 --profile minimal --component rust-src
./examples/xbox-native-gpu/scripts/build.ps1
```

Rust targets `x86_64-uwp-windows-msvc`. Because Rust does not distribute that
target's standard library, the build uses nightly `build-std`. It does not link
the GPUI desktop Win32 platform. The C++ app links the Rust static library,
compiles the existing HLSL compute shader, packages the UWP application and
verifies its development signature. The artifact is `gpui-kit-xbox-x64`.

The signing certificate is generated for each build. Only the public `.cer` is
included in artifacts; the private key stays on the build machine and is removed
at the end. Packaging is for Developer Mode, not a retail Xbox or Store release.

## Port boundaries

- `ui/src/platform.rs`: single-window GPUI platform, text shaping, rendering,
  lifecycle, input and resize callbacks.
- `ui/src/dispatcher.rs`: host-driven foreground executor and two background
  workers. UWP owns the event loop.
- `ui/src/lib.rs`: GPUI Kit theme, real Button components, controller selection,
  retained result state and guarded C ABI. Panics cannot unwind across C++.
- `src/App.cpp`: CoreWindow, gamepad polling, OS font loading, native GPU work
  and LocalState report storage. Direct2D no longer draws the interface.
- `patches/`: pinned GPUI WGPU and WGPU HAL changes for DirectX 12 selection and
  `CreateSwapChainForCoreWindow`. The HAL owns a COM reference and never treats
  CoreWindow as a desktop HWND.

Preparation downloads the pinned crate releases, checks/applies these patches
and decodes the checked-in package artwork. Generated sources, output packages
and private signing material are ignored. The Rust lockfile is checked in.

Clipboard, native file dialogs, external URL launching, desktop menus and text
editing/IME are not implemented. This host currently supports controller and
keyboard commands for the GPU lab. It is not a general Xbox GPUI distribution.

## Validation

The local Rust check uses the Android target to validate the platform trait and
application code without requiring a Windows SDK. It does not execute the
CoreWindow/DirectX 12 path. The Windows workflow builds the real UWP target,
runs the independent matrix reference tests and checks the package signature.
The device report is [validation/xbox-series-x-2026-10-02.json](validation/xbox-series-x-2026-10-02.json).
See [INSTALL.md](INSTALL.md) for the verified loose-folder deployment command.
The Xbox also compiled and ran a standalone Rust controller-selection smoke
check, and compiled this example’s actual HLSL shader locally. These checks do
not establish a full GPUI Cargo/UWP package build on the console.
