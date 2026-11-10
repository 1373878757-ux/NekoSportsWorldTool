# NekoSportsWorldTool｜简体中文便携包说明

## 支持平台

- macOS Apple Silicon（aarch64-apple-darwin，M 系列芯片）；Intel Mac 不适用。
- Windows 10/11 x64（x86_64-pc-windows-msvc）。
- GUI 包同时包含命令行入口。普通使用不需要安装 Rust、Cargo、Python 或 Node.js。

## macOS 使用

1. 将 ZIP 解压到自己有读写权限的文件夹。
2. 将 NekoSportsWorldTool.app 移到“应用程序”文件夹（可选），然后双击打开。
3. 首次启动会显示启动动画，初始化完成后进入主界面。

此包面向 Apple Silicon，没有 Apple Developer ID 签名或公证。macOS 可能显示来源或安全提示；请先核对 ZIP 的 SHA-256 与交付时公布的值。包内不更改 Gatekeeper 或系统安全设置。如果系统阻止启动，请联系提供此包的人，不要关闭系统安全保护。

## Windows 使用

1. 将 ZIP 完整解压到一个本地文件夹，不要直接在压缩包预览中运行。
2. 双击 NekoSportsWorldTool.exe。
3. 需要命令行帮助时，在该目录打开 PowerShell 并运行：

       .\NekoSportsWorldTool.exe --help
       .\NekoSportsWorldTool.exe --version

该 EXE 不带可信发布者的代码签名，Windows 可能提示未知发布者或 SmartScreen。请先确认下载来源并校验 SHA-256；不要为运行它关闭 Defender 或 SmartScreen。Actions 构建成功不等于所有 Windows 电脑均已完成实机兼容性测试。

## 本地数据

程序文件与用户数据分开保存：macOS 默认写入 ~/Library/Application Support/NekoSportsWorldTool-J-claude，Windows 默认写入 %LOCALAPPDATA%\NekoSportsWorldTool-J-claude。删除 ZIP 或应用文件不会自动删除这些用户数据。请勿手工删除或混用身份、会话或配置文件；换电脑时不要复制设备身份文件。

如只想隔离查看 GUI，可在启动前设置 NSWT_OFFLINE=1 和一个独立的 NSWT_DATA_DIR。离线模式阻止应用 HTTP 请求；只有自行准备的本地文件或缓存可用。离线模式不能验证真实学校服务、登录或在线更新。

## 许可证与校验

请一并保留 ZIP 内的 LICENSE、THIRD-PARTY-NOTICES.txt 和 licenses/。SHA256SUMS.txt 校验包内文件；与 ZIP 同目录的 SHA256SUMS.txt 校验 ZIP 本身。

原项目作者为 YanamiNeko，源码历史保留 ConCosew 来源，当前定制工程来源为 1373878757-ux/NekoSportsWorldTool。定制修改涉及界面、跨平台兼容与发行工程，详情可通过 BUILD-INFO.txt 中的提交追溯。项目依据 CC BY-NC 4.0 分发；嵌入的 Noto Sans SC 字体依据 SIL Open Font License 1.1 分发，第三方依赖许可文本位于 licenses/。本便携包供非商业使用。

## 使用边界

本包提供原有功能；它不代表已验证所有账号、学校接口或设备组合。请遵循学校平台规定。本次打包不会执行账户操作，也不构成真实运动数据写入测试。遇到道路服务不可用时，可能无法进行依赖在线道路数据的功能。
