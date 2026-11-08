# Architecture Review

## 审查基准

- 源仓库：ConCosew/NekoSportsWorldTool；原作者和原项目：YanamiNeko/NekoSportsWorldTool。
- 原始基准提交：`e229abdbab71e377b86632e588e607413af4b93c`，原 Cargo 版本 `0.3.0`。
- 定制开发分支：`feature/j-claude-cross-platform`；非商业使用与分享。
- 本次已推送的实施提交：`17a7a68b92dbf216fd81c21055f46fbe9b3672b2`；版本 `0.3.1`。
- 第二轮小修复提交：`f3b91f287aa7c928bc22f24cd9b6a2a0a677e9f1`；仅修正 Windows API-set 识别、等价布尔式与测试格式。
- 最终字节保全修复提交：`0b1ff9ec1ddee434bf80b866189146764bd8c381`，已推送同一开发分支；仅新增固定许可源 `-text` 属性与相应工作流触发路径。
- 本报告区分源码审查、原始构建、修改后自动化与真实窗口结果，不以历史 Windows 产物替代本次证据。

## 模块与数据流

| 层 | 实际入口/模块 | 职责 |
|---|---|---|
| 桌面入口 | `src/main.rs`、`src/lib.rs` | 无参数 GUI；带参数 CLI；默认 `gui` Feature；CLI-only 可关闭默认 Features |
| GUI | `src/ui/`，egui/eframe 0.29.1 | 页面状态、布局、事件绘制、线程结果接收、主题、字体、地图与关于页 |
| 桌面初始化 | `src/ui/startup.rs`、`src/startup_data.rs` | 单窗口启动状态机、后台严格本地初始化、超时、失败恢复和身份保全 |
| 统一展示 | `src/presentation.rs` | 产品名称、完整展示文案、署名、构建信息、来源与许可常量 |
| 运行策略 | `src/runtime.rs` | `NSWT_OFFLINE=1` 策略与 HTTP 入口的统一拒绝检查 |
| CLI | `src/cli/mod.rs`、`cmds.rs` | 参数与子命令分发、输出和退出码；复用核心业务 |
| 业务/API | `src/api/`、`src/crypto/` | 数据结构、会话、配置、网络接口和协议；本次不执行真实账户写入 |
| 本地数据 | `src/template.rs`、`src/track/` | GPX/JSON 读取、轨迹和统计处理 |
| 本地路由 | `crates/route-planner` | OSM 拓扑、道路投影、闭环路线、几何/运动学计算和本地导出 |
| 平台 | `src/platform.rs`、`src/android.rs` | 数据路径、原生输入/剪贴板/安全区；Android 原有私有目录与生命周期 |
| 更新 | `src/update.rs`、GUI 更新页 | 版本检查、资产匹配、下载、解包和经确认替换 |

GUI 的现有异步模型是后台线程与消息通道，不需要引入新的大型 GUI 或异步运行时。路由、JSON、GPX 和大部分轨迹计算使用可跨平台 Rust API。桌面资源不应依赖开发机绝对路径。

## 基准问题与最小修复范围

| 原始实现 | 影响 | 本次处理方向 |
|---|---|---|
| 启动构造阶段直接读取状态，无显式初始化失败页 | 读取失败易被默认值掩盖 | 分离严格本地初始化与单窗口启动状态机 |
| 桌面字体仅扫描 Windows 字体目录 | macOS 中文可能方块 | 复用已许可的 Noto Sans SC 作为嵌入回退 |
| 桌面数据根为可执行文件目录 | 只读目录写入失败，配置混入程序目录 | 系统用户目录、旧数据保留式迁移和独立测试目录 |
| 启动无条件查询公网 IP，且可能刷新保存会话 | 单靠空会话和关闭更新不能证明离线 | 显式离线模式与所有实际 HTTP 入口保护 |
| 桌面更新默认查询上游原版及旧资产名 | 覆盖定制程序风险 | 仅检查自己的定制发行源和精确平台资产 |
| 全局主题与地图画布存在浅色硬编码 | 深色界面不一致 | 使用统一语义色，不改原操作流程 |
| 每帧固定 200 ms 重绘 | 空闲无谓唤醒 | 动画/任务按需唤醒，保留必要计时 |
| 原工作流监听 `v*`，Android 同时发布 | 定制标签可能误触签名流程 | 独立 `jclaude-v*` Windows 构建入口 |

## 资源、依赖与平台边界

- 保留 `Cargo.lock` 以及 `vendor/winit-0.30.13`。该补丁说明其修改限于 Android NativeActivity 生命周期，桌面路径未变。
- 现有 Noto Sans SC 字体和 OFL/来源说明保留；不要求用户额外安装中文字体。
- Windows 使用 MSVC 原生 Runner；Mac 安装 Windows target 不等价于完成 Windows 原生链接或实机验收。
- Windows 包采用实际依赖白名单，字体嵌入时仍保留许可文本，不复制开发目录、凭证或历史下载包。
- 原项目 CC BY-NC 4.0、原作者、来源地址及修改说明继续保留；完整署名不代表原项目原创归属发生变化。

## 数据保全与文件系统边界

本地初始化先严格读取配置与会话，再创建缺失身份；损坏 JSON 显示不含原始内容的错误，保留原件。旧数据按白名单暂存复制后发布，新目录已有状态时不混合，显式 `NSWT_DATA_DIR` 不自动迁移真实用户数据。Mac `HOME`、Windows `LOCALAPPDATA` 及隔离目录必须为绝对路径。

身份首次写入采用同目录暂存、同步并以硬链接无覆盖发布，再读回实际获胜身份，防止两个程序同时首次启动生成不同身份。普通 JSON 更新采用同步暂存与原子替换；Unix 新建私有状态文件使用 `0600` 权限。APFS/NTFS 可支持此身份发布方式；不支持硬链接的自定义文件系统将显式报错，不降级覆盖已有身份。并发一致性与文件权限属于本次修改后测试范围，结果以测试日志为准。

## 本次验证结果与边界

修改前 Mac GUI/CLI 基线先行完成。修改后 locked 检查、测试、release 构建均通过：GUI 主库 204、CLI 主库 136，均另有路由库 8 与集成 7。GUI/CLI 两个实际程序各通过 30 项本地、离线和 loopback Mock 子进程验证。真实 Mac 窗口已观察启动展示、中文、九页切换、输入、本地地图导入与正常退出。

前三轮 Windows Actions 依次失败于 API-set 物理 DLL 误检、固定许可源 CRLF hash mismatch、Windows Server 2022 缺少 OpenGL 2.0+ context。修复提交 `36eeab7405f0e6f72d4faaa2fd73b509b274e84d` 只让工作流对最后一种精确环境限制记录 `GUI smoke unavailable`，不记为通过，其他失败仍阻断构建和上传。第四轮 `42496483831` 构建成功并上传 artifact；ZIP/EXE、PE x64、版本资源、静态 CRT、533 份许可及 538 项内部哈希已独立核验。GUI context 受限，Windows 10/11 实机桌面验收仍待完成。Mac 的测试工具曾在退出后自动重启且未继承离线环境，已立即停止并修复测试启动器；因此不把所有启动描述为从未联网，详细范围见 Mac_Deployment_Report.md。未使用真实账户执行运动记录提交或其他线上账户写入，未创建公开 Release。
