# GPUI Component 基础示例

本目录中的每个示例演示 GPUI Component 的一个功能。使用 `cargo run -p <package-name>` 运行 Rust package。较大的示例将 `story` gallery 作为普通依赖，因此不会启用它用于测试的开发依赖。

| 示例 | 命令 |
| --- | --- |
| Editor | `cargo run -p example-editor` |
| Brush | `cargo run -p example-brush` |
| Dock | `cargo run -p example-dock` |
| HTML | `cargo run -p example-html` |
| 大文本 | `cargo run -p example-large-text` |
| Markdown | `cargo run -p example-markdown` |
| Streaming Markdown | `cargo run -p example-stream-markdown` |
| 文本选择 | `cargo run -p text_selection` |
| 触摸选择 | `cargo run -p touch_selection` |

共享示例文档位于 `fixtures/`。

实验性的 [Xbox GPU lab](xbox-native-gpu/README.zh-CN.md) 将 GPUI Kit 嵌入 UWP CoreWindow，使用 DirectX 12 绘制界面，通过原生 Direct3D 11 计算。它使用独立 Rust workspace 和 Windows packaging workflow。

## 打开窗口

示例先调用 `gpui_kit::init(cx)`，再使用 `gpui_kit::open_window(options, cx, build)`。此 helper 挂载 Base Root，并返回 window handle 和内容 entity。原生与 Web story gallery 使用同一路径。

无界面的测试 fixture 可通过 GPUI test harness 直接创建 Root。

## 贡献

欢迎添加新示例并提交 pull request。每个示例应只演示一个功能，验证其行为，在关键代码处添加说明，并遵循现有示例和 GPUI Component 的代码风格与命名约定。
