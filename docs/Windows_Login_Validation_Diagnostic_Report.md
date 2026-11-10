# Windows 登录验证码错误诊断与处理修正

日期：2026-11-10。版本：0.3.1。Fork：`1373878757-ux/NekoSportsWorldTool`。分支：`feature/j-claude-cross-platform`。基线：`65bce9243f9ddcda3a932c64f5be9c8470ef80f2`。本轮源码提交：`6a303303df3109ef1a9005cc9dcd25bb291de5f2`。

## 结论

用户提供的 Windows 照片包含两次不同的失败：一次平台验证码校验通过后，登录接口返回 `10175：账号或密码错误，还可尝试2次，失败后将锁定1小时`；后续尝试的验证码凭证为空，平台二次校验返回 `10003`。这两个业务码应分别处理。照片不能证明账户已经锁定，也不能证明 Windows 网络栈或上游服务整体故障。

已修复软件将空验证码凭证记为“GT4 通过”、继续提交不完整凭证，以及未知校验结果仍放行到登录接口的问题。本轮只修正失败处理与日志，不调整验证码解算算法，不保证解决实际验证码拒绝或账号密码错误；真实账户登录结果未验收。

## 照片证据与限制

原始附件 `IMG_1883.HEIC` 仅用于查看。报告不收录账号、设备标识、IP、验证码令牌或原始照片。

| 观察 | 能支持的结论 | 不能支持的结论 |
|---|---|---|
| 一次 `[geevalidate] 验证通过`，随后 `业务错误 10175` | 该次已到达登录业务校验，服务器报告账号或密码错误并给出剩余次数 | 不能把这次归为验证码未通过，也不能断言账户已经锁定 |
| 后续多次 `GT4 通过 lot_number=`，值为空 | 软件未在调用平台二次校验前检查凭证完整性，成功文案不准确 | 缺少同次脱敏 GT4 原始响应，不能断言为什么没有凭证 |
| 后续三次 `geevalidate` 失败并显示 `10003` | 根据现有源码，这是平台验证码二次校验的业务拒绝码 | 不是 HTTP 状态码；不能据此判断 DNS、TLS 或 Windows 兼容性失败 |
| 照片版本号 `0.3.1` | 程序可以启动并进入登录界面 | 照片未显示完整构建提交号，不能确认用户运行的是哪个同版本构建 |

## 实际源码流程

`src/ui/jobs.rs::do_login` 在后台任务中调用 `src/api/login.rs::login`：

```text
checkGeeUse
  ├─ data=true：原免验证码路径
  └─ 其他：原 GT4 流程 → 凭证完整性检查 → geevalidate
                  ├─ 明确成功 10000：进入 login
                  ├─ 明确拒绝 10003：保留原有最多三轮上限
                  └─ 其他/缺失/无法解密：停止，不进入 login
login → 原业务错误处理 → 原会话保存
```

macOS 与 Windows 共用以上模块。`src/api/client.rs::make_agent` 使用同一 30 秒超时，当前这些函数没有 Windows 专属分支。照片已有业务响应，说明该次请求已走到应用层；这不等于完整排除验证码风控、网络环境、过期凭证或接口变化等原因。

`src/api/gt4.rs::solve_once` 原来只检查外层 `status`，然后直接读取 `data.seccode`；字段缺失会形成 JSON null。该模块本轮保持不变，在登录边界拒绝不完整结果。没有请求验证码服务来复现或优化解算。

## 修改文件

| 文件 | 修改 |
|---|---|
| `src/api/login.rs` | 提取凭证请求体校验和平台响应分类；四个必需凭证完整才请求；只接受明确成功；修正日志与最后一次失败提示；不输出 `lot_number`；停止后不做最后一次无意义等待 |
| `src/api/login_validation_tests.rs` | 新增 8 项合成数据/loopback 回归测试，不接触真实账户和验证码服务 |
| 本报告 | 记录证据、修改边界、静态检查、Windows 验证与交付 |

凭证字段保持原名 `lotNumber`、`captchaOutput`、`passToken`、`genTime`，其余平台请求体字段及顺序保持原有方式。前三项必须为非空字符串；`genTime` 接受非空字符串或正整数，保留原始值，不修改令牌或猜测字段替代值。未改变登录口令、Basic 认证、加密信封、设备 UUID 生成或身份数据。

`10003` 的原三轮上限及轮间两秒等待不增加；第三次失败明确显示停止，不再显示“重试”并额外等待。缺失凭证和未知响应立即停止。免验证码路径、成功登录后的原保存逻辑及登录接口 `10175` 错误消息保持原样。

品牌常量、启动动画、地图、定位、点位缓存、路线和运动提交源码均未修改。已有 `.gitignore` 修改和三份未跟踪调查报告保留，不纳入本轮提交。

## 静态检查

环境：macOS 27.2 / 26B123 / arm64；Rust 1.96.1，Cargo 1.96.1。基线 `cargo check --all-targets --locked` 通过。本轮遵守用户“不在 Mac 上本地测试”的约束，没有执行单元测试、GUI、CLI 或真实账户登录。

| 检查 | 实际结果 |
|---|---|
| `cargo check --all-targets --locked` | 通过，包含测试代码编译 |
| 同上加 `--no-default-features` | 通过 |
| `cargo clippy --all-targets --locked` | 通过，保留既有 warnings；本次两个源码文件没有 Clippy warning |
| 同上加 `--no-default-features` | 通过，保留既有 warnings |
| 两个修改文件 `rustfmt --edition 2021 --check --config skip_children=true` | 通过 |
| `cargo fmt --all -- --check` | 失败；剩余 25 个文件均为本轮未修改文件的原有格式差异，未整体格式化 |
| `git diff --check` | 通过 |

日志位于 `/Users/mac/Desktop/run fasting/verification/login-validation-20261110/`，包括 `static-checks.json`、各命令日志、`format-failures.json` 及基线登录文件的 rustfmt 差异。

## Windows 自动化验证

现有 Windows workflow 由源码推送触发：Run `42779264987`，目标 `x86_64-pc-windows-msvc`，Rust 1.96.1；2026-11-10 实际完成，结果 `success`，构建提交与本报告源码提交一致。

| Windows 检查 | 实际结果 |
|---|---|
| 默认 GUI/CLI 检查、测试、Clippy | 通过；主 crate 278 项、route-planner 单测 10 项、集成 7 项，0 失败 |
| CLI-only 检查、测试、release 构建 | 通过；主 crate 169 项、route-planner 单测 10 项、集成 7 项，0 失败 |
| 本轮新增 8 项登录校验测试 | 两种 feature 配置均通过 |
| CLI `--help`、`--version` | 通过 |
| GUI/CLI release、PE 与系统依赖检查、许可收集、ZIP、解压后校验与 CLI smoke | 通过 |
| Runner GUI smoke | `unavailable_missing_opengl_2_context`；不能视为 GUI 实机通过 |
| Windows 10/11 实机、真实验证码与真实账户登录 | 未执行 |

新增 8 项测试覆盖：

1. 完整凭证构造的原有平台请求体字段和值不变。
2. 数字时间戳保持原始数值。
3. 缺失、null、空白、类型异常凭证在 HTTP 入口之前拒绝，且没有成功日志。
4. 只有 `10000` 成功；`10003` 明确拒绝；未知码停止。
5. 缺失和类型异常业务码停止，不暴露原始响应。
6. 非 2xx 或无法解密响应停止，不暴露原始响应。
7. 平台错误码和剩余次数提示保留。
8. 真实加密请求仅发送到 loopback `geevalidate` 路径，成功/失败/未知响应按规则处理，日志不包含测试凭证。

这些测试验证本地请求边界和响应处理，不代表实际验证码通过率或真实账户登录成功。保留原有 Windows 工作流，不创建公开 Release。

## 构建与交付

本轮已推送源码到现有 Fork 开发分支，没有创建公开 Release。交付目录：

`/Users/mac/Desktop/run fasting/NekoSportsWorldTool-login-validation-6a30330/`

Windows ZIP：

`Windows/NekoSportsWorldTool-J-claude-Windows-x64.zip`

ZIP SHA-256：

```text
5ee79a0cb13b1fc9a1d51c58e369426dd631b51d53d6642415d795f552e91c5e
```

包内 EXE SHA-256：

```text
65693c78af177dc0094c1ead2ca0ffe533e4759e1aa48902358e7b5efa176fca
```

本机下载后再次核验 ZIP CRC、外部 SHA-256、全部 538 项内部文件哈希，均一致；共 539 个文件，`BUILD-INFO.txt` 的提交号、版本、架构、产品名和完整署名匹配。保留原有许可证和依赖许可，无账户配置、会话、身份和日志数据。Windows 包保持未签名状态。

Windows 构建证据 `Windows/windows-build-evidence.json` 与下载后核验 `package-verification.json` 随交付目录提供；完整运行日志与 Run JSON 位于上文 verification 目录。Windows 实机 GUI 与真实账户验证仍须单独进行。Mac 本轮没有启动程序或生成新 Mac 安装包。

## 用户侧核对

照片已经出现账号或密码错误及剩余次数提示，应先在官方客户端核对账号、密码和账户状态，避免连续尝试耗尽次数。官方客户端完成正常验证，也不代表当前工具可以复用该验证码结果。不要改设备 ID、关闭 TLS 校验、伪造凭证或通过高频重试处理此问题。

本轮未执行真实账户登录、运动记录提交或其他线上数据写入。现有验证码自动流程与平台要求是否仍兼容，需要合法的正常验证集成及进一步证据；不能把本轮错误处理修正称为“登录问题彻底解决”。
