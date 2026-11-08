# Mac Deployment Report

## 环境与部署

| 项目 | 已检测值 |
|---|---|
| macOS | 27.2，Build 26B123 |
| CPU | arm64 / Apple Silicon |
| Rust / Cargo | 1.96.1 |
| Git | 2.54.0 (Apple Git-157) |
| Xcode 命令行工具 | `/Library/Developer/CommandLineTools`，SDK 27.2 |
| 工程 | `/Users/mac/Developer/NekoSportsWorldTool` |
| 自有 Fork | `1373878757-ux/NekoSportsWorldTool` |
| 开发分支 | `feature/j-claude-cross-platform` |
| 原始提交 | `e229abdbab71e377b86632e588e607413af4b93c` |
| 已推送实施提交 / 版本 | `17a7a68b92dbf216fd81c21055f46fbe9b3672b2` / `0.3.1` |
| 第二轮小修复提交 | `f3b91f287aa7c928bc22f24cd9b6a2a0a677e9f1` |
| 最终字节保全修复提交 | `0b1ff9ec1ddee434bf80b866189146764bd8c381`，已推送同一开发分支 |
| Windows smoke 状态修复提交 | `36eeab7405f0e6f72d4faaa2fd73b509b274e84d`，记录标准 Runner OpenGL context 限制 |
| 当前 UI 提交 / Windows 构建提交 | `a4c5dcf4d2953f3f556a9911f735b8653cb4e825`，Windows Run `42509136365` 已成功上传 artifact |
| 当前离线 Mac 验证包 | `/Users/mac/Desktop/run fasting/verification/gui/NekoSportsWorldTool.app`；GUI 二进制 SHA-256 `a1e99e45d8e0108483a09a68dc4d026d2b4215bcd54512fd961cb29f78de178f` |

源码已克隆；修改功能前完成原始源码编译基线，未重置已有其他远端分支。日志根为 `/Users/mac/Desktop/run fasting/verification/baseline`。

## 原始源码基线：已执行

| 命令/检查 | 结果 |
|---|---|
| `cargo check --locked` | 通过，退出 0 |
| `cargo build --release --locked --bin nekosportsworldtool` | 通过，退出 0 |
| `cargo test --workspace --locked` | 通过：主库 182、路由库 8、集成 7 |
| `cargo check --locked --no-default-features` | 通过，退出 0 |
| `cargo test --workspace --locked --no-default-features` | 通过：主库 121、路由库 8、集成 7 |
| CLI-only release build | 通过，退出 0 |
| `cargo fmt --all -- --check` | 失败，退出 1；存在原有格式差异，不作为功能失败或伪称通过 |
| `cargo clippy --all-targets --locked` | 退出 0，有原有告警；未用 `-D warnings` 宣称无告警 |
| 原始 CLI `--help` | 退出 0，不打开 GUI |
| 原始 CLI `--version` / 未知命令 | 均退出 1；原版本不支持 `--version` |
| `file` 检查 | Mach-O arm64 |

原始 GUI 未在可能自动联网的模式下执行功能测试；实际 GUI 测试要求先加入离线保护与隔离数据目录。

## 修改后验证

2026-11-08 前一轮源码 `0b1ff9e` 的本地自动化日志已实际生成，位置为 `/Users/mac/Desktop/run fasting/verification/local/source-0b1ff9e`；`results.json` 的七项 locked 检查、测试、构建及 clippy 均退出 0。当前 UI 提交为 `a4c5dcf4d295`，其 Windows Actions 也已完成；格式检查另行记录于上级 `local` 目录，失败不含在通过结论中。真实 GUI 单独验收，不以编译和自动化绘制测试替代桌面窗口。

| 命令/检查 | 结果与证据 |
|---|---|
| `cargo check --locked` | 退出 0，`gui-check.log` |
| `cargo build --release --locked --bin nekosportsworldtool` | 退出 0，`gui-release.log` |
| `cargo test --workspace --locked` | 主库 204、路由库 8、集成 7 全通过，`gui-tests.log` |
| `cargo check --locked --no-default-features` | 退出 0，`cli-check.log` |
| `cargo test --workspace --locked --no-default-features` | 主库 136、路由库 8、集成 7 全通过，`cli-tests.log` |
| `cargo build --release --locked --no-default-features --bin nekosportsworldtool --target-dir target-cli` | 退出 0，`cli-release.log`；独立目标目录避免覆盖 GUI 程序 |
| `cargo clippy --all-targets --locked` | 退出 0，`clippy.log`；lib 15、lib-test 19（含 15 重复）、路由库 4，与原基线告警数量一致，新增告警已消除；不宣称 `-D warnings` 通过 |
| `cargo fmt --all -- --check` | 退出 1，`local/fmt.log`；原基线 39 个失败文件，当前 27 个，未新增失败文件，12 个已修；见 `local/fmt-comparison.json`，未全库重排 |
| `python3 scripts/verify_local.py --binary target-cli/release/nekosportsworldtool --output <独立证据目录>` | 30 项通过，`cli-cli-smoke/cli-smoke.json`；只使用临时文件和 loopback Mock |
| `python3 scripts/verify_local.py --binary target/release/nekosportsworldtool --output <独立证据目录>` | GUI 完整构建的 CLI 入口同样 30 项通过，`gui-cli-smoke/cli-smoke.json`；带参数不启动 GUI |

自动化覆盖精确署名常量与 GUI 禁止重复硬编码、状态栏 `Powered by J Claude` 精确文案、中文字体字形、3500 ms 启动最短展示/400 ms 淡出/失败/超时/重试/交互隔离、小窗口及 HiDPI 绘制、损坏配置保留、迁移原字节与上传回执保全、并发首次身份一致性、离线 HTTP 拒绝和定制更新源筛选。CLI 的 help/version/未知命令、本地 GPX/JSON、中文空格路径、离线命令阻止和更新确认/错误分支均有真实子进程记录；未创建账户数据、下载更新或替换可执行文件。

## 真实 Mac GUI：已执行部分

真实桌面测试使用独立的本地开发 `.app` 包承载实际编译的 GUI 二进制，进程记录显式设置 `NSWT_OFFLINE=1` 与独立 `NSWT_DATA_DIR`，成功测试的状态位于 `verification/gui/bundle-first-launch/isolated-data`。这不是经过 Developer ID 签名或公证的 macOS 正式发行包。

总体记录为 `verification/gui/gui-acceptance.json`。本轮实际观测的 `17a7a68` GUI 二进制 SHA-256：`6ead97c758c05686f7c3400fab8de22f4381f8934aedf41598e4faf49e30d3d7`。为了抓取真实异步启动页，仅在隔离目录用 FIFO 延迟读取合成配置，随后输入有效 `{}` 并还原普通 JSON 文件；截图不是额外绘制的展示页，最短展示/淡出时间由确定性单元测试另行验证。

| 实际操作 / 观察 | 结果与证据 |
|---|---|
| 慢初始化启动页 | 中央 `just callme...`、底部 `prod.by 田宸宇 J Claude`、窗口标题正确，`gui/splash-slow-initialization.jpg` |
| 启动动画增量（当前源码） | 3500 ms 最短展示；标题 0–1200 ms，线条 1200–2000 ms，流动高光 2000 ms 起，署名 2500–3200 ms，成功淡出 400 ms；`src/ui/startup.rs` 时间轴测试 |
| 状态栏定制文案（当前源码） | 右侧显示 `Powered by J Claude`；About 仍显示原作者、来源、许可证和定制说明；`src/presentation.rs`、`src/ui/layout_tests.rs` |
| 主窗口与中文 | 深色主窗口、离线提示、中文标签正常，`gui/main-offline.jpg` |
| About | 两行署名、原作者、来源、非商业许可、字体说明与独立数据目录可见，`gui/about-offline.jpg` |
| 页面与输入操作 | 九页均实际切换；键盘输入合成文本 `offline-test`；本地“换一版”操作已执行，未登录真实账户 |
| 本地 OSM 导入 | 实际读取合成离线 OSM：4 节点、8 有向边、1 建筑，`gui/local-map-loaded.jpg`；不构成真实学校路线或服务端规则验收 |
| 设置保存 | About 中将启动更新设为关闭，实际保存至隔离配置；不执行更新下载或替换 |
| 正常退出 | 使用 Cmd+Q，`gui/bundle-first-launch/exit.json` 记录退出 0 |
| 提交后 About 身份 | `gui/about-committed.jpg` 显示版本 `0.3.1`、构建 `17a7a68b92db`、macos/aarch64 与原项目归属 |
| 退出后重启 | `gui/restart/exit.json` 记录退出 0 且 `same_identity=true`，同一隔离目录的设备身份保持不变 |
| 窗口大小变化 | 已从 1760×1504 截图像素的窗口调整至 3024×1774，`gui/resized-main.jpg` 和 `about-committed.jpg` 中控件、导航与署名正常；不等价于 Windows 系统 DPI 矩阵 |
| 损坏文件错误与重试 | 构造合成损坏配置，实际显示错误、重试和退出；原字节保留、不创建替代身份、不显示私有测试标记。修复合成文件后点击重试进入主界面，最后退出 0；见 `gui/error-retry` 的截图、AX 文本、`corrupt-file-evidence.json` 与 `exit.json` |
| 空闲进程观察 | 20 秒区间三次 `ps` 样本 CPU 均为 0.0%，累计 CPU 时间均为 0:00.63，RSS 约 89 MB；同时 `lsof` 未发现该 PID 的互联网 socket，`gui/idle-network-evidence.json` |
| 最终二进制新一轮启动 | 2026-11-09 进程以 `NSWT_OFFLINE=1` 和 `/tmp/nswt-final-gui-20261109` 启动；CUA 确认主窗口、原窗口标题、离线提示及未登录状态。观察记录为 `gui/final-current-run.json`；本次未重新捕获 splash 截图 |

初次截图来自功能修改完成但尚未提交时的构建，About 内嵌构建号显示原 HEAD `e229abdbab71`；不得将该截图当作提交版本的内嵌构建号证明。实施提交后 release 重建日志为 `gui-release-committed.log`，`17a7a68b92db` 构建号由 `about-committed.jpg` 单独证明。截图像素尺寸是测试截图记录，不据此推断显示器物理分辨率或系统缩放百分比。

最终源码 `0b1ff9e` 相对实际观测的 `17a7a68` 仅有 Windows 打包 API-set 识别、一个逻辑等价的 GUI 启用条件、测试格式和固定许可 Git 字节属性/工作流触发修正；上述真实 GUI 证据保留原出处，不改写成最终提交截图。最终源码的七项 Mac 自动化已全 0，两套 CLI 脚本各 30 项通过，元数据见 `local/source-0b1ff9e/results.json` 与 `binaries.json`。此前 `f3b91f2` 的验证日志单独保留，未覆盖。

Windows Actions 第五轮 Run `42499555243` 在正确标记 GUI smoke unavailable 后，因 Windows 仍占用 `gui-1.stderr.txt` 而清理失败，artifact 未上传；第六轮 Run `42500313727` 的产物作为历史修正记录保留。最终 Run `42509136365` 成功构建并上传：EXE SHA-256 为 `02f41aeb49578b0f51fbee038dda280bde4f3dd921bf8da119c1480f4dad7311`，539 个文件中的 538 项内部校验及 PE x64、版本、内嵌 commit、533 份许可均验证通过。工作流仍将 GUI smoke 标记为 `unavailable_missing_opengl_2_context`，不是通过。ZIP SHA-256 为 `62ea2f2c0d58639d72ba847b63ed90f78ba9ac84dcb109a4f77f61103ad5b25d`，本地文件为 `/Users/mac/Desktop/run fasting/artifacts/windows-run-42509136365/NekoSportsWorldTool-J-claude-Windows-x64.zip`。此项与本机 Mac 离线 GUI 验收分开；Windows 10/11 的实际图形与 DPI 测试仍待实机完成。

| Mac 验证二进制 | 版本 / SHA-256 |
|---|---|
| 当前 GUI（同时提供 CLI 入口） | `NekoSportsWorldTool 0.3.1 (a4c5dcf4d295; aarch64-apple-darwin)`；`a1e99e45d8e0108483a09a68dc4d026d2b4215bcd54512fd961cb29f78de178f` |
| 历史 CLI-only 证据 | 前一轮 `0b1ff9e` 构建；`f87222ebb470955ece92e0352dde277748b86ad2142e22d3bf587a25ec0b6907` |

安全离线开发 `.app` 位于 `/Users/mac/Desktop/run fasting/verification/gui/NekoSportsWorldTool.app`。2026-11-09 对当前 GUI 二进制实际启动并观察主窗口：版本 `0.3.1 (a4c5dcf4d295; aarch64-apple-darwin)`，SHA-256 `a1e99e45d8e0108483a09a68dc4d026d2b4215bcd54512fd961cb29f78de178f`；`NSWT_OFFLINE=1`、隔离数据目录，CUA 确认原窗口标题、离线横幅、启动页、完整署名和状态栏文案。启动页完整阶段的历史截图和当前源码时间轴测试均已保留。该 `.app` 是本地验证工具，不是签名/公证发行包。

空闲 CPU 和 socket 观察仅说明采样区间内的进程表现，不是历史网络流量审计，也不替代离线传输策略及 Mock 测试。不对所有机器的 CPU、显卡或内存占用作保证。

实际 Mac 字体选择显示 `STHeiti Light.ttc`，完整中文署名渲染正常；嵌入 Noto Sans SC 回退的字形覆盖另由自动化测试验证，不将 Mac 的系统字体截图当作 Windows 字体实机证据。

## 测试工具异常与纠正

首次直接启动裸二进制后，CUA 无法绑定非 `.app` 程序，因此由测试器发送 SIGTERM 终止；这次不是成功的 GUI 退出验收。随后把同 SHA 的实际二进制放入本地开发 `.app` 包，完成上述真实观测和 Cmd+Q 退出。

另一次 CUA 在程序退出后调用 `getAXState` 时自动重启应用，且未继承子进程的离线/隔离环境。发现后立即停止，并把测试启动器改为每次调用强制设置离线模式和独立数据目录。该次不能保证没有发生匿名 IP/更新查询，因此本报告不声称所有启动均从未联网。记录见 `gui/harness-note.json`。默认目录仅作文件名检查，只有新生成的 `identity.json`，没有真实 `config.json` / `session.json`；未使用真实账号登录、执行运动记录提交或其他线上账户写入。后续成功验证均须以进程环境记录为准。

收尾等价布尔表达式修正后的第一轮复验，测试启动器误把 `NSWT_OFFLINE=1` 施加到整套单元测试，导致需要 loopback Mock 或特定输入状态的既有用例被离线策略拦截：主库 186 通过、18 失败。异常记录为 `local/gui-tests-correction-offline-harness.log` 和 `correction-offline-harness-results.json`，不隐去这次失败，也不据此关闭产品的离线保护。移除标准单元测试进程的全局离线变量、保留隔离数据目录后，标准 cargo test 与 clippy 均已重新通过，见 `correction-results.json`。此轮真实 GUI 使用单独的离线测试启动器，不沿用标准单元测试的环境配置。

## 数据与安全边界

默认用户数据根为 `~/Library/Application Support/NekoSportsWorldTool-J-claude`；已记录的离线 GUI 测试使用 `NSWT_OFFLINE=1` 和独立 `NSWT_DATA_DIR`，上述工具异常单独披露。不得为验收回读实际账号目录或执行运动记录提交、补签。完整日志须脱敏，不收录配置内容或 Token。
