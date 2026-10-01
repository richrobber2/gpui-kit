# GPUI Kit Android 示例

原生 Android 计数器示例，包含 Add、Reset 按钮和深色模式开关。
计数器状态仅保留在当前应用进程中。

## 构建

支持的构建环境为 Android 上 Termux PRoot 内的 Debian ARM64。需要
Rust/Cargo、`aarch64-linux-android` Rust target、Termux Clang 和 LLVM 工具、
OpenJDK 21、`aapt`/`aapt2`、`apksigner`、`zipalign`、`zip` 和 `patchelf`。
Cargo 配置使用本机的 Termux 编译器路径；其他系统需要配置 Android NDK。

使用当前仓库中的 GPUI Kit，并将 GPUI Mobile 克隆到 `gpui-kit` 的同级目录：

| 仓库 | 修订版本 |
| --- | --- |
| [GPUI Kit](https://github.com/richrobber2/gpui-kit) | `3a142844d3661159964dce9e5512ca9a40286160`，作为本示例的基础版本 |
| [GPUI Mobile](https://github.com/longbridge/gpui-mobile) | `9075e3aa3eea812127f2c60ed66f0cd5798ff245` |

下载 [Google API 34 平台压缩包](https://dl.google.com/android/repository/platform-34-ext12_r01.zip)，
将其中的 `android.jar` 解压到 `android/android.jar`。下载
[R8 8.3.37](https://dl.google.com/dl/android/maven2/com/android/tools/r8/8.3.37/r8-8.3.37.jar)
到 `android/r8.jar`。本地构建依赖不会提交到 Git。

```sh
rustup target add aarch64-linux-android
cd gpui-kit/examples/android-demo
cargo run --manifest-path xtask/Cargo.toml -- apk
```

输出为 `dist/gpui-kit-demo.apk`，使用 Android 调试签名。应用支持 ARM64 和
Android API 29（Android 10）及以上版本，不申请网络或敏感权限。

构建后直接打开 Android 安装程序，无需 ADB：

```sh
cargo run --manifest-path xtask/Cargo.toml -- install
```

Android 提示时确认安装。已有 APK 可使用
`cargo run --manifest-path xtask/Cargo.toml -- open` 打开，无需重新构建。
命令先将 APK 复制到 Termux 的真实 home 目录，以便 content provider
在 PRoot 文件系统之外分享文件。

已连接 ADB 设备时，也可使用 `adb install -r dist/gpui-kit-demo.apk` 安装。
单独构建 Rust 库时，运行
`cargo build --lib --locked --target aarch64-linux-android`，然后运行 `bash package.sh`。

应用依赖 `../../crates/kit` 和 `../../../gpui-mobile`，均使用 GPUI snapshot
`0.3.7`。其余依赖由 Cargo.lock 固定。

Java 编译使用 API 34 平台 JAR，`aapt2` 使用设备上的
`/system/framework/framework-res.apk` 打包资源。可通过 `FRAMEWORK_RES` 覆盖该路径。
NativeActivity 通过小型 Java Activity 子类加载 Rust 库，R8/D8 编译器位于
`android/r8.jar`。安装后的应用无需外部服务。

请妥善备份并保密保存 `android/debug.keystore`；后续更新必须使用相同签名密钥。
默认调试签名密码为 `android`。构建输出、生成的 Java helper、日志、SDK JAR 和
签名密钥均排除在 Git 之外。

## 设备验证

已在搭载 Android 14 和 Adreno 610 Vulkan GPU 的 Moto G Play (2024) 上安装并运行。
计数器正常显示并响应点击，启动中止和 platform-view helper 缺失错误已消失。

打包时从固定版本的 GPUI Mobile checkout 复制 platform-view 生命周期 helper
及其 Java 依赖。调试 profile 禁用 `wgpu-types` 的 debug assertions，避免默认
WGPU instance flags 请求某些 Android 驱动未提供的 Vulkan 调试入口。
