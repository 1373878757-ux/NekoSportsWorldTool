# NekoSportsWorldTool — Windows x64

just callme...

prod.by 田宸宇 J Claude

本定制版基于 YanamiNeko 的 NekoSportsWorldTool 及 ConCosew 维护分支，保留原项目功能与版权。上述完整署名表示定制和工程修改，不代表原项目全部由署名者原创。本作品按原 CC BY-NC 4.0 许可证使用和分享，禁止商业用途。

## 启动

1. 将 ZIP 完整解压到一个本地目录，不要直接在压缩包内运行。
2. 双击 `NekoSportsWorldTool.exe`。
3. 启动页中央显示 `just callme...`，底部显示 `prod.by 田宸宇 J Claude`，本地初始化成功后进入主界面。

普通用户不需要安装 Rust、Cargo、Python、Node.js 或开发 SDK。目标平台为 Windows 10 / Windows 11 x64；构建成功不等于所有 Windows 设备和显示设置已实机验收。

本发行包未作代码签名，Windows 可能显示信誉或未知发布者提示。请核对来源及 SHA-256；不要通过关闭 Defender 或 SmartScreen 来处理问题。

## 本地数据与离线验证

默认用户数据目录为 `%LOCALAPPDATA%\NekoSportsWorldTool-J-claude`，程序与配置分开保存。旧版程序旁的数据仅在新目录没有已知状态时按白名单复制，原件保留；不要随意删除 `identity.json` 或手动混合不同设备的身份和会话。

首次身份保存使用同目录硬链接进行无覆盖发布，避免同时启动的两个进程互相替换设备身份。请使用支持硬链接的本地 NTFS 数据目录；不支持此操作的自定义文件系统会显示初始化错误，并保留已有文件。

如需隔离的离线验证，可在 PowerShell 中设置：

```powershell
$env:NSWT_OFFLINE = '1'
$env:NSWT_DATA_DIR = Join-Path $env:TEMP ('neko-offline-' + [guid]::NewGuid())
& '.\NekoSportsWorldTool.exe'
```

该模式禁止应用 HTTP 请求，不导入真实用户目录，并禁用联网操作。退出测试 PowerShell 后，环境变量不影响其他会话。离线验证使用本地 GPX/JSON/OSM 或合成数据；不要用真实账户执行提交、补签等操作。

## 命令行

无参数启动 GUI；带参数进入 CLI，不应弹出启动页：

```powershell
& '.\NekoSportsWorldTool.exe' --help
& '.\NekoSportsWorldTool.exe' --version
& '.\NekoSportsWorldTool.exe' template --file '.\sample.gpx'
```

`template` 只读取用户手动选择的本地 GPX/JSON，进行海拔分析，不上传该文件。其他命令以本包 `--help` 的实际输出为准。

## 更新与校验

正常模式进入主界面后，程序按设置检查自己的定制发行源。没有定制 Release 或当前平台资产时可继续使用；不会回退用上游原版覆盖定制版。下载与替换需明确确认。

包内 `SHA256SUMS.txt` 校验其他发行文件；ZIP 自身校验值位于 ZIP 旁、同次构建生成的另一份 `SHA256SUMS.txt` 中。请保留 `LICENSE`、`THIRD-PARTY-NOTICES.txt` 和 `licenses` 目录；字体和第三方代码的许可文本随包保留。

源码与来源：

- 原项目：<https://github.com/YanamiNeko/NekoSportsWorldTool>
- 本次源码来源：<https://github.com/ConCosew/NekoSportsWorldTool>
- 定制开发 Fork：<https://github.com/1373878757-ux/NekoSportsWorldTool>

本次交付仅验证离线及 Mock 能力，不构成真实账户、学校服务接口或运动记录提交的验收结论。
