# GPUI Kit Xbox 原型

本示例将 GPUI Kit 嵌入 Xbox UWP 原生应用。界面由 GPUI 的 WGPU renderer 通过 DirectX 12 和 CoreWindow 绘制；矩阵运算和数值校验沿用 [richrobber2/xbox-native-gpu](https://github.com/richrobber2/xbox-native-gpu) 的 Direct3D 11 实现。

这是实验性平台后端，已于 2026-10-02 在 Xbox Series X 上启动。实际 GPUI 界面通过 DirectX 12 绘制，D3D11 的 256×256 计算验证了全部 65,536 个输出。实体手柄输入及挂起/恢复仍需单独验证。

## 已下载模型工作台

Models 为默认工具；LB/RB 或 Ctrl 1–4 在 Models、IDE、GPU、Controls 间切换。工作台选择
`D:\DevelopmentFiles\AnimaModels` 中现有的 Anima Preview 3、MiaoMiao Anima 1.6
及 MiaoMiao Anima 2.9B Beta 1.1 checkpoint。私有原生引擎和共享 encoder、VAE、
tokenizer 需放在 `D:\DevelopmentFiles\AnimaModels\GpuiRuntime`，同时放入引擎现有的
`VCRUNTIME140.dll` 和 `vcruntime140_1.dll`；compiler bin 目录也需要其 runtime
依赖。仓库不包含
引擎或权重，也不会自动下载。

用 F6、Shift F6 或方向键/左摇杆移动组件焦点，以 Enter/A 选择模型、编辑 prompt、
切换 sampling steps 或执行 Generate/Cancel。默认参数为 256×256、seed 42、CFG 4。
生成期间禁止切换模型；切换工具时计算继续，正常退出时终止应用拥有的子进程。

GPUI 每个 sampling step 读取引擎的 `XRGB1` 预览。这是近似 latent preview，
仅最终图像经过 VAE decode。完成时校验硬件执行、模型、steps 和图像尺寸。
每次生成使用新的 LocalState 目录并保留输出和诊断。支持原生键盘字符输入，
尚未实现 IME 和 Xbox 屏幕键盘。

设备验证可明确上传一次性的 `studio-start.request` 到本应用 LocalState，使用完整
原生 schema：`model`、`prompt`、`negative_prompt`、`seed`、`width`、`height`、
`steps`、`cfg_scale`、`gpu: true`、`preview_every: 1`。空闲时仅消费一次。
`studio-status.json` 提供模型可用状态、进度和错误；不读取现有 Workbench presets。

## Component 工作台

界面使用 GPUI Kit 的 `ButtonGroup`、`Button`、`TabBar`、`DescriptionList`、
`Tag` 和 `Progress` 组合原生计算流程。选择尺寸与执行计算分开；Latest result
显示校验和耗时，Recent runs 保存当前会话最新的五次结果。切换视图不会执行计算。
所有组件统一采用适合电视观看的字号比例。

聚焦尺寸按钮、Run 或 Latest/Recent tabs 后执行。计算期间禁止改变尺寸和重复运行；
错误不清除已有结果。每次结果仅为单次采样，不代表吞吐量基准。

## 全局键盘与手柄导航

四个工具共用导航栏、命令列表和电视字号。手柄焦点命令与编辑器方向键分开；
A 同时发送 key-down 和 key-up，直接激活 GPUI Kit 组件。

| 手柄 | 键盘 | 操作 |
| --- | --- | --- |
| LB / RB | Ctrl 1–4 | 上一个/下一个工具；直接选择工具 |
| 方向键 / 左摇杆 | F6 / Shift F6 | 下一个/上一个组件焦点 |
| A | Enter / 按钮上的 Space | 激活当前组件 |
| B | Escape | 关闭命令、返回、离开编辑器或取消任务 |
| Menu | Ctrl Shift P | 命令列表 |
| View | F1 | Controls |
| LT / RT | 编辑器中的 Page Up / Page Down | 滚动工具、编辑器或 IDE 诊断 |

摇杆 dead zone 为 0.55，首次重复延迟 350 ms，后续 120 ms；扳机后续 160 ms。
Elite paddles 使用用户的 Xbox Accessories profile，不声称独立读取原始 paddle 事件。
Controls 使用生成的透明手柄插图，绑定文本全部由 GPUI 绘制；prompt 见
[design/imagegen-prompts.txt](design/imagegen-prompts.txt)。

## GPUI IDE

使用 GPUI Kit Editor/Input 移植源码编辑流程：每文件保留 buffer、undo/redo、选择、
缩进、文件搜索、dirty 标记、保存以及本机 Rust 诊断。Ctrl P 聚焦文件搜索，Enter
打开首个匹配项；Ctrl S 保存，F5 检查当前 Rust 文件。编辑器内 Tab 缩进，F6
移动应用焦点。切换工具保留编辑内容和任务。复制/剪切/粘贴使用应用内剪贴板。

优先使用 `D:\DevelopmentFiles\GpuiWorkspace`，否则使用本应用的
`LocalState/workspace`；最多读取 64 个 Rust 文件、每个 1 MiB、八层目录，排除
`target`、`.git` 和符号链接。仅当不存在时创建初始 main.rs。保存时拒绝覆盖外部
修改，保留隐藏备份。检查以当前 buffers 的副本运行 `rustc --emit=metadata`，
不保存或链接 binary。现有 Xbox compiler 需私有地放在
`D:\DevelopmentFiles\GpuiRuntime\compiler`，仓库不包含 compiler binaries。
旧 Workbench 的 Git、terminal、AI 和完整 project build 尚未全部移植。

设备检查可在本应用 LocalState 上传一次性的 `navigation.request.json`：
`{ "id": "unique", "inputs": [...] }`，最多 32 个输入、64 KiB。
输入支持 `op: command` 加 `code`（1–4 工具，5/6 前后工具，7/8 前后焦点，9 命令，
10 返回，11 保存，12 检查，13 编辑器，14 文件搜索），或 `op: key` 加 `key`、
`op: text` 加 `text`、`op: activate`。请求仅消费一次，ack 为
`navigation.consumed.txt`，状态和诊断见 `navigation-status.json`。

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

系统剪贴板、原生文件对话框、外部 URL、桌面菜单、IME 和屏幕键盘尚未实现；这仍是实验性的单窗口 Xbox platform 移植。

## 验证

本地 Android target 的 Rust check 验证 platform trait 和应用代码，不执行 CoreWindow/DirectX 12 路径。Windows workflow 构建真正的 UWP target、运行独立矩阵参考测试并验证安装包签名。设备报告见 [validation/xbox-series-x-2026-10-02.json](validation/xbox-series-x-2026-10-02.json)，已验证的 loose-folder 部署命令见 [INSTALL.zh-CN.md](INSTALL.zh-CN.md)。Xbox 还在本机编译并运行了独立的 Rust 手柄选择 smoke check，并编译了本示例的实际 HLSL shader。这些检查不代表已在主机上完成完整 GPUI Cargo/UWP 安装包构建。

Models/IDE/Controls 部署和原生运行验证见
[validation/xbox-tools-2026-10-02.json](validation/xbox-tools-2026-10-02.json)。
三个现有 checkpoint 均完成了 256×256、两步 GPU 生成；通过本应用输入 bridge
验证了中间预览、取消、编辑器保存/undo/剪贴板和本机 Rust 诊断。实体手柄与
键盘交互仍需单独验证。
