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

同次测试已通过中文完整署名字形覆盖、启动最短展示和淡出、失败/超时/重试、启动期间主窗口交互隔离，以及小窗口/HiDPI 的展示文案和边界测试。启动页/主窗口/About 的真实 Mac 桌面截图仍待回填；中文实际观感不能仅凭字形测试或 ASCII 截图判定通过。
