# GPUI Kit Android demo

A native Android counter demo with Add and Reset buttons and a dark mode switch.
Counter state lasts for the current application process.

## Build

The supported environment is Debian ARM64 in Termux PRoot on Android. Required
tools are Rust/Cargo, the `aarch64-linux-android` Rust target, Termux Clang and
LLVM tools, OpenJDK 21, `aapt`/`aapt2`, `apksigner`, `zipalign`, `zip`, and `patchelf`.
The Cargo configuration uses the Termux compiler paths.

Use this repository for GPUI Kit and clone GPUI Mobile next to `gpui-kit`:

| Repository | Revision |
| --- | --- |
| [GPUI Kit](https://github.com/richrobber2/gpui-kit) | `3a142844d3661159964dce9e5512ca9a40286160` |
| [GPUI Mobile](https://github.com/longbridge/gpui-mobile) | `9075e3aa3eea812127f2c60ed66f0cd5798ff245` |

Download [Google's API 34 platform archive](https://dl.google.com/android/repository/platform-34-ext12_r01.zip)
and extract its `android.jar` to `android/android.jar`. Download
[R8 8.3.37](https://dl.google.com/dl/android/maven2/com/android/tools/r8/8.3.37/r8-8.3.37.jar)
to `android/r8.jar`. These local build dependencies are excluded from Git.

```sh
rustup target add aarch64-linux-android
```

```sh
cd gpui-kit/examples/android-demo
cargo run --manifest-path xtask/Cargo.toml -- apk
```

The output is `dist/gpui-kit-demo.apk`, signed with an Android debug key. The
app targets ARM64 and Android API 29 (Android 10) or later, and requests no network or
sensitive permissions.

Build and open Android's installer in one command (no ADB required):

```sh
cargo run --manifest-path xtask/Cargo.toml -- install
```

Approve installation when Android prompts. To open the existing APK without
rebuilding, use `cargo run --manifest-path xtask/Cargo.toml -- open`.
The command stages the APK in Termux's real home so its content provider can
share it outside the PRoot filesystem.

For an already connected ADB device, install with
`adb install -r dist/gpui-kit-demo.apk`.

To build the Rust library separately, use
`cargo build --lib --locked --target aarch64-linux-android`, then `bash package.sh`.

The app loads GPUI Kit from `../../crates/kit` and the compatible mobile
platform from `../../../gpui-mobile`, revision
`9075e3aa3eea812127f2c60ed66f0cd5798ff245`. Both use GPUI snapshot `0.3.7`.
Cargo.lock pins the remaining dependencies.

This build uses Termux's Android ARM64 compiler, Debian's APK packaging tools,
OpenJDK 21, and Google's API 34 platform JAR. Java compilation uses that JAR;
`aapt2` packages resources against `/system/framework/framework-res.apk`, following
RustDL's Termux build. Set `FRAMEWORK_RES` to override that resource framework.
The compiler paths in
`.cargo/config.toml` are specific to this host; use an Android NDK toolchain on
other systems. The standard debug signing password is `android`.

Android's NativeActivity hosts the Rust library through a small Java activity
subclass. Google's R8/D8 compiler is stored at `android/r8.jar` for packaging.
The installed app requires no external service.

Keep `android/debug.keystore` private and back it up. Future updates must use the
same signing key. Build outputs, generated Java helpers, logs, SDK JARs, and
signing keys are excluded from Git.

## Device verification

The demo was installed and launched on a Moto G Play (2024), Android 14, with
an Adreno 610 Vulkan adapter. The counter rendered and responded to taps;
the startup abort and missing platform-view helper error no longer occurred.

Packaging includes GPUI Mobile's platform-view lifecycle helper and its Java
dependencies from the pinned mobile checkout. The debug profile disables
`wgpu-types` debug assertions so WGPU's default instance flags do not require
Vulkan debugging entry points that some Android drivers omit.
