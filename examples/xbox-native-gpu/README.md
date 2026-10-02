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

## Downloaded model studio

Models is the default tool. The persistent navigation rail opens Models, IDE,
GPU and Controls; LB/RB and Ctrl 1–4 switch tools.
The studio selects the existing Anima Preview 3, MiaoMiao Anima 1.6 and
MiaoMiao Anima 2.9B Beta 1.1 checkpoints in `D:\DevelopmentFiles\AnimaModels`.
The private native engine and shared encoder/VAE/tokenizers must be staged in
`D:\DevelopmentFiles\AnimaModels\GpuiRuntime`, alongside the engine’s existing
`VCRUNTIME140.dll` and `vcruntime140_1.dll` dependencies. The compiler bin directory
also needs its runtime dependencies. Neither engines nor weights
are shipped in this repository or downloaded automatically.

Move focus to a checkpoint, prompt, sampling-steps button or Generate/Cancel,
then activate it with Enter/A. Default generation is 256×256, seed 42 and CFG 4.
Model changes are blocked during generation. Switching tools leaves an active
job running; normal app shutdown terminates the owned child.

The GPUI preview updates from the engine's `XRGB1` files every sampling step.
These are approximate latent previews; only the final image is VAE-decoded.
Completion checks hardware execution, selected model, steps and image dimensions.
Each job gets a new LocalState directory; outputs and diagnostics are retained.
Native keyboard character input is supported; IME and the Xbox on-screen keyboard
are not implemented.

For an explicit one-shot device check, upload `studio-start.request` to this
app's LocalState using the full native request schema (`model`, `prompt`,
`negative_prompt`, `seed`, `width`, `height`, `steps`, `cfg_scale`, `gpu: true`,
`preview_every: 1`). It is consumed once when idle. `studio-status.json` reports
availability, progress and errors. Existing Workbench presets are never read.

## Component workbench

The UI composes GPUI Kit `ButtonGroup`, `Button`, `TabBar`, `DescriptionList`,
`Tag` and `Progress` components around the native workload. Size selection and
execution are separate actions. Latest result shows verification and timings;
Recent runs retains the newest five samples for this session. Changing views
never starts a workload. All components share the television typography scale.

Focus a workload-size button, Run, or the Latest/Recent tabs and activate it.
Size changes and duplicate runs are blocked while busy; errors leave prior
results available. These are individual samples, not a throughput benchmark.

## Keyboard and controller navigation

All tools share one application shell, command list and television theme.
Keyboard arrows keep their normal editing behavior. Controller focus movement
uses a separate command bridge, not simulated editor arrow keys. A sends both
key-down and key-up so standard GPUI Kit buttons actually activate.

| Controller | Keyboard | Action |
| --- | --- | --- |
| LB / RB | Ctrl 1–4 | Previous/next tool; direct tool selection |
| D-pad / left stick | F6 / Shift F6 | Next/previous focusable control |
| A | Enter / Space on buttons | Activate focused control |
| B | Escape | Close commands, return from Controls, leave editor, cancel active job |
| Menu | Ctrl Shift P | Commands |
| View | F1 | Controls |
| LT / RT | Page Up / Page Down in editor | Scroll active tool; editor pages or IDE diagnostics |

Stick navigation has a 0.55 dead zone, 350 ms initial repeat delay and 120 ms
repeat interval; triggers repeat at 160 ms. Elite paddles follow the user's
[Xbox Accessories profile](https://news.xbox.com/en-us/2023/10/26/xbox-october-update-2023/).
The app does not claim separate raw paddle events. Controls uses a generated
transparent controller illustration, with all binding text rendered by GPUI.
The artwork prompts are in [design/imagegen-prompts.txt](design/imagegen-prompts.txt).

## GPUI IDE

The IDE ports the source-editing workflow into GPUI Kit `Editor` and `Input`
components: retained per-file buffers, undo/redo, selection, indentation, a
searchable file list, dirty markers, explicit Save and native Rust diagnostics.
Ctrl P focuses file search; Enter opens the first match. Ctrl S saves; F5 checks
the selected Rust source. Tab indents within the editor; F6 moves application
focus without consuming Tab. Switching tools preserves buffers and running jobs.
Copy/cut/paste use an app-local clipboard; there is no cross-app clipboard yet.

The workspace is `D:\DevelopmentFiles\GpuiWorkspace` when present, otherwise
this app's `LocalState/workspace`. Discovery is bounded to 64 Rust files, 1 MiB
per file and eight directory levels, excluding `target`, `.git` and symlinks.
An initial `main.rs` is created only if missing. Saves reject external edits
and retain the prior version as a hidden backup. Checks compile a snapshot of
open buffers with `rustc --emit=metadata`, without saving or linking a binary.
The existing Xbox compiler must be privately staged at
`D:\DevelopmentFiles\GpuiRuntime\compiler`; no compiler binaries are in git.
This is the GPUI editing/checking port, not yet parity with the old Workbench's
Git, terminal, AI and full project-build features.

For device verification, the app consumes its own `navigation.request.json`
once, with `{ "id": "unique", "inputs": [...] }`. Each input is
`{ "op": "command", "code": 1 }` (1–4 tools, 5/6 previous/next, 7/8 focus,
9 commands, 10 back, 11 save, 12 check, 13 IDE editor, 14 IDE files),
`{ "op": "key", "key": "ctrl-a" }`, `{ "op": "text", "text": "..." }`, or
`{ "op": "activate" }`. Maximum 32 inputs and 64 KiB per request. Requests are
renamed once; `navigation.consumed.txt` acknowledges the ID and
`navigation-status.json` reports active tool and IDE diagnostics. This is scoped
to this app's LocalState, not the original Workbench's storage.

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
- `ui/src/lib.rs`: GPUI Kit theme, lifecycle and guarded C ABI. Panics cannot
  unwind across C++.
- `ui/src/workbench.rs`: component composition, controller commands and bounded
  session result history.
- `src/App.cpp`: CoreWindow, gamepad polling, OS font loading, native GPU work
  and LocalState report storage. Direct2D no longer draws the interface.
- `patches/`: pinned GPUI WGPU and WGPU HAL changes for DirectX 12 selection and
  `CreateSwapChainForCoreWindow`. The HAL owns a COM reference and never treats
  CoreWindow as a desktop HWND.

Preparation downloads the pinned crate releases, checks/applies these patches
and decodes the checked-in package artwork. Generated sources, output packages
and private signing material are ignored. The Rust lockfile is checked in.

Native file dialogs, external URL launching, system clipboard, desktop menus,
IME and the on-screen keyboard are not implemented. This remains an experimental
single-window Xbox platform port.

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

The Models/IDE/Controls deployment and native runtime checks are recorded in
[validation/xbox-tools-2026-10-02.json](validation/xbox-tools-2026-10-02.json).
All three existing checkpoints completed 256×256, two-step GPU generation;
intermediate preview, cancellation, editor save/undo/clipboard and native Rust
diagnostics were checked through this app’s input bridge. Physical controller
and keyboard interaction still need separate verification.

The Models page supports FP32 and, when the separately staged precision engine is present, FP16 mixed and BF16 mixed. Mixed modes pack linear/convolution inputs and weights into 16-bit GPU operands, with FP32 accumulation, normalization, attention reductions and outputs. They currently support 256 × 256 images. FP16 overflow fails with a recoverable error; no mode silently falls back. Completion checks the engine’s reported mode and nonzero packed operand/dispatch counters. Lower precision can change the image; faster generation is not promised.


Xbox hardware verification: [precision report](validation/xbox-precision-2026-10-02.json). All three downloaded models completed FP16 and BF16 generation, with decoded PNGs and live previews. These low-step checks validate the pipeline, not image quality or performance. The measured mixed runs were slower than the existing FP32 engine.
