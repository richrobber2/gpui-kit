# 使用手机安装到 Xbox

此开发应用用于处于 Developer Mode 的 Xbox Series X|S。家中不需要 Windows PC，但需要 Windows cloud runner 构建安装包。

## GitHub 构建

1. 使用 `richrobber2/gpui-kit` 的 `xbox-prototype` branch，源码位于 `examples/xbox-native-gpu`，需要完整 GPUI Kit 仓库。
2. 打开 **Actions → Build GPUI Kit Xbox prototype → Run workflow**，选择 `xbox-prototype`。使用账户现有的 Actions 配额和计费规则。
3. 等待成功，下载并解压 **gpui-kit-xbox-x64** artifact。
4. 找到主 `XboxGpu...x64...appx` 和 `Dependencies/x64/*.appx`。`.cer` 是公开开发证书，不包含私钥。

## Device Portal 安装

1. 启动 Dev Mode 并登录，在 **Dev Home → Remote Access Settings** 中启用 Device Portal 和身份验证，在设备上设置凭据。
2. 手机连接同一局域网，打开 Dev Home 显示的 HTTPS 地址。先确认是本机 Xbox，再接受自签名证书提示。不要将 Portal 暴露到公网。
3. 登录后选择 **Add / Deploy app**，上传主 APPX 和必要的 x64 dependency APPX。不要使用 APPXUPLOAD 文件。
4. 在 Dev Home 中打开应用详情，将 **App type** 设置为 **Game**，然后启动。

## 已验证的命令部署

2026-10-02，测试主机的 APPX 更新流程报告成功，但应用无法启动；loose-folder 流程成功启动了同一 executable。在仓库中对下载的原始 artifact ZIP（不是内部 APPX）运行：

```bash
python3 examples/xbox-native-gpu/scripts/deploy.py gpui-kit-xbox-x64.zip \
  --portal https://YOUR-XBOX:11443 --insecure
```

`--insecure` 接受本机 Xbox 的自签名 TLS 证书。Portal 需要身份验证时添加 `--username YOUR-USER`；密码通过提示输入，不写入命令参数。`--dry-run` 仅验证 artifact，不连接主机。

命令确认 Portal 是 Xbox、验证 package identity，然后上传按 APPX hash 命名的目录、注册并启动应用。它只停止已有的 Xbox GPU Lab process，保留其他应用及其文件。注册后的应用从上传的开发目录运行，请保留该目录。HTTP 成功不能代替启动验证；仍需检查实际界面和 LocalState 报告。

## 运行

应用打开后自动执行一次经过数值校验的 256×256 计算。

- D-pad 或键盘方向键：选择 64、128 或 256，默认 256。
- A 或 Enter：运行选中的矩阵乘法。
- X 或键盘 X/Space：128×128。Y：64×64。
- 每次计算都与单线程 C++ reference 对比全部输出。
- 界面显示系统报告的实际应用内存限制，不进行大内存压力测试。
- GPU 时间包括 buffer 创建、上传、dispatch、同步和回读，不包括 device/shader 初始化。单次结果不能作为可靠吞吐率基准。

应用在自己的 LocalState 中写入 `native-gpu-result.json`；可通过 Device Portal 文件视图查看，或拍摄结果界面。存储仍可用时，错误写入 `error.txt`。

界面使用 DirectX 12，计算使用 Direct3D 11 hardware。GPUI 绘制和初始计算已在 Xbox Series X 上验证，实体手柄输入及挂起/恢复仍为实验性。启动失败时请保留 `error.txt` 和 Device Portal 诊断。

## 故障排查

- 缺少依赖：添加同一 artifact 的 x64 dependency package。
- 内存限制约 1 GB：确认 App type 为 Game 后重启应用；显示值以系统报告为准。
- 无法更新：每次构建使用相同 publisher name 的新开发证书。卸载旧开发应用再安装会删除该应用的本地报告。
- 设备、签名或部署错误：保留完整错误，不要关闭身份验证或修改系统时间。
- GPU 错误：关闭并重启应用；驱动仍不可用时重启主机。本应用不使用软件图形 fallback。

## 状态

portable CPU 测试已通过。Windows workflow 编译 C++ 和 HLSL、生成并验证签名，然后上传安装包。上述命令已从手机执行：GPUI 成功绘制，首次及重复启动均验证了全部 65,536 个 GPU 输出，系统报告 5 GiB 内存限制。单次 GPU 总时间约 18 ms，包含分配、传输和同步，不能据此宣称吞吐率提升。源码 ZIP 不能直接安装，请使用成功构建 artifact 中的 APPX。
