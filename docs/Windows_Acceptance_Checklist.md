# Windows 10 / 11 x64 Acceptance Checklist

本清单仅针对本次 MSVC 定制版本。Windows Runner 的编译/CLI/PE/ZIP 检查与实际 Windows 桌面验收分开记录。未提供实机证据前，以下项目统一为 **待实机验证**；不得以旧版 GNU 测试或 Mac 截图勾选。

## 验收记录

| 项目 | 记录 |
|---|---|
| 测试日期 / 测试人 | 待实机验证 |
| Windows 版本 / Build / x64 | 待实机验证 |
| CPU / GPU / 显卡驱动 | 待实机验证 |
| 显示器分辨率 / 缩放 | 待实机验证 |
| ZIP SHA-256 / EXE SHA-256 | `62ea2f2c0d58639d72ba847b63ed90f78ba9ac84dcb109a4f77f61103ad5b25d` / `02f41aeb49578b0f51fbee038dda280bde4f3dd921bf8da119c1480f4dad7311` |
| 源码版本 / 构建提交 | `0.3.1` / `a4c5dcf4d2953f3f556a9911f735b8653cb4e825`；下载后已核对内嵌 commit |

开发分支已推送。最新 Run `42509136365`（源码 `a4c5dcf4d2953f3f556a9911f735b8653cb4e825`）成功构建并上传 ZIP。解压、CLI smoke、PE、许可和 538 项内部 hash 独立验证通过，GUI smoke 仍为 `unavailable_missing_opengl_2_context`，不算通过。下载 ZIP 位于 `/Users/mac/Desktop/run fasting/artifacts/windows-run-42509136365/NekoSportsWorldTool-J-claude-Windows-x64.zip`。以下 Windows 10/11 桌面项目仍全部待实机验证，不能以 Runner 检查替代。

## 启动与展示

本次 UI 增量将启动最短展示调整为 3500 ms、成功淡出约 400 ms，并将状态栏改为 `Powered by J Claude`。以下 Windows 项仍需 Windows 10/11 实机执行；Actions 构建成功不等价于图形实机通过。

- [ ] Windows 10 x64：完整解压后双击 EXE 正常启动，无缺失 DLL 或崩溃。
- [ ] Windows 11 x64：完整解压后双击 EXE 正常启动，无缺失 DLL 或崩溃。
- [ ] 启动页中央精确显示 `just callme...`。
- [ ] 底部精确显示 `prod.by 田宸宇 J Claude`，“田宸宇”无方块或乱码。
- [ ] 主窗口标题为 `NekoSportsWorldTool`，没有独立 J Claude 品牌标题。
- [ ] 动画流畅，初始化成功进入主界面，无黑白闪屏。
- [ ] 本地初始化失败可见错误、重试和退出；不会无限加载。
- [ ] 程序退出后进程结束，再次启动仍显示启动页。

## 图形与操作

- [ ] 1080p / 100% 缩放无截断或重叠。
- [ ] 1440p / 125% 缩放无截断或重叠。
- [ ] 4K / 150% 缩放无截断或重叠。
- [ ] 窗口缩放、页面切换、输入框和按钮正常。
- [ ] About 文案、版本、平台、来源及许可证可见。
- [ ] 地图画布和图例符合统一深色主题，路线/起终点可区分。
- [ ] 空闲时没有持续高频动画或明显异常 CPU 占用。

## CLI、文件与依赖

- [ ] `--help`、`--version` 正常输出，退出码正确，不创建 GUI。
- [ ] 未知命令报错且退出码非零；输出重定向正常。
- [ ] 中文和含空格的解压目录可启动。
- [ ] 系统用户数据目录可保存、恢复配置和本地文件。
- [ ] 隔离数据目录不触碰真实配置，离线模式阻止 HTTP。
- [ ] 旧数据迁移保留原件，不重建设备身份，不混合两套已有状态。
- [ ] 无 Rust/Cargo/Python/Node.js/开发 SDK 的干净用户环境可运行。
- [ ] 包内许可文件和校验清单完整，文件校验通过。
- [ ] 如出现 SmartScreen/Defender 提示，如实记录；不关闭安全软件。

## 边界

本次不执行真实运动记录提交、补签或其他线上账户写入。更新检查的网络行为可使用 Mock 验证；实际程序替换须经明确确认，本清单不要求通过真实自替换证明功能。
