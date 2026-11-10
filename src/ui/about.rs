//! 关于页与自动更新流程：检查 / 下载进度 / 确认弹窗 / 重启或安装收尾。
//!
//! 桌面端替换 exe 后走 relaunch 重启；Android 端下载 APK 到私有目录后
//! 交给系统安装器（platform::install_apk）。

use super::{theme, App};
use crate::presentation::{
    APP_NAME, BUILD_COMMIT, BUILD_TARGET, CUSTOMIZATION_NOTE, LICENSE_NAME, SIGNATURE,
    SOURCE_REPOSITORY, TAGLINE, UPSTREAM_AUTHOR, UPSTREAM_REPOSITORY,
};
use crate::update::ReleaseInfo;
use eframe::egui;
use std::path::Path;

const PROJECT_LICENSE: &str = include_str!("../../LICENSE");
const FONT_LICENSE: &str = include_str!("../../assets/fonts/OFL-1.1.txt");
const DEPENDENCY_LICENSE_INDEX: &str = include_str!("about_dependency_index.txt");

const CHANGELOG: &[(&str, &str, &str)] = &[
    (
        "6a30330",
        "2026-11-10",
        "验证码凭证校验：拒绝不完整凭证，未知验证结果停止流程",
    ),
    (
        "65bce92",
        "2026-11-10",
        "防止迟到的定位结果覆盖手动坐标编辑",
    ),
    ("71a455e", "2026-11-09", "增加可复用的当前位置设置入口"),
    (
        "79a8956",
        "2026-11-09",
        "校园地图图层独立加载，失败时保留已有显示",
    ),
    ("6688430", "2026-11-09", "打卡点地图图层与道路加载解耦"),
    ("a4c5dcf", "2026-11-09", "升级启动页动画并调整状态栏署名"),
    (
        "17a7a68",
        "2026-11-08",
        "统一品牌信息、离线启动与 Windows 构建流程",
    ),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum AboutPanel {
    Changelog,
    Licenses,
    Diagnostics,
}

#[derive(Clone)]
struct AboutDialog {
    panel: AboutPanel,
    copied: bool,
}

fn safe_data_dir(path: &Path, home: Option<&Path>, custom: bool) -> String {
    if custom {
        return "自定义数据目录（实际路径已隐藏）".into();
    }
    if let Some(relative) = home.and_then(|home| path.strip_prefix(home).ok()) {
        let relative = relative.to_string_lossy();
        if relative.is_empty() {
            "~".into()
        } else {
            format!("~{}{}", std::path::MAIN_SEPARATOR, relative)
        }
    } else {
        "用户数据目录（具体路径已隐藏）".into()
    }
}

fn diagnostic_data_dir() -> String {
    let path = crate::platform::data_dir();
    let custom = std::env::var_os("NSWT_DATA_DIR").is_some();
    #[cfg(target_os = "windows")]
    let home = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    safe_data_dir(&path, home.as_deref(), custom)
}

fn diagnostic_text(data_dir: &str) -> String {
    format!(
        "应用版本：{}\n构建提交：{}\n操作系统：{}\nCPU 架构：{}\n构建目标：{}\n数据目录：{}",
        crate::platform::version_name(),
        BUILD_COMMIT,
        std::env::consts::OS,
        std::env::consts::ARCH,
        BUILD_TARGET,
        data_dir,
    )
}

/// 更新流程的 UI 状态（随 App 存活，消息协议回填）。
#[derive(Default)]
pub struct UpdateUi {
    pub checking: bool,
    pub latest: Option<ReleaseInfo>,
    pub up_to_date: bool,
    pub check_error: Option<String>,
    pub downloading: bool,
    pub done: u64,
    pub total: Option<u64>,
    /// 等待用户确认开始下载（启动检查发现新版本时也置此）。
    pub confirm: Option<ReleaseInfo>,
    /// 下载替换完成后的收尾动作。
    pub finish: Option<FinishAction>,
    /// 启动询问模式：是否检查更新。
    pub ask_startup: bool,
}

#[derive(Clone)]
pub enum FinishAction {
    /// 桌面：替换完成，等待重启。
    Restart { tag: String },
    /// Android：APK 已就绪，等待调起安装器（桌面构建下不会构造）。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    InstallApk { tag: String },
}

impl App {
    pub fn draw_about(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut requested_panel = None;
        egui::ScrollArea::vertical()
            .id_salt("about_scroll")
            .show(ui, |ui| self.draw_about_content(ui, &mut requested_panel));
        if let Some(panel) = requested_panel {
            ctx.data_mut(|data| {
                data.insert_temp(
                    egui::Id::new("about_dialog_state"),
                    AboutDialog {
                        panel,
                        copied: false,
                    },
                );
            });
        }
        self.draw_about_dialog(&ctx);
    }

    fn draw_about_content(&mut self, ui: &mut egui::Ui, requested_panel: &mut Option<AboutPanel>) {
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.heading(egui::RichText::new(APP_NAME).size(28.0).strong());
            ui.label(
                egui::RichText::new(TAGLINE)
                    .size(17.0)
                    .color(theme::text_dim()),
            );
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(SIGNATURE)
                    .small()
                    .color(theme::text_dim()),
            );
            ui.add_space(7.0);
            ui.label(
                egui::RichText::new(format!("Version {}", crate::platform::version_name()))
                    .small()
                    .color(theme::accent()),
            );
            ui.add_space(14.0);
        });

        ui.label(egui::RichText::new("功能概览").strong());
        let cards = [
            ("Campus Map", "已支持", "校园地图与平台打卡点可视化"),
            (
                "Route Editor",
                "仅预览",
                "可查看路线预览；尚不支持交互式编辑",
            ),
            ("Cross Platform", "已支持", "macOS / Windows"),
        ];
        if ui.available_width() >= 690.0 {
            ui.columns(3, |columns| {
                for (column, (title, state, description)) in columns.iter_mut().zip(cards) {
                    draw_feature_card(column, title, state, description);
                }
            });
        } else {
            for (title, state, description) in cards {
                draw_feature_card(ui, title, state, description);
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("软件更新").strong());
            ui.label(
                egui::RichText::new(format!("当前版本 {}", crate::platform::version_name()))
                    .color(theme::text_dim()),
            );
        });
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            let busy = self.update.checking || self.update.downloading;
            if ui
                .add_enabled(
                    !busy && !crate::runtime::offline(),
                    theme::primary_btn("检查更新"),
                )
                .clicked()
            {
                self.check_update(true);
            }
            let status = if self.update.checking {
                ("检查中…", theme::text_dim())
            } else if self.update.check_error.is_some() {
                ("检查失败", theme::err())
            } else if self.update.up_to_date {
                (
                    if cfg!(target_os = "android") {
                        "已是最新版本"
                    } else {
                        "暂无可用定制版更新"
                    },
                    theme::ok(),
                )
            } else if self.update.latest.is_some() {
                ("发现可用更新", theme::warn())
            } else if crate::runtime::offline() {
                ("离线模式，未检查", theme::text_dim())
            } else {
                ("尚未检查", theme::text_dim())
            };
            ui.colored_label(status.1, status.0);
        });
        if let Some(rel) = self.update.latest.clone() {
            ui.add_space(3.0);
            ui.label(format!(
                "最新版本：{}（{:.1} MB）",
                rel.tag,
                rel.asset_size as f64 / 1024.0 / 1024.0,
            ));
            if !rel.title.is_empty() {
                ui.colored_label(theme::text_dim(), &rel.title);
            }
            let notes: Vec<&str> = rel.notes.lines().take(8).collect();
            for line in notes {
                ui.label(egui::RichText::new(line).small().color(theme::text_dim()));
            }
            if self.update.downloading {
                self.draw_download_progress(ui);
            } else if self.update.finish.is_none() {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!crate::runtime::offline(), egui::Button::new("立即更新"))
                        .clicked()
                    {
                        if self.any_busy() {
                            self.status = "有任务进行中，请稍后再试".into();
                        } else {
                            self.update.confirm = Some(rel.clone());
                        }
                    }
                });
            }
        }
        if self.update.downloading && self.update.latest.is_none() {
            self.draw_download_progress(ui);
        }
        if let Some(fin) = self.update.finish.clone() {
            match fin {
                FinishAction::Restart { tag } => {
                    ui.colored_label(theme::ok(), format!("√ 已更新到 {tag}，重启后生效"));
                }
                FinishAction::InstallApk { tag } => {
                    ui.colored_label(theme::ok(), format!("√ 已下载 {tag}，请在弹窗中安装"));
                }
            }
        }
        let repo = if cfg!(target_os = "android") {
            crate::update::REPO_FORK
        } else {
            crate::update::REPO_CUSTOM
        };
        if let Some(e) = &self.update.check_error {
            ui.colored_label(theme::err(), format!("× {e}"));
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new("自动检查更新").strong());
        ui.horizontal_wrapped(|ui| {
            for (m, label) in [
                ("silent", "静默检查（发现新版才提示）"),
                ("ask", "每次询问"),
                ("off", "关闭"),
            ] {
                let selected = self.config.update_check == m;
                if ui.radio(selected, label).clicked() {
                    self.config.update_check = m.to_string();
                    if crate::api::model::save_config(&self.config).is_ok() {
                        self.status = "已保存启动检查设置".into();
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            theme::external_link(
                ui,
                "查看更新源",
                format!("https://github.com/{repo}/releases"),
            );
            ui.colored_label(
                theme::text_dim(),
                "检查更新只会检查版本；下载仍需用户确认。",
            );
        });

        ui.add_space(12.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            for (label, panel) in [
                ("更新日志", AboutPanel::Changelog),
                ("开源许可", AboutPanel::Licenses),
                ("诊断信息", AboutPanel::Diagnostics),
            ] {
                if ui.button(label).clicked() {
                    *requested_panel = Some(panel);
                }
            }
        });
    }

    fn draw_about_dialog(&self, ctx: &egui::Context) {
        let id = egui::Id::new("about_dialog_state");
        let Some(mut dialog) = ctx.data(|data| data.get_temp::<AboutDialog>(id)) else {
            return;
        };
        let title = match dialog.panel {
            AboutPanel::Changelog => "更新日志",
            AboutPanel::Licenses => "开源许可",
            AboutPanel::Diagnostics => "诊断信息",
        };
        let mut open = true;
        let available =
            (ctx.screen_rect().size() - egui::vec2(24.0, 24.0)).max(egui::vec2(120.0, 120.0));
        egui::Window::new(title)
            .id(egui::Id::new("about_dialog_window"))
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_size([available.x.min(600.0), available.y.min(500.0)])
            .min_size([available.x.min(300.0), available.y.min(260.0)])
            .max_size(available)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_max_width(available.x.min(720.0));
                egui::ScrollArea::vertical()
                    .id_salt("about_dialog_scroll")
                    .show(ui, |ui| match dialog.panel {
                        AboutPanel::Changelog => draw_changelog(ui),
                        AboutPanel::Licenses => draw_licenses(ui),
                        AboutPanel::Diagnostics => {
                            let data_dir = diagnostic_data_dir();
                            ui.label("以下信息可用于排查兼容性问题，不包含账户或精确位置。");
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(diagnostic_text(&data_dir)).monospace());
                            if let Some(font) = &self.font_loaded {
                                ui.label(format!("界面字体：{font}"));
                            }
                            ui.add_space(10.0);
                            if ui.add(theme::primary_btn("复制诊断信息")).clicked() {
                                ctx.copy_text(diagnostic_text(&data_dir));
                                dialog.copied = true;
                            }
                            if dialog.copied {
                                ui.colored_label(theme::ok(), "已复制到剪贴板");
                            }
                        }
                    });
            });
        ctx.data_mut(|data| {
            if open {
                data.insert_temp(id, dialog);
            } else {
                data.remove::<AboutDialog>(id);
            }
        });
    }

    fn draw_download_progress(&self, ui: &mut egui::Ui) {
        let bar = egui::ProgressBar::new(self.update.fraction()).text(self.update.progress_text());
        ui.add(bar);
    }

    /// 全局窗口：启动询问 / 确认更新 / 更新收尾（不依赖当前标签页）。
    pub fn draw_update_windows(&mut self, ctx: &egui::Context) {
        if self.update.ask_startup {
            let mut done: Option<&str> = None;
            egui::Window::new(egui::RichText::new("检查更新").strong())
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_max_width(320.0);
                    ui.label("是否检查新版本？\n协议变更时旧版本可能异常，建议检查。");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!crate::runtime::offline(), theme::primary_btn("检查"))
                            .clicked()
                        {
                            done = Some("check");
                        }
                        if ui.button("本次跳过").clicked() {
                            done = Some("skip");
                        }
                        if ui.button("不再询问").clicked() {
                            done = Some("off");
                        }
                    });
                });
            if let Some(choice) = done {
                self.update.ask_startup = false;
                match choice {
                    "check" => self.check_update(false),
                    "off" => {
                        self.config.update_check = "off".into();
                        let _ = crate::api::model::save_config(&self.config);
                    }
                    _ => {}
                }
            }
        }

        if let Some(rel) = self.update.confirm.clone() {
            let mut close = false;
            let mut go = false;
            egui::Window::new(egui::RichText::new(format!("发现新版本 {}", rel.tag)).strong())
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_max_width((ctx.screen_rect().width() - 32.0).min(460.0));
                    if !rel.title.is_empty() {
                        ui.label(&rel.title);
                    }
                    ui.label(format!(
                        "大小 {:.1} MB · 来源 {}",
                        rel.asset_size as f64 / 1024.0 / 1024.0,
                        rel.repo
                    ));
                    let notes: Vec<&str> = rel.notes.lines().take(8).collect();
                    if !notes.is_empty() {
                        ui.add_space(4.0);
                        for line in notes {
                            ui.label(egui::RichText::new(line).small());
                        }
                    }
                    ui.add_space(4.0);
                    ui.colored_label(
                        theme::warn(),
                        if cfg!(target_os = "android") {
                            "更新会替换为所选 Release 版本并重启程序"
                        } else {
                            "确认后才会下载定制版并替换当前程序；原用户数据目录保留。"
                        },
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!crate::runtime::offline(), theme::primary_btn("立即更新"))
                            .clicked()
                        {
                            go = true;
                        }
                        if ui.button("忽略").clicked() {
                            close = true;
                        }
                    });
                });
            if go || close {
                self.update.confirm = None;
            }
            if go {
                if self.any_busy() {
                    self.status = "有任务进行中，请稍后再试".into();
                } else {
                    self.start_update_download(rel);
                }
            }
        }

        if let Some(fin) = self.update.finish.clone() {
            let mut done = false;
            let (title, body, button) = match fin {
                FinishAction::Restart { tag } => (
                    format!("已更新到 {tag}"),
                    "程序已替换完成，重启后使用新版本。".to_string(),
                    "重启程序".to_string(),
                ),
                FinishAction::InstallApk { tag } => (
                    format!("已下载 {tag}"),
                    "点击安装，在系统弹窗中确认；若提示禁止安装，请允许本应用安装未知应用。"
                        .to_string(),
                    "安装".to_string(),
                ),
            };
            egui::Window::new(egui::RichText::new(&title).strong())
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_max_width(360.0);
                    ui.label(&body);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_btn(&button)).clicked() {
                            done = true;
                        }
                        if ui.button("稍后").clicked() {
                            self.update.finish = None;
                        }
                    });
                });
            if done {
                match self.update.finish.take() {
                    Some(FinishAction::Restart { .. }) => match crate::relaunch::relaunch_self() {
                        Ok(()) => {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(e) => {
                            self.status = e.clone();
                            self.popup = Some(super::PopupInfo {
                                title: "更新完成".into(),
                                lines: vec![e],
                            });
                        }
                    },
                    Some(FinishAction::InstallApk { .. }) => {
                        crate::platform::install_apk(
                            &crate::platform::data_dir()
                                .join(crate::update::APK_NAME)
                                .to_string_lossy(),
                        );
                        self.update.finish = None;
                    }
                    None => {}
                }
            }
        }
    }
}

fn draw_feature_card(ui: &mut egui::Ui, title: &str, state: &str, description: &str) {
    egui::Frame::group(ui.style())
        .fill(theme::background())
        .stroke(egui::Stroke::new(1.0, theme::border()))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_min_height(82.0);
            ui.label(egui::RichText::new(title).strong());
            ui.colored_label(
                if state == "已支持" {
                    theme::ok()
                } else {
                    theme::warn()
                },
                state,
            );
            ui.label(
                egui::RichText::new(description)
                    .small()
                    .color(theme::text_dim()),
            );
        });
}

fn draw_changelog(ui: &mut egui::Ui) {
    ui.label(format!("当前应用版本：{}", crate::platform::version_name()));
    ui.label("以下为当前开发分支的部分 Git 提交摘要，不代表独立的已发布版本。");
    ui.add_space(10.0);
    for (commit, date, summary) in CHANGELOG {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(*date).small().color(theme::text_dim()));
            ui.label(egui::RichText::new(*summary).strong());
            ui.label(
                egui::RichText::new(*commit)
                    .monospace()
                    .small()
                    .color(theme::text_dim()),
            );
        });
        ui.separator();
    }
}

fn draw_licenses(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(APP_NAME).strong());
    ui.label(format!("原项目作者：{UPSTREAM_AUTHOR}"));
    ui.label(format!("上游来源：{UPSTREAM_REPOSITORY}"));
    ui.label(format!("源码来源：{SOURCE_REPOSITORY}"));
    ui.label(CUSTOMIZATION_NOTE);
    ui.label(format!("项目许可证：{LICENSE_NAME}"));
    ui.add_space(8.0);
    egui::CollapsingHeader::new("项目许可证全文 · CC BY-NC 4.0")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(PROJECT_LICENSE).small());
        });
    ui.separator();
    ui.label(egui::RichText::new("Noto Sans SC").strong());
    ui.label("字体许可证：SIL Open Font License 1.1");
    egui::CollapsingHeader::new("字体许可证全文 · SIL OFL 1.1")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(FONT_LICENSE).small());
        });
    ui.separator();
    ui.label(egui::RichText::new("第三方依赖").strong());
    ui.label("以下清单来自锁定的 Cargo 依赖元数据；发行包另含 THIRD-PARTY-NOTICES.txt 与 licenses/ 中的原文。");
    egui::CollapsingHeader::new("依赖版本与 SPDX 许可标识")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(DEPENDENCY_LICENSE_INDEX)
                    .monospace()
                    .small(),
            );
        });
    ui.add_space(4.0);
    theme::external_link(
        ui,
        "查看上游项目许可证",
        format!("https://github.com/{UPSTREAM_REPOSITORY}/blob/main/LICENSE"),
    );
}

impl UpdateUi {
    fn fraction(&self) -> f32 {
        match self.total {
            Some(t) if t > 0 => (self.done as f32 / t as f32).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    fn progress_text(&self) -> String {
        let mb = |n: u64| format!("{:.1} MB", n as f64 / 1024.0 / 1024.0);
        match self.total {
            Some(t) if t > 0 => format!("{}/{}", mb(self.done), mb(t)),
            _ => format!("已下载 {}", mb(self.done)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_data_directory_hides_user_and_custom_path_segments() {
        let home = Path::new("/Users/alice");
        let relative = Path::new("Library")
            .join("Application Support")
            .join("ExampleApp");
        let data = home.join(&relative);
        let display = safe_data_dir(&data, Some(home), false);
        assert_eq!(
            display,
            format!("~{}{}", std::path::MAIN_SEPARATOR, relative.display())
        );
        assert!(!display.contains("alice"));
        assert_eq!(
            safe_data_dir(&data, Some(home), true),
            "自定义数据目录（实际路径已隐藏）"
        );
        assert_eq!(
            safe_data_dir(Path::new("/private/custom/location"), Some(home), false),
            "用户数据目录（具体路径已隐藏）"
        );
    }
}
