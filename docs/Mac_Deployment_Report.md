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

2026-11-08 本地修改后自动化日志已实际生成，位置为 `/Users/mac/Desktop/run fasting/verification/local`。真实 GUI 单独验收，不以编译和自动化绘制测试替代桌面窗口。

| 命令/检查 | 结果与证据 |
|---|---|
| `cargo check --locked` | 退出 0，`gui-check.log` |
| `cargo build --release --locked --bin nekosportsworldtool` | 退出 0，`gui-release.log` |
| `cargo test --workspace --locked` | 主库 204、路由库 8、集成 7 全通过，`gui-tests.log` |
| `cargo check --locked --no-default-features` | 退出 0，`cli-check.log` |
| `cargo test --workspace --locked --no-default-features` | 主库 136、路由库 8、集成 7 全通过，`cli-tests.log` |
| `cargo build --release --locked --no-default-features --bin nekosportsworldtool --target-dir target-cli` | 退出 0，`cli-release.log`；独立目标目录避免覆盖 GUI 程序 |
| `cargo clippy --all-targets --locked` | 退出 0，仍有告警，`clippy.log`；不宣称 `-D warnings` 通过 |
| `cargo fmt --all -- --check` | 退出 1，`fmt.log`；保留原始已有格式问题，不全库重排 |
| `python3 scripts/verify_local.py --binary target-cli/release/nekosportsworldtool --output <独立证据目录>` | 30 项通过，`cli-smoke/cli-smoke.json`；只使用临时文件和 loopback Mock |

自动化覆盖精确署名常量与 GUI 禁止重复硬编码、中文字体字形、启动最短展示/失败/超时/重试/交互隔离、小窗口及 HiDPI 绘制、损坏配置保留、迁移原字节与上传回执保全、并发首次身份一致性、离线 HTTP 拒绝和定制更新源筛选。CLI 的 help/version/未知命令、本地 GPX/JSON、中文空格路径、离线命令阻止和更新确认/错误分支均有真实子进程记录；未创建账户数据、下载更新或替换可执行文件。

真实 GUI 的窗口、页面/输入操作、缩放、退出重启和空闲状态：待本次实际桌面证据回填。

## 数据与安全边界

默认用户数据根为 `~/Library/Application Support/NekoSportsWorldTool-J-claude`；真实 GUI 使用 `NSWT_OFFLINE=1` 和独立 `NSWT_DATA_DIR`。不得回读实际账号目录，不自动执行运动记录提交或补签。完整日志须脱敏，不收录配置内容或 Token。
