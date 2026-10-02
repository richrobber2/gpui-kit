# GPUI Kit Xbox 原型

本示例将 GPUI Kit 嵌入 Xbox UWP 原生应用。界面由 GPUI 的 WGPU renderer 通过 DirectX 12 和 CoreWindow 绘制；矩阵运算和数值校验沿用 [richrobber2/xbox-native-gpu](https://github.com/richrobber2/xbox-native-gpu) 的 Direct3D 11 实现。

这是实验性平台后端，已于 2026-10-02 在 Xbox Series X 上启动。实际 GPUI 界面通过 DirectX 12 绘制，D3D11 的 256×256 计算验证了全部 65,536 个输出。实体手柄输入及挂起/恢复仍需单独验证。

## Component 工作台

界面使用 GPUI Kit 的 `ButtonGroup`、`Button`、`TabBar`、`DescriptionList`、
`Tag` 和 `Progress` 组合原生计算流程。选择尺寸与执行计算分开；Latest result
显示校验和耗时，Recent runs 保存当前会话最新的五次结果。切换视图不会执行计算。
所有组件统一采用适合电视观看的字号比例。

方向键左/右选择尺寸，A / Enter 执行，上显示最新结果，下显示历史结果，Tab
切换视图。X 执行 128×128，Y 执行 64×64。计算期间禁止改变尺寸和重复执行；
发生错误时仍可查看之前的结果。每次结果仅为单次采样，不代表吞吐量基准。

## 构建

运行仓库中的 **Actions → Build GPUI Kit Xbox prototype**，或在安装了 Visual Studio UWP C++ v142 workload 和 Windows SDK 22621 及以上版本的 Windows 环境运行：

```powershell
rustup toolchain install nightly-2026-09-30 --profile minimal --component rust-src
./examples/xbox-native-gpu/scripts/build.ps1
```

Rust 使用 `x86_64-uwp-windows-msvc` target，以 nightly `build-std` 构建标准库，不链接 GPUI 的桌面 Win32 platform。C++ 应用链接 Rust static library，编译 HLSL、生成 UWP 安装包并验证开发签名。构建产物名为 `gpui-kit-xbox-x64`。

每次构建生成临时开发证书，产物仅包含公开 `.cer`；私钥在构建结束时删除。安装包用于 Developer Mode，不用于零售模式或 Store 发布。

## 移植范围

- `ui/src/platform.rs`：单窗口 GPUI platform、文字排版、绘制、生命周期、输入和尺寸变化回调。
- `ui/src/dispatcher.rs`：由宿主驱动的前台 executor 和两个后台 worker；UWP 负责事件循环。
- `ui/src/lib.rs`：GPUI Kit theme、生命周期，以及阻止 panic 跨越 C++ 边界的 C ABI。
- `ui/src/workbench.rs`：组件组合、手柄命令，以及有数量上限的会话结果历史。
- `src/App.cpp`：CoreWindow、手柄轮询、系统字体加载、原生 GPU 运算和 LocalState 报告保存。界面不再通过 Direct2D 绘制。
- `patches/`：固定版本的 GPUI WGPU 和 WGPU HAL 补丁，支持 DirectX 12 和 `CreateSwapChainForCoreWindow`。HAL 持有 COM 引用，不将 CoreWindow 当成 HWND。

准备脚本下载固定版本的 crate，验证并应用补丁，解码仓库中的图标。生成的源码、安装包和私钥不纳入版本控制；Rust lockfile 已提交。

剪贴板、原生文件对话框、外部 URL、桌面菜单、文本编辑和 IME 尚未实现。目前支持 GPU lab 的手柄和键盘命令，尚不是通用 Xbox GPUI 发行版。

## 验证

本地 Android target 的 Rust check 验证 platform trait 和应用代码，不执行 CoreWindow/DirectX 12 路径。Windows workflow 构建真正的 UWP target、运行独立矩阵参考测试并验证安装包签名。设备报告见 [validation/xbox-series-x-2026-10-02.json](validation/xbox-series-x-2026-10-02.json)，已验证的 loose-folder 部署命令见 [INSTALL.zh-CN.md](INSTALL.zh-CN.md)。Xbox 还在本机编译并运行了独立的 Rust 手柄选择 smoke check，并编译了本示例的实际 HLSL shader。这些检查不代表已在主机上完成完整 GPUI Cargo/UWP 安装包构建。
