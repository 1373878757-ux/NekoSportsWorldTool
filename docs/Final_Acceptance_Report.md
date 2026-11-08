# Final Acceptance Report

## 状态说明

2026-11-09：原始基线、功能实施、Mac 自动化与离线真实 GUI 验证已完成；源码已推送现有 Fork 开发分支。Windows 第六次 Actions Run `42500313727` 成功构建、上传并独立核验 ZIP。Runner 的 GUI smoke 因缺少 OpenGL 2.0+ context 记录为 unavailable，不算 GUI 通过；Windows 10/11 实机显示仍待验。六次 Actions 的历次结果与修正历史、下载包哈希均记录在本报告和 Windows Build Report。

- 工程：`/Users/mac/Developer/NekoSportsWorldTool`
- 版本：`0.3.1`
- 开发分支：`feature/j-claude-cross-platform`
- 实施提交：`17a7a68b92dbf216fd81c21055f46fbe9b3672b2`
- 第二轮源码修正提交：`f3b91f287aa7c928bc22f24cd9b6a2a0a677e9f1`
- 最终字节保全修复提交：`0b1ff9ec1ddee434bf80b866189146764bd8c381`
- Windows smoke 状态分类提交：`36eeab7405f0e6f72d4faaa2fd73b509b274e84d`
- Windows ZIP 源码提交：`4965f6c25a0e606010ab7fbfdebf9a68892d0a89`
- Windows 首轮 Actions：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42492957332>（打包步骤 failure）
- Windows 第二轮 Actions：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42494146681>（许可源字节完整性步骤 failure）
- Windows 第三轮 Actions：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42495076057>（failure：GUI smoke 报 OpenGL 2.0+ 不可用；artifact upload skipped）
- Windows 第四轮 Actions：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42496483831>（success；GUI smoke 明确记为 unavailable，artifact 已上传）
- Windows 第五轮 Actions：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42499555243>（failure：smoke log cleanup 文件仍被占用；artifact 未上传）
- Windows 第六轮 Actions：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42500313727>（success；GUI smoke 明确记为 unavailable）
- Windows ZIP：[NekoSportsWorldTool-J-claude-Windows-x64.zip](</Users/mac/Desktop/run fasting/artifacts/windows-run-42500313727/NekoSportsWorldTool-J-claude-Windows-x64/NekoSportsWorldTool-J-claude-Windows-x64.zip>)；SHA-256 `e98282e8c4ac456e3ea6e287b65114047de1b03849e1fe7a72c9c80f0e5e7817`
- Windows EXE SHA-256：`422fa5f4204f3a759544c287814e85a42d4b24627348ca3466eaebc244162512`

## 已完成并有证据

- 环境检查、源码克隆和独立开发分支。
- 修改前原始源码基线：Mac GUI/CLI locked check、release build、workspace tests 通过。
- 原始 `cargo fmt --all -- --check` 存在遗留格式差异；clippy 退出 0 但有遗留告警，已分别记录。
- 原始 CLI 帮助退出 0；原 `--version` 不支持，退出 1。Mach-O 架构为 arm64。
- 最终源码 `0b1ff9e` 的七项 Mac locked 检查、GUI/CLI release 构建与 clippy 均退出 0；GUI 主库 204、CLI 主库 136，均另有路由库 8 与集成 7 全通过。
- 格式检查仍退出 1，但失败文件由基线 39 个减为 27 个，无新增失败文件；`local/fmt-comparison.json` 保留比较依据，不宣称全库格式已通过。
- 收尾小修复后的标准 cargo test 再次通过 204+8+7，clippy 退出 0，告警数量回到原始基线（lib 15、lib-test 19、路由库 4），新增告警已消除。
- 最终源码的 GUI 完整构建与 CLI-only 二进制分别通过 30 项 CLI/离线/loopback Mock 验证，未创建账户状态、下载更新或替换程序；两种 `--version` 均包含 `0.3.1 (0b1ff9ec1dde; aarch64-apple-darwin)`。
- 第一轮真实 Mac GUI 已观察启动页、完整中文署名、原名称窗口标题、About 归属、九页切换、输入、本地合成 OSM 导入和 Cmd+Q 退出 0。
- 提交后 Mac GUI 重建的 About 实际显示 `17a7a68b92db`；重启身份保全、窗口调整、损坏合成文件保留与修复后重试、正常退出均已验证，并记录了 20 秒空闲 CPU/socket 观察。
- 2026-11-09 再次通过 CUA 检查最终 GUI 二进制的实际运行窗口：标题为 `NekoSportsWorldTool`，离线提示可见、账号未登录；进程环境为 `NSWT_OFFLINE=1` 和 `/tmp/nswt-final-gui-20261109`。详情见 `gui/final-current-run.json`。本次检查观察主窗口，没有重新采集启动页截图。
- Windows 目标第三方许可收集实跑通过：262 包、533 份许可/来源文件，连同索引共 534 个文件。

证据根：`/Users/mac/Desktop/run fasting/verification`。

- `baseline/`：原始源码 `results.json`、命令日志与原始 CLI 记录。
- `local/source-0b1ff9e/`：最终源码 `results.json`、`binaries.json`、locked 命令日志与 GUI/CLI 两套 `cli-smoke.json`；上级 `local/` 仍保留早期验证、许可、格式比较、提交扫描和异常日志。
- `gui/`：`gui-acceptance.json`、实际截图、进程/退出、损坏文件、空闲 CPU/socket 与工具异常记录。
- `windows/`：Actions 监控与证据。前三轮历史日志、第四轮至第六轮结果及 Run `42500313727` 完整日志均保留；Run `42499555243` 的清理失败日志也保留。最新 artifact 独立下载及解析证据位于 `/Users/mac/Desktop/run fasting/artifacts/windows-run-42500313727/NekoSportsWorldTool-J-claude-Windows-x64/independent-artifact-verification.json`。
- `changed-files.md` / `.json`：相对原始基准的 86 个实际变更路径，包括源码、资源、许可文本与报告。

## 修改后完成矩阵

| 要求 | 状态 | 所需证据 |
|---|---|---|
| 统一展示常量与精确匹配 | 自动化通过 | `src/presentation.rs` 精确测试与禁止 GUI 重复硬编码测试 |
| 单窗口启动页与失败恢复 | 自动化与真实观测通过 | 六项启动状态/布局测试、启动截图、真实合成损坏配置错误/修复/重试；不以固定时间掩盖失败 |
| 完整署名中文与 About | Mac 第一轮真实观测通过 | `gui/splash-slow-initialization.jpg`、`about-offline.jpg`、字体字形测试 |
| 主窗口保持原项目名称 | 通过 | 入口常量、精确测试与实际窗口标题 |
| 深色主题与地图一致性 | Mac 已观察 | 主界面、九页、本地 4 节点/8 边/1 建筑合成 OSM、两种实际窗口尺寸；Windows DPI 仍待实机 |
| 系统用户目录、保留式迁移 | 自动化与隔离重启通过 | 损坏数据/白名单迁移/原字节/并发首次身份测试、隔离进程记录、重启 `same_identity=true` |
| 离线模式与真实 GUI 测试 | 已执行，含测试器环境异常披露 | 成功测试显式隔离/离线；一次工具自动重启缺少环境已立即停止，详见下文 |
| 默认 GUI / CLI-only 回归 | 最终源码通过 | `local/source-0b1ff9e/results.json` 七项全 0；GUI204+8+7、CLI136+8+7；每个二进制30项脚本 |
| Fork 分支推送和 Windows Actions | 已推送；Run 1-3 修复 API-set、许可 SHA 行尾与 OpenGL 状态分类；Run 4 上传成功，Run 5 因临时日志占用清理失败，Run 6 上传成功；GUI smoke unavailable | `42492957332` / `42494146681` / `42495076057` / `42496483831` / `42499555243` / `42500313727` |
| Windows EXE、ZIP、SHA 与下载 | 最新 artifact 已下载；PE、538 项内部 hash、ZIP/EXE SHA 独立核验；未签名 | ZIP SHA `e98282e8…e5e7817`；EXE SHA `422fa5f4…44162512`；GUI smoke unavailable，Win10/11 实机待验 |
| Windows 10/11 实机 | 待实机验证 | 当前交付物在两系统实际验收 |
| 真实账户运动提交/补签 | 未执行，范围外 | 不得为完成测试清单执行 |
| 公开 Release | 未创建，已实查为空 | Fork releases API 返回 `[]`；`authorization-boundaries.json` 记录 release_count=0、任务未创建标签 |

## 真实 GUI 证据与诚实边界

实际进程与退出证据位于 `verification/gui/bundle-first-launch`，正常 Cmd+Q 退出码为 0。截图目录为 `verification/gui`；原始工作区构建的 About 显示 `e229abdbab71`，是修改尚未提交时的内嵌 HEAD。实施提交后重建 release，实拍 `about-committed.jpg` 显示 `17a7a68b92db`；`restart/exit.json` 证明正常退出 0 且身份一致。`error-retry` 证明损坏合成配置不被覆盖、标记不泄漏、修复后重试成功。

源码提交 `36eeab7` 在 `0b1ff9e` 基础上只调整 Windows Actions 对标准 Runner 缺失 OpenGL 2.0+ context 的精确状态记录；这项情况记为 GUI smoke unavailable，其余 smoke/打包失败仍阻断上传。后续 `5dc3db7`、`6d2b258`、`4965f6c` 不改 Mac GUI 绘制或展示常量；`5dc3db7` 只改 Android 标题、源码注释及重复文案测试。Mac 启动页和中文署名截图仍属于 `17a7a68`。2026-11-09 对最终 Mac GUI 二进制进行了新一轮离线隔离 GUI 检查，实际打开主窗口，证据见 `gui/final-current-run.json`；本次未重新采集 splash 截图。`0b1ff9e` 的 Mac 七项自动化全 0、两种二进制各 30 项脚本通过，构建元数据已登记于 `local/source-0b1ff9e/results.json` 与 `binaries.json`。最终 GUI SHA-256 为 `7bc76b9df5060fdbb6fa681a65c595a024628c0d9b9134cd7a4a8418c8dd821f`；CLI-only 为 `f87222ebb470955ece92e0352dde277748b86ad2142e22d3bf587a25ec0b6907`。

最终离线开发 `.app` 位于 `/Users/mac/Desktop/run fasting/verification/gui/NekoSportsWorldTool.app`。2026-11-09 的真实 GUI 进程使用其中最终 GUI 二进制，`--version` 为 `0.3.1 (0b1ff9ec1dde; aarch64-apple-darwin)`、SHA-256 为 `7bc76b9df5060fdbb6fa681a65c595a024628c0d9b9134cd7a4a8418c8dd821f`；进程环境明确为 `NSWT_OFFLINE=1` 与 `/tmp/nswt-final-gui-20261109`。CUA 观察到项目标题、离线提示与未登录状态，隔离目录中仅有 `identity.json`。本轮启动页没有截图；品牌启动页截图仍引用早前 `17a7a68` 的真实观测，后续提交没有修改 Mac GUI 显示逻辑。测试结束后已将开发包装器的默认隔离目录恢复为 `gui/bundle-first-launch/isolated-data`；仍可通过显式 `NSWT_DATA_DIR` 指向新的临时隔离目录。该 `.app` 不是签名/公证发行包。

`idle-network-evidence.json` 记录 20 秒区间三次 CPU 0.0%、累计 CPU 时间不变，RSS 约 89 MB；`lsof` 未观察到该进程的互联网 socket。这是采样观察，不代表对全部运行历史没有网络行为的证明。`resized-main.jpg` 与 `about-committed.jpg` 记录实际窗口调整，不取代 Windows 100%/125%/150% DPI 验收。

首次裸二进制无法供 CUA 绑定，测试器以 SIGTERM 停止，再使用同 SHA 二进制的本地开发 `.app` 包完成观测。另一次 CUA 在退出后自动重启程序，未继承离线和隔离环境；发现后立即停止，启动器现已强制每次设置这些环境。这一次不能保证没有匿名 IP/更新查询，故不声称“所有启动从未联网”。默认目录仅检查文件名，只有新生成的 `identity.json`，未见真实 `config.json` / `session.json`；未使用真实账号、提交运动记录或执行其他线上账户写入。记录为 `verification/gui/harness-note.json` 与 `gui-acceptance.json`。

当前开发 `.app` 为本地验证工具，不是已签名/公证的 macOS 公开发行包。Windows Actions 编译成功也不能取代 Windows 10/11 的双击启动、图形、DPI、干净系统和安全软件提示实机验证。

授权边界记录为 `verification/authorization-boundaries.json`：2026-11-08 实查 Fork 发行列表为空，本任务未创建公开 Release 或标签，未执行真实账户登录和业务写入。该记录不把已披露的匿名联网可能性写成“绝对未联网”。

首轮 Windows Actions 的 GUI/CLI check、test、release 与 clippy 已通过；打包脚本把 API-set 导入项视为应存在的物理 DLL，依赖检查失败，尚无上传 artifact 或解压后 GUI 烟测结果。完整失败日志已保留，不以首次编译成功代替发行包完成。

第二轮 Windows Actions 已通过 API-set 依赖检查，但固定许可源在 Windows Git checkout 时被自动转换行尾，导致严格 SHA 校验失败；Git `core.autocrlf=true` 已精准复现，原始 blob 与 manifest 一致。已对固定许可源禁用文本转换，真实 Git checkout 前后验证 30 份许可原文全部精确匹配 SHA。第三轮验证了这项修复：262 个包、533 份许可文件收集通过，release 构建、PE/依赖检查、ZIP 生成、包内 SHA 与解压校验、CLI smoke 均通过；随后 GUI smoke 的进程以退出码 1 结束，stderr 报 `egui_glow requires opengl 2.0+`，artifact 上传被跳过。第四轮 `42496483831` 使用源码提交 `36eeab7`，在标准 Windows Server 2022 Runner 对该精确 context 错误标记 GUI smoke `unavailable_missing_opengl_2_context`；其他构建、PE、依赖、许可、CLI smoke、ZIP 和 hash 校验通过，artifact 已上传并独立核验。第五轮 `42499555243` 的测试和构建通过，但 smoke stderr 文件仍被进程占用，清理失败且 artifact 未上传。提交 `4965f6c` 修正清理后，第六轮 `42500313727` 成功上传并独立核验 artifact。最新 ZIP SHA-256 为 `e98282e8c4ac456e3ea6e287b65114047de1b03849e1fe7a72c9c80f0e5e7817`，EXE SHA-256 为 `422fa5f4204f3a759544c287814e85a42d4b24627348ca3466eaebc244162512`。第六轮 GUI smoke 仍是 `unavailable_missing_opengl_2_context`，不能替代 Windows 10/11 实机验收。

收尾代码等价修正后的首次复验误将全局离线变量用于整套单元测试，使既有 loopback/界面输入用例遭策略拦截，出现主库 186 通过、18 失败。该次保留为 `local/gui-tests-correction-offline-harness.log` 与结果 JSON；标准单元测试清除该全局变量、保留隔离数据目录后已重新通过 204+8+7，clippy 退出 0，记录为 `correction-results.json`。此记录与实际离线 GUI 试验分开，不隐去验证过程失败，也不把测试器环境问题归为已经证明的功能回归。

## 交付和许可

工程仍为 NekoSportsWorldTool；启动中央文案为 `just callme...`，完整署名为 `prod.by 田宸宇 J Claude`。J Claude 不单独作为品牌展示。保留 YanamiNeko、ConCosew 来源、CC BY-NC 4.0 和实际第三方许可；用途为非商业使用与分享。

Windows ZIP 已从成功的第六轮 Actions Run `42500313727` 下载并独立核验：<https://github.com/1373878757-ux/NekoSportsWorldTool/actions/runs/42500313727>。本地文件为 `/Users/mac/Desktop/run fasting/artifacts/windows-run-42500313727/NekoSportsWorldTool-J-claude-Windows-x64/NekoSportsWorldTool-J-claude-Windows-x64.zip`，SHA-256 为 `e98282e8c4ac456e3ea6e287b65114047de1b03849e1fe7a72c9c80f0e5e7817`；CLI smoke、538 项包内校验和 ZIP/EXE 检查通过，GUI smoke 因 Runner 缺少 OpenGL 2.0+ context 记录为 unavailable，不算 GUI 通过。完整构建日志、独立校验结果与重建步骤见 `Windows_Build_Report.md`。剩余验收项为 Windows 10/11 实机 GUI 启动、显示、DPI 与干净系统验证。
