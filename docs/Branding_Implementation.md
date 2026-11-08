# Display Text and Signature Implementation

## 已确认文案

```text
just callme...
prod.by 田宸宇 J Claude
```

主窗口和产品名称保持 `NekoSportsWorldTool`。`J Claude` 仅作为“田宸宇”的共同完整署名出现，不作为独立品牌标题、字标或窗口后缀。

## 实施约定

- 展示字符串集中在统一 Rust 常量模块，不在多个 GUI 文件重复硬编码。
- 启动页中央显示完整第一行，底部显示完整第二行；About 复用同一常量。
- 精确匹配测试保护大小写、空格和三个 ASCII 点号。
- 原作者 YanamiNeko、上游项目和维护来源、许可证及第三方许可继续展示和随包保留。
- 中性运动/路径几何图标不使用独立 J Claude 或 JC 字标。

## 启动与视觉

采用现有 egui/eframe 单窗口。启动状态为本地初始化、淡出、主界面；失败提供错误说明、重试和退出。最短展示 800 ms，文案淡入约 400 ms，成功淡出约 250 ms；本地初始化超时 30 秒显示错误状态。更新/IP/账户刷新不作为进入主界面的前提。

默认深色体系：背景 `#0B0D12`、主文字 `#F5F5F7`、次文字 `#9CA3AF`、强调色 `#8B9EFF`、分割线 `#262A35`。统一控件和地图语义色，保留页面布局、功能和路线区分。中文由已许可 Noto Sans SC 回退保障。

## 验证状态

`src/presentation.rs` 为实际统一常量模块。2026-11-08 的 `verification/local/gui-tests.log` 已通过 `presentation_text_matches_the_approved_spelling_exactly` 与 `gui_files_do_not_duplicate_the_approved_presentation_literals`，检查大小写、空格、三个 ASCII 点号和 GUI 文件禁止重复硬编码。

同次测试已通过中文完整署名字形覆盖、启动最短展示和淡出、失败/超时/重试、启动期间主窗口交互隔离，以及小窗口/HiDPI 的展示文案和边界测试。

真实 Mac 观测已有 `verification/gui/splash-slow-initialization.jpg`：中央和底部位置、两行精确文案、中文署名及原产品窗口标题正常。`about-offline.jpg` 显示相同常量署名以及原作者、来源、CC BY-NC 4.0 和字体归属。`main-offline.jpg` 与 `local-map-loaded.jpg` 记录深色主界面和实际本地地图导入；未出现独立 J Claude 标题或 JC 字标。

真实 GUI 总记录为 `verification/gui/gui-acceptance.json`；慢初始化截图通过隔离合成配置的本地读取延迟捕获，不替换程序展示逻辑。实际文案由同一常量模块进入启动页与 About，截图观测与精确匹配测试共同形成证据。

初次截图的 About 构建号仍为未提交修改时原 HEAD `e229abdbab71`；实施源码已提交并推送为 `17a7a68b92dbf216fd81c21055f46fbe9b3672b2`。提交后重建的 `about-committed.jpg` 已实际显示 `17a7a68b92db`，`resized-main.jpg` 与该 About 截图记录窗口变大后的真实布局。损坏合成配置时，`gui/error-retry/error-screen.jpg` 中保留相同的两行展示文案，错误与重试按钮可见；修复后真实重试进入主窗口。

Windows Actions 状态分类修复提交 `36eeab7405f0e6f72d4faaa2fd73b509b274e84d` 不改变展示常量或图形逻辑；最终 Run `42500313727` 构建并上传的 ZIP/EXE 已独立校验，摘要在 Windows_Build_Report.md。标准 Runner 缺少 OpenGL 2.0+ context 时，工作流将真实 GUI smoke 标为 unavailable，不声称通过。Mac 启动页/署名截图仍属于 `17a7a68`；2026-11-09 已重新观察最终二进制的离线主窗口、标题和未登录状态，详见 Mac_Deployment_Report.md。Windows 10/11 实机启动页、中文署名、窗口标题、DPI 和交互仍待实机单独验收。
