# Windows Build Report

## 目标与当前状态

目标：Windows 10 / Windows 11 x64，`x86_64-pc-windows-msvc`。本次使用自有 Fork 的 Windows Runner，不以 Mac target 安装或旧 GNU 交叉编译报告代替 MSVC 构建证据。

当前 Windows 构建、EXE、ZIP、SHA-256：待本次 Actions 实际结果回填。

## 构建与发布约束

- Runner：`windows-2022`；项目 Rust 工具链与 Mac 一致为 1.96.1。
- 开发分支：`feature/j-claude-cross-platform`；定制标签使用 `jclaude-v*`，避开原 Android 的 `v*` 发布触发。
- 本次只推送开发分支并运行 Actions；不创建公开 Release，不创建发布标签。
- 默认 GUI 与 CLI-only 分别执行 locked 检查、测试和构建；普通工作流权限为只读仓库内容。
- 发布版本目标 `0.3.1`，合法 Windows 数字版本 `0.3.1.0`；主产品名仍为 NekoSportsWorldTool。
- 检查 PE x64 架构、版本资源、图标、动态依赖和实际签名状态；设置静态 CRT 不等同于证明无外部 DLL。

## 便携包

目标文件：`NekoSportsWorldTool-J-claude-Windows-x64.zip`。根目录包括程序、原 LICENSE、Windows 说明、第三方许可索引/文本及包内 SHA256SUMS；ZIP 自身另有校验值。

打包只使用白名单，不包含账户配置、设备身份、Token、缓存、日志、上传回执、PDB、无关 DLL 或历史 downloads。许可证脚本必须在缺失实际许可文本时失败。

`scripts/collect_licenses.py` 使用 locked Windows 目标的实际普通依赖闭包，显式以 UTF-8 解码 Cargo metadata。缺失的上游许可采用固定提交和 SHA-256 校验的已缓存文本。Mac 上已使用 `--target x86_64-pc-windows-msvc --offline` 实跑通过：262 个依赖包、533 份许可/来源文件，连同索引共 534 个文件。证据为 `verification/local/license-collector-windows.log` 与 `.json`；此结果只证明许可收集，不替代 Windows 编译。

## 证据记录（待补）

| 项目 | 本次证据 |
|---|---|
| Actions URL / Run ID / Commit | 未执行或运行中，待回填 |
| MSVC check / test / build | 待回填 |
| EXE 文件、架构、版本、依赖、签名 | 待回填 |
| ZIP 解压与解压后 CLI | 待回填 |
| 包内 SHA / ZIP SHA | 待回填 |
| 本地下载产物 | 待回填 |

Windows Runner 成功不能代替 Windows 10/11 的图形、DPI 和双击启动实机验收；项目详见 Windows_Acceptance_Checklist.md。
