# Windows Build Report

## 目标与当前状态

目标：Windows 10 / Windows 11 x64，`x86_64-pc-windows-msvc`。本次使用自有 Fork 的 Windows Runner，不以 Mac target 安装或旧 GNU 交叉编译报告代替 MSVC 构建证据。

前三次 Windows Actions 依次暴露 API-set 误判、Windows checkout 行尾改变固定许可 SHA，以及标准 Windows Server 2022 Runner 缺少 OpenGL 2.0+ context。工作流现在把精确的 OpenGL context 错误记录为 `GUI smoke unavailable`，不会标成通过；CLI、PE、许可、ZIP 和校验错误仍会令任务失败。第四次 Actions 已成功并上传 artifact。下载包经独立校验：ZIP SHA-256 `13cf827b1baa29d5e49c4ca1f3e0874b06f493f398cd2964fa4cf7fa0f348c5d`，EXE SHA-256 `b35828baa6d834b61d4b4838994d62e8bc84c42b3e2680423a42ebd8ba6663f3`。该 CI 结果不代表 Windows 10/11 实机 GUI 验收通过。

- 仓库：`1373878757-ux/NekoSportsWorldTool`
- 分支：`feature/j-claude-cross-platform`
- Run ID：`42492957332`
- 本次运行：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42492957332>
- 第二轮修复提交：`f3b91f287aa7c928bc22f24cd9b6a2a0a677e9f1`
- 第二轮 Run ID：`42494146681`，<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42494146681>（许可完整性步骤 failure）
- 最终字节保全修复提交：`0b1ff9ec1ddee434bf80b866189146764bd8c381`
- 第三轮 Run ID：`42495076057`，<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42495076057>（failure：解压后 GUI smoke 无法取得 OpenGL 2.0+ 环境；artifact 上传 skipped）
- Windows context 分类提交：`36eeab7405f0e6f72d4faaa2fd73b509b274e84d`（仅将精确缺失 OpenGL context 记为 unavailable；其他 smoke/打包错误仍失败）
- 第四轮 Run ID：`42496483831`，<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42496483831>（success；GUI smoke 状态明确为 unavailable）
- Artifact：`NekoSportsWorldTool-J-claude-Windows-x64`，ID `13623878028`，大小 `16,477,808` bytes，artifact container SHA-256 `269c0af6112dcf8d48f7013c8bcb29f77946bcd23fedf32e71023e2ae0d21198`
- 已下载 ZIP：[NekoSportsWorldTool-J-claude-Windows-x64.zip](</Users/mac/Desktop/run fasting/artifacts/windows-run-42496483831/NekoSportsWorldTool-J-claude-Windows-x64.zip>)；独立校验 JSON：`/Users/mac/Desktop/run fasting/artifacts/windows-run-42496483831/independent-artifact-verification.json`

首轮失败点：`scripts/package_windows.ps1:37` 把导入项 `api-ms-win-core-synch-l1-2-0.dll` 作为必须在 System32 发现的物理 DLL，抛出 `Non-system DLL requires explicit packaging`。失败发生在打包依赖检查，未运行解压后 GUI 烟测，也未上传 artifact。已将规范命名的 Windows API-set 契约从物理 DLL 存在性检查中豁免，同时保留真实 DLL、动态 VC 运行库与依赖白名单检查；该修复已在后续 Actions 运行中通过。

首轮证据：`verification/windows/run-42492957332-full.log`、`-failed.log` 与 `-metadata.json`。Windows 测试主库 GUI 204、CLI-only 136，均另有路由库 8 与集成 7；两种 release 构建均完成。该轮整体结论仍为 **failure**，不能标成 Windows 发行包通过。

第二轮 `42494146681` 同样完成 GUI/CLI 检查、测试、clippy 和 release 构建，API-set 检查已通过，但许可 collector 报 `Upstream license hash mismatch for accesskit@0.16.3`，未到解压后 EXE 烟测或 artifact 上传。真实 Git checkout 复现确认 `core.autocrlf=true` 改写了固定许可文本：Apache 文件从原 Git blob 9723 字节变为 9899 字节，SHA-256 从 `62c7a1e3…` 变为 `20fe7b00…`；MIT/chromium 同样改变，manifest 始终匹配原始 blob。证据为 `run-42494146681-crlf-reproduction.json` 与该轮 full/failed 日志。

修复范围为 `.gitattributes` 对 `scripts/license_sources/**` 设置 `-text`，并让该文件变更触发工作流；保留原始许可源字节、collector 及 manifest 的严格 SHA 校验，不用放宽或重算错误文本 hash 的方式绕过完整性检查。真实 `core.autocrlf=true` checkout-index 前后比较已确认 30 份唯一固定许可文本全部保持原始 SHA，证据为 `license-crlf-before.json` 与 `license-crlf-after.json`。第三轮已确认 262 个包、533 份许可文件收集通过，旧的 Windows checkout 行尾问题已解决；第三轮当时剩余的 CI 限制是解压后 GUI smoke 在 Actions 环境中报告缺少 OpenGL 2.0+，并非许可校验失败。第四轮现已上传并独立核验 ZIP；Windows 10/11 实机 GUI 验收仍待完成。

第三轮 Actions `42495076057` 于 `2026-11-08T15:38:12Z` 在 `windows-2022`（Microsoft Windows Server 2022，镜像 `win22/20261101.353`）结束。GUI/CLI 检查测试、CLI-only feature boundary 和两个 release 构建均为 success：GUI 主库 204、路由库 8、集成 7；CLI-only 主库 136、路由库 8、集成 7，均无失败。打包脚本完成 PE x64/版本/署名和依赖检查、许可收集、包内与 ZIP SHA 生成、解压校验；解压后的 CLI smoke 先通过。随后的真实 GUI smoke 启动进程以 code 1 退出，stderr 为 `GUI 启动失败: egui_glow: OpenGL: egui_glow requires opengl 2.0+.`；该 Runner 未提供程序所需的 OpenGL 2.0+ context，导致打包步骤 failure，`actions/upload-artifact@v4` skipped。ZIP 曾在 Runner 临时 `dist` 中生成，但没有上传，日志中也没有记录其 SHA；不能下载或作为已交付产物验收。证据为 `/Users/mac/Desktop/run fasting/verification/windows/run-42495076057-summary.json`、`-metadata.json`、`-full.log` 与 `-failed.log`。

第四轮 `42496483831` 使用源码提交 `36eeab7405f0e6f72d4faaa2fd73b509b274e84d`，于 `2026-11-08T15:57:23Z` 完成 success。GUI/CLI 检查测试仍为 204+8+7 和 136+8+7，CLI-only feature boundary、release builds、PE/依赖、262 包/533 许可收集、解压后正式 EXE 的离线 `--help` / `--version` / 未知命令 smoke、ZIP 与包内 checksum 均通过。GUI 进程未能建立 OpenGL 2.0+ context；工作流将这个精确错误记为 `unavailable_missing_opengl_2_context`，并在 `windows-build-evidence.json` 明示，不作为 GUI smoke 通过。其他 smoke 错误仍会失败。Artifact 上传成功，下载 artifact 后独立解析 PE、确认内嵌 commit 与包版本、计算 ZIP/EXE SHA 并核对 538 项内部 hash。下载目录为 `/Users/mac/Desktop/run fasting/artifacts/windows-run-42496483831`，详情见其中 `independent-artifact-verification.json`。Windows 10/11 图形桌面验收仍待实机。

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

## 证据记录

| 项目 | 本次证据 |
|---|---|
| Actions URL / Run ID / Commit | 第四轮 `36eeab7` / `42496483831` workflow success；此前三轮失败原因见上文 |
| MSVC check / test / build | 通过；GUI 204+8+7、CLI-only 136+8+7 全通过；CLI-only boundary 与 GUI/CLI release build 通过 |
| EXE 文件、架构、版本、依赖、签名 | PE32+ x64，版本 `0.3.1`，ProductName `NekoSportsWorldTool`，署名 `prod.by 田宸宇 J Claude`，无动态 CRT imports，签名 `NotSigned` |
| ZIP 解压与解压后 CLI / GUI | 解压后 CLI smoke 通过；GUI smoke `unavailable_missing_opengl_2_context`，未标为通过；实机 GUI 待 Windows 10/11 |
| 下载 ZIP SHA-256 / EXE SHA-256 | `13cf827b1baa29d5e49c4ca1f3e0874b06f493f398cd2964fa4cf7fa0f348c5d` / `b35828baa6d834b61d4b4838994d62e8bc84c42b3e2680423a42ebd8ba6663f3` |
| 包内校验与内容 | 539 个文件，538 个内部 hash 通过，533 份许可；ZIP `17,168,296` bytes，EXE `28,460,032` bytes |
| 本地下载产物 | `/Users/mac/Desktop/run fasting/artifacts/windows-run-42496483831/NekoSportsWorldTool-J-claude-Windows-x64.zip`；artifact ID `13623878028`，上传容器 digest `269c0af6112dcf8d48f7013c8bcb29f77946bcd23fedf32e71023e2ae0d21198` |

Windows Runner 成功不能代替 Windows 10/11 的图形、DPI 和双击启动实机验收；项目详见 Windows_Acceptance_Checklist.md。

## 重新构建步骤

优先在现有 Fork 的 `Windows customized build` 工作流手动运行 `feature/j-claude-cross-platform`，确认本次执行所用 SHA，再下载同次 Actions artifact。普通工作流只有 `contents: read`，不会创建 Release。

在具备 MSVC、Windows SDK、Rust 1.96.1 和 Python 的 Windows 开发环境，也可在独立检出的最终源码目录执行与工作流一致的命令。打包输出目录必须为空，脚本发现同名 ZIP 会停止，不覆盖旧产物。

```powershell
function Invoke-CheckedCargo {
    & cargo @args
    if ($LASTEXITCODE -ne 0) { throw "cargo failed: $LASTEXITCODE" }
}
$buildTarget = 'x86_64-pc-windows-msvc'
$env:NSWT_BUILD_COMMIT = (& git rev-parse HEAD).Trim()
$env:NSWT_BUILD_TARGET = $buildTarget
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
Invoke-CheckedCargo check --locked --target $buildTarget
Invoke-CheckedCargo test --workspace --locked --target $buildTarget
Invoke-CheckedCargo clippy --all-targets --locked --target $buildTarget
Invoke-CheckedCargo check --locked --no-default-features --target $buildTarget
Invoke-CheckedCargo test --workspace --locked --no-default-features --target $buildTarget
Invoke-CheckedCargo build --release --locked --no-default-features --bin nekosportsworldtool --target $buildTarget --target-dir target-cli
Invoke-CheckedCargo build --release --locked --bin nekosportsworldtool --target $buildTarget
./scripts/package_windows.ps1 -OutputDirectory 'dist-rebuild'
```

每一步非零退出即停止，不能跳过失败步骤称为复现成功。`package_windows.ps1` 自行收集第三方许可、检查 PE/依赖/资源、白名单打包、计算双层校验、解压校验并调用实际 EXE 烟测。直接复制 Mac 程序或仅安装 Windows target 不构成以上原生构建。
