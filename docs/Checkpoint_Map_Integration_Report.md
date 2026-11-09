# 打卡点独立地图集成报告

日期：2026-11-09

工程版本：0.3.1

分支：`feature/j-claude-cross-platform`
基线 Commit：`3368325408f8f4f86c82810798752a81cfb94f75`

最终代码 Commit：`3d12bf3cfc355f6779c46b9476b73836618e74d9`

## 结论

本次将平台打卡点只读展示从路网规划结果中分离。无 OSM、HTTP 502、超时、空 OSM、缺少围栏，以及策略接口失败时，已存在且作用域匹配的合法缓存仍可在地图独立绘制。真实 Overpass 失败由 loopback HTTP 测试覆盖；未用账户凭据访问平台。

| 验收场景 | 实际结果 |
|---|---|
| Overpass HTTP 502 | 通过；保留缓存点位，路网层单独显示 502。 |
| Overpass 超时 | 通过；缓存点位保留，路网层报告请求超时。 |
| 空 OSM | 通过；缓存点位保留，路网层报告未找到可用道路。 |
| 无围栏 / 无道路 / 无路线预览 | 通过；地图数据层照常绘制打卡点。 |
| 接口数量、解析、有效坐标和地图层计数 | 离线混合数据测试验证 2 条原始行、1 个有效坐标、1 个无效坐标；有效数据显示计数与坐标列表一致。GUI 合成样本为 4/4，有 1 对坐标重合实体，仍保留 4 个实体。 |
| BD-09 与 WGS84 | 转换边界测试通过；详情同时显示原始 BD-09 与地图 WGS84 坐标。没有自动交换经纬度。 |
| Mac 离线 GUI | 通过；最终 release GUI 以 `NSWT_OFFLINE=1` 和独立 `NSWT_DATA_DIR=/Users/mac/Library/Caches/NekoSportsWorldTool-phase1-20261109-final` 启动。实际窗口显示 4 个合成缓存点、4 个视口内点位、3 个标记位置（含重合实体）、中文详情及独立道路/围栏/路线状态。重启后重新读取 4 点；未登录、未发送运动写入。 |
| Windows 构建 | 通过；GitHub Actions Run `42612266893`，提交 `3d12bf3cfc355f6779c46b9476b73836618e74d9`，Artifact `NekoSportsWorldTool-J-claude-Windows-x64`。Runner smoke 已完成 EXE 元数据、包内容、CLI 和校验检查；其 GUI smoke 因 runner 缺少 OpenGL 2 context 不可用，Windows 实机 GUI 验收仍待完成。 |
| 业务隔离 | 新增代码没有调用运动记录提交接口。未登录 GUI 的“开始跑步”控件不可用。没有真实账户提交或线上写入。 |

## 原有依赖与改造

原运行流程在学校路网 worker 中先获取围栏，再下载/读取道路和策略，最后构建 `PreviewInputs`。地图点位过去主要通过 `PreviewInputs` 或路线预览进入画布；路网或策略失败可能导致点位不可见。

新增独立 `CheckpointLayerState` 与 `BoundaryLayerState`。UI 直接消费点位、围栏、路网和路线数据各自的状态，不把道路错误作为点位错误。跑步页使用独立 WGS84 checkpoint canvas；路线预览保留原有 BD-09 canvas。OSM 页将独立点位和围栏绘制在道路图层之上。打卡点交互仅提供选择、详情、定位与适配范围，不修改平台数据。

平台点位刷新复用现有 `fetch_points_ext` 请求和 300 秒业务缓存。增加观测回调以区分接口原始行和严格解析结果；只有业务解析成功、且请求未取消时才在调用方提交缓存。缺少缓存的点位请求限为 300 秒内最多 3 次，不自动重试；忙时刷新被忽略。取消与缓存提交由同一锁串行化，迟到响应不能覆盖新缓存。缓存过期后只允许查看，并标记为过期，不延长业务 TTL。

路网刷新时平台缓存先独立发布。围栏结果发布后依据账号、学校、锚点和 run-area 校验缓存范围。真实围栏请求失败时，当前同账号/学校/锚点缓存保留并标注“尚未通过本次围栏响应核对；仅供查看”；区域明确不匹配时拒绝显示旧区域数据。没有围栏时不伪造围栏。没有有效缓存且围栏/区域服务失败时，在线点位接口不会用猜测的 area 请求。

## 数据模型与坐标

新增 [src/checkpoint.rs](../src/checkpoint.rs)：

- `CheckpointViewModel` 保存 UI 内部稳定 ID、可选原始平台 ID、可选原始名称、原始坐标及 CRS、转换后的地图坐标及 CRS、来源、缓存时间、校验结果和完整原始 JSON。
- 缺失名称只在界面回退为“未命名打卡点”；缺失 ID 显示“未提供”。两者均不会写回原始数据。
- `CheckpointStats` 统计原始、对象解析、有效坐标、异常、缺失元数据、重复平台 ID 与坐标重合数。相同坐标不等同于同一平台记录，不做坐标去重。
- `UserWaypoint` 为后续本地用户选点预留独立类型，与只读平台点位区分。

平台点位由 `api::fence::point_xy` 按既有逻辑读取 BD-09 `lat/lon`，当 BD 字段缺失或为零时回退 GCJ-02 `glat/glon` 并转换为 BD-09；再使用 `school_roads::to_wgs` 逆变换一次供 WGS84 地图显示。OSM 图与平台点位在 WGS84 画布相遇；围栏在绘制边界转换一次。旧路线输入和路线预览仍使用原坐标流程，不共享展示 ViewModel。测试覆盖多地理位置转换往返、轴顺序和地图投影平移/缩放。

地图统计区分有效实体数、视口内绘制实体数、屏幕标记位置数及视口外实体数。合成 GUI 夹具中 4 个有效实体全部在视口内，因其中两点重合，屏幕标记位置为 3 处；列表仍可分别选中重合点。

## 缓存与失败状态

新增 `PointsSnapshot` 仅读取当前数据根下的 `points_cache.json`，不读取账户 session、不执行 HTTP、不迁移其他用户目录。离线启动跳过旧目录自动迁移；只从显式隔离数据目录读取缓存。快照缺失、损坏、fresh、stale、加载中和请求错误有独立状态。业务缓存加载器仍遵守原有 300 秒 TTL，展示读取 stale 快照不会把它重新当成可供路线业务使用的新缓存。

HTTP 502、传输超时、空 OSM 分别由本机 loopback HTTP server 返回真实状态/响应或延迟触发；每种情形确认只产生一个 Overpass 请求。测试不访问公网 Overpass 或真实平台账户。

## 修改文件

- 新增：`src/checkpoint.rs`、`src/ui/checkpoint_jobs.rs`、`src/ui/checkpoints.rs`、`src/ui/checkpoint_integration_tests.rs`。
- 修改：`src/api/client.rs`（Windows loopback 测试 socket 设回阻塞模式）、`src/api/model.rs`、`src/api/points.rs`、`src/api/school_roads.rs`、`src/lib.rs`、`src/startup_data.rs`、`src/ui/jobs.rs`、`src/ui/layout_tests.rs`、`src/ui/map.rs`、`src/ui/mod.rs`、`src/ui/msgs.rs`、`src/ui/osm.rs`、`src/ui/route_refresh_tests.rs`、`src/ui/run.rs`。
- `.gitignore` 与三份现有调查文档为任务开始前已存在的用户修改，不属于本次提交。

## 验证记录

环境：macOS 27.2 build 26B123、Apple Silicon、Rust 1.96.1、Cargo 1.96.1。构建和测试使用 `CARGO_NET_OFFLINE=true`，没有 Cargo 外部依赖下载。

- 基线：GUI/CLI `cargo check`、`cargo test` 和 Clippy 成功；全仓 `cargo fmt --all -- --check` 已在无本次变更的 Android/route-planner 文件失败。
- 最终 GUI 默认 feature：`cargo check --locked` 通过；`cargo test --workspace --locked` 通过，根 crate 242、route-planner 单元 8、集成 7 项通过。
- CLI-only：`cargo check --locked --no-default-features` 通过；`cargo test --workspace --locked --no-default-features` 通过，根 crate 153、route-planner 单元 8、集成 7 项通过。现有 dead-code 警告保留。
- `cargo clippy --all-targets --locked` 通过。仓库存在既有 warning；本次新增的缓存辅助代码 Clippy 警告已修正。
- 全仓格式检查仍报告已存在格式差异，例如 `crates/route-planner/tests/basic.rs` 和 `src/android.rs`。对本次 Rust 文件单独运行 `rustfmt --edition 2021 --config skip_children=true`；`git diff --check` 通过。没有递归格式化无关 GUI 子模块。
- 本地 release GUI 构建通过。Mac GUI 通过 CUA 实际启动专用验证 bundle，检查离线状态、4 个合成点、中文元数据和坐标、缓存过期标识、道路/围栏/路线独立状态；重启后 4 个点重新加载。点选详情和“显示全部点位”实际操作通过，CUA 对最终窗口做了即时截图核验；目录内另留有此前导出的详情截图 `mac-checkpoints-details.png`（其实际编码为 JPEG）及最终日志。首次验证 bundle 启动出现初始化超时；改用新的独立用户缓存目录和验证 bundle 后，最终 GUI 初始化及验收成功。没有将失败启动记为通过。
- Windows Actions 首次运行 `42611705497` 暴露既有 loopback 认证测试在 Windows 接受 socket 上读到 `WouldBlock`；测试夹具显式把 accepted socket 设为阻塞并设置读写超时后，本地目标测试通过，完整 Actions Run `42612266893` 通过。
- 全部原始基线和终测日志位于 `/Users/mac/Desktop/run fasting/verification/checkpoint-map-phase1-20261109/`。

## Windows 与交付状态

仓库已有 `.github/workflows/windows-build.yml`，目标 `x86_64-pc-windows-msvc`。Run `42612266893` 已对最终代码完成 GUI/CLI 检查与测试、Clippy、release 构建、ZIP 打包和 CLI/包 smoke。提交：`3d12bf3cfc355f6779c46b9476b73836618e74d9`。Artifact ID：`13670678492`。下载文件：`/Users/mac/Desktop/run fasting/verification/checkpoint-map-phase1-20261109/windows-actions-42612266893/NekoSportsWorldTool-J-claude-Windows-x64.zip`。ZIP SHA-256：`bd55599fe883093809dcfadd37eea633c76ec9a6e38204cc7c8ced44542d2b02`；ZIP 内 `SHA256SUMS.txt` 与独立重算结果一致，包内包含 `NekoSportsWorldTool.exe`、README、LICENSE、THIRD-PARTY-NOTICES 及 539 个文件。Windows Runner 未提供 OpenGL 2 context，故 GUI smoke 明确记为不可用，仍需 Windows 10/11 实机验证显示、缩放、点选与中文字体。

没有创建公开 Release。没有触发运动记录提交、真实账户登录或账户数据写入。验收目录及完整日志位于 `/Users/mac/Desktop/run fasting/verification/checkpoint-map-phase1-20261109/`。

## 已知限制与后续

- 平台返回的完整原始 JSON 可能含额外元数据；详情默认折叠显示。合法坐标行即使名称或 ID 缺失仍显示。
- 缓存文件只记录一个当前点位作用域；换账号、学校、锚点或明确校区时拒绝保留旧数据。缓存未通过当前围栏区域复核时会标出仅供查看。
- 点位查看不等于有可用底图：本阶段使用现有离线矢量图层，若没有 OSM/围栏，只有点位和中性画布。
- 下一阶段可在同一 WGS84 MapView 上引入独立 `UserWaypoint` 编辑图层与本地草稿，但应继续与平台点位只读层隔离；本阶段没有实现选点写入或路线业务改动。
