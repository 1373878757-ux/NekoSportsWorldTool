# 地图加载与设备定位配置诊断报告

日期：2026-11-09。版本：0.3.1。源码：用户 Fork `1373878757-ux/NekoSportsWorldTool`，分支 `feature/j-claude-cross-platform`。本轮基线：`44e377e91bed4dbc39f2e4b8ec2c709bc4360352`。

## 结论与证据边界

这是两个独立问题。HTTP 502 发生在公共 Overpass HTTP 请求阶段；“学校围栏内没有可用道路”发生在成功下载、解析、WGS84→BD-09 对齐之后的围栏裁剪阶段。一次失败的 HTTP 请求不会继续解析或裁剪。没有真实校区边界与对应成功 OSM 响应的配对样本，不能认定具体学校缺路、坐标转换有缺陷或裁剪算法错误。

设备页的编辑缓冲区和运行时身份分别是 `device_buf` 与 `identity`；原有实现已经在 `identity.json` 保存成功后同步运行时身份。没有发现“保存成功但运行时仍读旧值”的直接证据。前序诊断记录里的“哈尔滨市 + 38.901678, 121.540241”仍包含内置大连锚点，因此会被原有运行前检查拒绝；改城市名称不会自动移动坐标。

用户明确表示测试账号由本人负责，并要求停止 Mac 本地测试。本轮不代为登录、不发起真实账号跑步流程、不访问线上运动写入；Mac 只执行静态检查及构建。自动化测试由现有 Windows Actions 执行。正式 Mac GUI、真实校区定位和在线接口验证由用户完成，不能用编译结果代替。

## 实际源码与调用流程

- `src/ui/device.rs`：编辑缓冲区→点击保存→`api::model::save_identity`→保存成功后更新 `identity`。撤销修改从已保存运行时身份恢复。
- `src/api/model.rs`：`identity.json` 的读写复用原有持久化。默认数据根由 `src/platform.rs` 提供；本轮没有改动用户配置文件、缓存或会话。
- `src/crypto/header.rs`：锚点范围检查和旧大连默认配置拦截；本轮新增统一的问题说明，保留原来的布尔拦截条件。
- `src/api/flow.rs`：仍在原有实时点位请求位置检查定位配置，仅替换错误提示。登录、预检、点位请求、轨迹生成及提交协议的顺序和行为未调整。
- `src/api/school_roads.rs`：按当前围栏或合法点位确定范围→匹配学校及范围的 7 天缓存→单次匿名 Overpass 请求→OSM 解析→构图→WGS84/BD-09 对齐副本→围栏裁剪验证→接受缓存。返回给地图的原始图仍为 WGS84。
- `crates/route-planner/src/load.rs`：原有 highway 筛选、节点连边及建筑解析算法保留，只增加统计字段。道路筛选后的缺失节点引用数在整个 XML 解析完成后计算，避免 ways 先于 nodes 时误报。

macOS 与 Windows 共用上述路网代码，没有新增平台专属网络、坐标或裁剪分支。endpoint、45 秒 HTTP 总超时、Overpass 查询 timeout=25、24 MiB 响应上限、7 天路网 TTL、平台点位 300 秒 TTL均保持原值。没有下载或构建原作者仓库源码。

## 本轮最小修改

| 文件 | 修改内容 |
|---|---|
| `crates/route-planner/src/load.rs` | 新增 `OsmParseStats`：已解析唯一节点、way、原始 highway way、保留道路 way、未解析道路节点引用计数；不调整道路筛选或构图。 |
| `src/api/school_roads.rs` | 为 HTTP 请求生成进程内递增 request ID，记录状态码、耗时和响应字节；HTTP 错误正文最多读取 1024 bytes，日志摘要最多 240 个字符并清除控制字符。新增 parse/graph/align/clip/cache 阶段日志。 |
| `src/road_status.rs` | 分别展示 HTTP 502、OSM 解析失败、构图无道路、围栏裁剪无道路。502 正文包含 timeout 时仍按 HTTP 502 归类。 |
| `src/crypto/header.rs` | 定位问题集中生成明确提示：城市空、坐标无效、城市已改但锚点仍是大连默认值、城市仍为原默认值。 |
| `src/ui/device.rs` | 显示待保存、无效和当前保存配置状态；拒绝新修改的不合法定位数据。未修改定位、只修改机型等设备字段时仍允许保存，默认定位继续被运行前检查拒绝。 |
| `src/api/flow.rs` | 原位置使用统一错误原因，并提示到设备页修改后点击“保存”；不放宽校验、不自动交换经纬度。 |

原有 `.gitignore` 的三行修改、三份未跟踪调查文档均予保留，不纳入本轮源码提交。品牌展示常量、启动动画、账号模块、运动提交与身份请求字段不变。

## 如何判读新日志

| 阶段 | 输出 | 可以确认 / 不能据此认定 |
|---|---|---|
| `[road:http]` | request ID、HTTP 状态、elapsed_ms；失败正文摘要 | 502 表示 HTTP 服务或网关失败；连接/DNS/TLS/超时会保留底层错误。不能把 502 认作空道路。 |
| `[road:parse]` | bytes、nodes、ways、highway_ways、retained_road_ways、unresolved_road_node_refs | 区分原始没有 highway、被既有筛选去除、节点引用缺失、XML 错误。nodes 是成功解析且 ID 唯一的节点数。 |
| `[road:graph]` | crs=WGS84、nodes、edges | 构图成功与否。edge 为有向边，不能直接与 way 数比较。 |
| `[road:align]` | crs=BD-09、nodes、edges | 对齐副本已经完成。计数不变不等于真实校区坐标已经验收。 |
| `[road:clip]` | fence_rings、裁剪前后节点与边数 | 无围栏时明确记录 0 且不伪造边界；有围栏且归零时明确报裁剪阶段，根因仍需配对样本。 |
| `[road:cache]` | 当前作用域缓存命中或解析失败 | 有效缓存命中不请求 Overpass；失败下载及裁剪为空不写入新路网。 |

没有新增自动重试。每次 downloader 仍只有一次 `send_form`，缓存加载 fetch 仍是 `FnOnce`。UI 取消令牌不能立即中断已经开始的同步 HTTP；切换校区或独立刷新仍可能存在请求重叠。request ID 用于分辨这些请求，不宣称本轮修复了重叠调度。

## 定位配置限制

输入顺序仍是纬度、经度，内部不会自动交换或自动纠偏。本地合法范围校验不能证明一个城市名与一组坐标在地理上完全匹配，也不能识别所有数值合法的轴互换。只对可确定的旧大连默认锚点给出具体提示；不会把其他城市/坐标组合自动判成真实所在地。

为保持现有业务边界，即使更换锚点，城市文字仍为“大连市”时也沿用原拦截规则。这是既有策略限制；本轮不擅自解除，也不建议用户填写虚假城市绕过。真实在大连的使用场景需要后续单独评估显式确认策略。

## 检查记录

环境：macOS 27.2 build 26B123 / arm64，Rust 1.96.1，Cargo 1.96.1。Cargo 使用离线依赖模式，没有下载依赖。

| 本轮命令 | 实际结果 |
|---|---|
| `cargo check --workspace --all-targets --locked` | 通过；包括新增测试代码的编译。没有运行测试可执行文件。 |
| `cargo check --workspace --all-targets --locked --no-default-features` | 通过。 |
| `cargo clippy --workspace --all-targets --locked` | 通过，存在原有 warnings；未用 `-D warnings` 宣称无告警。 |
| `cargo fmt --all -- --check` | 退出 1。对 HEAD 只读导出后执行同一检查：基线与当前均为 26 个失败文件，没有新增失败文件。没有整体格式化无关文件。 |
| `git diff --check` | 通过。 |
| Mac 单元测试及 GUI 测试 | 按用户要求，本轮不执行；既有 242 项根库、8 项 route-planner 单测、7 项集成测试为前序基线记录，不冒充本轮结果。 |
| Windows 自动化测试与发行 ZIP | Run `42625859386` 成功；GUI 配置根库 248 项、CLI-only 根库 158 项；两种配置的路由单测均 9 项、集成均 7 项通过。发行 ZIP 已下载并核验。 |

新增测试涵盖：原始/筛选后 highway 计数、ways 先于 nodes、502 错误摘要和单次请求、超时分类、XML 错误/空道路/缺失节点分层、围栏裁剪前后计数、未改变返回 WGS84 坐标、无效路网不落盘、离线禁止传输、城市已改但锚点未改、空城市/无效坐标与无关设备修改兼容性。现有缓存复用与点位独立显示测试继续保留。

日志和静态检查结果：`/Users/mac/Desktop/run fasting/verification/map-device-diagnostics-20261109/`。所有新样本仅为合成公开地理坐标或 loopback HTTP，不使用账号凭据。

## Mac 交付与用户验证

已安装入口为 `/Users/mac/Applications/NekoSportsWorldTool.app`。其启动器已清除 `NSWT_OFFLINE` 与 `NSWT_DATA_DIR`，使用常规在线模式和原默认数据目录。当前应用进程不强行关闭，避免打断用户操作；本轮构建安装后需退出并重开才能加载新代码。

用户负责测试账号、登录、真实城市及锚点核对、校区选择与在线地图刷新。本轮不修改这些真实数据，不代为开始跑步，不提交运动记录。需要用户实测的项目：实际校区 OSM 覆盖、围栏与道路配对、真实地理位置匹配、GUI 提示及网络环境。Windows 编译和 Mock 通过也不能替代 Windows 实机图形验收。

## 交付记录

- 源码提交：`8f1a0753b91975c2968ed3cccdf06884b84550b7`，已推送到用户 Fork 的既有开发分支。
- Mac release 编译成功；安装位置：`/Users/mac/Applications/NekoSportsWorldTool.app`。实际为 arm64，完成本地 ad-hoc 签名与 `codesign --verify --deep --strict` 文件校验；没有 Developer ID 签名或公证。本轮没有执行 Mac GUI 或 CLI 测试。
- Mac 可执行文件 SHA-256：`db70b531e1f152ae8d0b06b3e0b1d146449ca2bcf7dd94e072f7d70500be7b8a`。
- Mac ZIP：`/Users/mac/Desktop/run fasting/verification/map-device-diagnostics-20261109/NekoSportsWorldTool-0.3.1-macOS-arm64-diagnostics.zip`，SHA-256：`70f123d817524821a34d1d08e6146b21d3749ed0877d50761d8113bb9aeb8a14`。
- 当前运行中的旧进程未重启；用户退出并重开应用后使用该提交。
- Windows Actions Run：`42625859386`，对应上述代码提交，沿用 `windows-build.yml`，结论 `success`。GUI/CLI 检查、两种 feature 配置的 workspace 单测、Clippy、release、打包和 CLI/package smoke 均通过。
- Windows 测试数量：默认 GUI 配置根 crate 248、route-planner 9 单测与 7 集成测试；CLI-only 根 crate 158、route-planner 9 单测与 7 集成测试。均为 0 失败。没有账户登录或运动记录提交测试。
- Windows ZIP：`/Users/mac/Desktop/run fasting/verification/map-device-diagnostics-20261109/windows-actions-42625859386/NekoSportsWorldTool-J-claude-Windows-x64.zip`。
- Windows ZIP SHA-256：`90037d77a97a5e3e2d90635b773e3f3f3ebf6c48a5aacda06e8f750c647f0f46`；与 runner 输出一致；ZIP CRC 及内部 538 项 checksum 均通过，共 539 个文件，保留 LICENSE 和 THIRD-PARTY-NOTICES。
- Windows EXE SHA-256：`9828efc8a6ed5233b37933b3912e3e932f1677707dba70eb3fc90c380a6576ff`。包内版本 0.3.1、目标 `x86_64-pc-windows-msvc`、完整开发署名与源码提交匹配。
- Runner 的 GUI smoke 明确为 `unavailable_missing_opengl_2_context`；Windows 10/11 实机窗口、显示/缩放和在线地图仍待用户完成，不能把构建成功标记为实机通过。
- 未创建公开 Release；保护的四个已有工作区文件 SHA-256 均保持原值。
