//! 校园地图：先显示点位，后台读取道路；文件导入保留为高级选项。

use eframe::egui;
use std::sync::Arc;

use route_planner::{Coord, RoadGraph};

use super::map::{collect_bounds, MapState};
use super::{theme, App};

#[derive(Clone)]
pub struct NetworkLoad {
    pub pending: bool,
    pub warning: String,
    pub offline: bool,
    pub request_id: u64,
    pub campuses: Vec<crate::api::campus::Campus>,
    pub selected_key: String,
    pub scope: Option<(i64, String, Option<crate::location::Coordinate>, String)>,
    pub inputs: Option<super::run::PreviewInputs>,
    pub result: Result<Arc<RoadGraph>, String>,
}

pub struct OsmPage {
    pub(super) display_network: Option<Arc<RoadGraph>>,
    pub(super) display_scope: Option<super::map_jobs::MapDisplayScope>,
    pub(super) auto_scope: Option<super::map_jobs::MapDisplayScope>,
    pub(super) auto_attempted: bool,
    pub(super) pending_manual_path: Option<String>,
    pub(super) map_only_request: bool,
    pub campuses: Vec<crate::api::campus::Campus>,
    pub selected_key: String,
    pub offline: bool,
    pub cancel: Arc<crate::task_control::Control>,
    pub cache_entries: Vec<crate::api::school_roads::Entry>,
    pub gpx_path: String,
    pub gpx_text: Option<String>,
    pub gpx: Option<crate::track::analysis::Analysis>,
    pub request_id: u64,
    pub path: String,
    pub busy: bool,
    pub status: String,
    pub map: MapState,
    pub fitted: bool,
}

impl Default for OsmPage {
    fn default() -> Self {
        OsmPage {
            display_network: None,
            display_scope: None,
            auto_scope: None,
            auto_attempted: false,
            pending_manual_path: None,
            map_only_request: false,
            cancel: Arc::new(crate::task_control::Control::default()),
            campuses: Vec::new(),
            selected_key: String::new(),
            offline: false,
            cache_entries: Vec::new(),
            gpx_path: String::new(),
            gpx_text: None,
            gpx: None,
            request_id: 0,
            path: String::new(),
            busy: false,
            status: String::new(),
            map: MapState::default(),
            fitted: false,
        }
    }
}

fn ring_to_pts(ring: &[Coord]) -> Vec<(f64, f64)> {
    ring.iter().map(|c| (c.lat, c.lon)).collect()
}

impl App {
    fn draw_local_map_import(&mut self, ui: &mut egui::Ui) -> bool {
        let mut load = false;
        egui::CollapsingHeader::new("高级：本地地图文件与诊断")
            .id_salt("advanced_map_import")
            .show(ui, |ui| {
                ui.label("通常无需准备文件。公共地图暂不可用时，可导入已合法获取的校园地图。");
                ui.label("支持 OpenStreetMap XML（.osm / .xml），原始坐标为 WGS84，最大 24 MiB；不支持直接导入 PBF、JSON 或网页。");
                ui.label("数据来源：OpenStreetMap 网站的小范围导出，或你有权使用的兼容 XML；请保留来源署名。文件不会上传，也不会变成平台打卡点。");
                theme::external_link(ui, "OpenStreetMap 小范围地图导出", "https://www.openstreetmap.org/export");
                ui.label("程序会检查文件大小、文本格式、完整性、道路和坐标。导入失败会保留此前的地图设置。");
                ui.label("文件只提供道路；学校边界和平台点位仍从各自数据层读取，文件不替代校园边界约束。");
                ui.horizontal_wrapped(|ui| {
                    ui.label("本地地图文件：");
                    ui.add_sized([ui.available_width().clamp(100.0, 340.0), 26.0],
                        egui::TextEdit::singleline(&mut self.osm_page.path).hint_text("选择文件或拖入 .osm / .xml"));
                    #[cfg(not(target_os = "android"))]
                    if ui.button("浏览…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("OSM XML", &["osm", "xml"]).pick_file() {
                            self.osm_page.path = path.display().to_string();
                        }
                    }
                    if ui.add_enabled(!self.osm_page.path.trim().is_empty() && !self.osm_page.busy,
                        theme::primary_btn("加载路网")).clicked() {
                        load = true;
                    }
                });
                if ui.add_enabled(!crate::runtime::offline() && !self.osm_page.busy && !self.checkpoints.busy,
                    egui::Button::new("重新下载路网")).clicked() {
                    self.use_school_roads(true);
                }
                ui.small("重新下载会绕过有效道路缓存；公共服务请求至少间隔 30 秒，不会自动重试。");
                if let Some(net) = self.displayed_road_network() {
                    ui.small(format!("地图统计：节点 {} · 有向边 {} · 建筑 {}",
                        net.graph.node_count(), net.graph.edge_count(), net.buildings.len()));
                }
                if !self.osm_page.status.is_empty() {
                    ui.small(format!("详细状态：{}", self.osm_page.status));
                }
                ui.small("逐阶段计数及 HTTP 响应摘要可在运行日志页查看。");
            });
        load
    }

    pub fn draw_osm_page(&mut self, ui: &mut egui::Ui) {
        self.ensure_map_loaded();
        if self.auth_expired {
            ui.colored_label(
                theme::warn(),
                "学校登录已失效，请重新登录；本地地图仍可查看。",
            );
            if ui
                .add_enabled(
                    !crate::runtime::offline() && !self.login_busy,
                    theme::primary_btn("重新登录"),
                )
                .clicked()
            {
                self.do_login();
            }
        }
        if !self.osm_page.status.is_empty() {
            ui.colored_label(
                theme::plain(),
                crate::road_status::map_summary(&self.osm_page.status),
            );
        }

        egui::ScrollArea::vertical()
            .id_salt("school_roads_scroll")
            .show(ui, |ui| self.draw_osm_contents(ui));
    }

    fn draw_osm_contents(&mut self, ui: &mut egui::Ui) {
        let mut load_now = false;
        let displayed_network = self.displayed_road_network();

        self.draw_checkpoint_controls(ui);
        ui.separator();

        ui.label("打卡点会独立显示；校园道路在后台读取，失败后可主动重新加载。");
        let school = self
            .session
            .as_ref()
            .and_then(|s| s.profile.get("campusName"))
            .and_then(|v| v.as_str())
            .unwrap_or("请先登录并核对实际位置");
        ui.label(format!("当前学校：{school}"));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !crate::runtime::offline() && !self.osm_page.busy && !self.checkpoints.busy,
                    theme::primary_btn("自动获取学校路网"),
                )
                .clicked()
            {
                self.use_school_roads(false);
            }
            if self.osm_page.busy {
                if ui.button("取消获取路网").clicked() {
                    self.osm_page.cancel.cancel();
                    self.cancel_checkpoint_load();
                    self.osm_page.request_id = self.osm_page.request_id.wrapping_add(1);
                    self.osm_page.busy = false;
                    self.osm_page.pending_manual_path = None;
                    self.osm_page.status =
                        "已取消接收新结果；保留当前地图，已发出的请求会结束或超时".into();
                }
            } else if ui
                .add_enabled(
                    !crate::runtime::offline() || !self.osm_page.path.trim().is_empty(),
                    egui::Button::new("重试获取路网"),
                )
                .clicked()
            {
                if self.osm_page.path.trim().is_empty() {
                    self.refresh_map_roads(false);
                } else {
                    self.load_map_file_for_view(self.osm_page.path.clone());
                }
            }
        });
        if self.network.is_none() && self.displayed_road_network().is_some() {
            ui.colored_label(
                theme::warn(),
                "正在显示上次成功加载的地图，仅供查看；本次道路尚未就绪。",
            );
        }
        theme::external_link(
            ui,
            "地图数据 © OpenStreetMap contributors",
            "https://www.openstreetmap.org/copyright",
        );
        load_now |= self.draw_local_map_import(ui);
        self.draw_road_tools(ui);

        // 拖拽导入
        let dropped: Vec<String> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.as_ref().map(|p| p.display().to_string()))
                .collect()
        });
        if let Some(p) = dropped.into_iter().find(|p| !p.is_empty()) {
            self.osm_page.path = p;
            if !self.osm_page.busy {
                load_now = true;
            }
        }

        ui.add_space(4.0);
        super::map::legend(
            ui,
            &[
                ("道路", theme::road()),
                ("建筑", theme::building()),
                ("平台打卡点", egui::Color32::from_rgb(240, 180, 0)),
                ("校园围栏", egui::Color32::from_rgb(180, 118, 0)),
            ],
        );
        let rect = ui.available_rect_before_wrap();
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(
                rect.width().max(200.0),
                if ui.available_width() < 600.0 {
                    240.0
                } else {
                    400.0
                },
            ),
            egui::Sense::click_and_drag(),
        );
        let canvas = response.rect;
        // Boundary and checkpoint layers do not depend on PreviewInputs or a road result.
        let display_fences: Vec<Vec<(f64, f64)>> = self
            .boundary
            .fences
            .iter()
            .map(|f| {
                f.iter()
                    .map(|&(a, b)| crate::api::school_roads::to_wgs(a, b))
                    .collect()
            })
            .collect();
        if !self.osm_page.fitted
            && (!display_fences.is_empty()
                || self.osm_page.gpx.is_some()
                || self.checkpoints.data.stats.valid > 0)
        {
            let mut bounds = self.checkpoints.data.coordinates();
            bounds.extend(display_fences.iter().flatten().copied());
            if let Some(gpx) = &self.osm_page.gpx {
                bounds.extend(gpx.segments.iter().flatten().copied());
            }
            self.osm_page.map.fit(canvas, &bounds);
            self.osm_page.fitted = true;
        }

        if let Some(net) = &displayed_network {
            if !self.osm_page.fitted {
                let mut bounds: Vec<(f64, f64)> = collect_bounds(
                    &net.edges_deg()
                        .iter()
                        .map(|e| ring_to_pts(e))
                        .collect::<Vec<_>>(),
                );
                for b in &net.buildings {
                    bounds.extend(ring_to_pts(b));
                }
                self.osm_page.map.fit(canvas, &bounds);
                self.osm_page.fitted = true;
            }
        }

        super::checkpoints::fit_checkpoint_view(
            &mut self.osm_page.map,
            canvas,
            &self.checkpoints.data.points,
            self.checkpoints.selected.as_deref(),
            &mut self.checkpoints.fit_all,
            &mut self.checkpoints.focus,
        );
        self.osm_page.map.interact(ui, canvas);

        painter.rect_filled(canvas, egui::Rounding::ZERO, theme::map_background());
        if let Some(net) = &displayed_network {
            self.osm_page.map.draw_network(&painter, canvas, net);
        } else if self.checkpoints.data.stats.valid == 0
            && display_fences.is_empty()
            && self.osm_page.gpx.is_none()
        {
            painter.text(
                canvas.center(),
                egui::Align2::CENTER_CENTER,
                "道路图层未加载；合法打卡点缓存可独立显示",
                egui::FontId::proportional(14.0),
                egui::Color32::from_rgb(150, 160, 156),
            );
        }

        for f in &display_fences {
            self.osm_page.map.draw_polygon(
                &painter,
                canvas,
                f,
                egui::Color32::from_rgb(180, 118, 0),
                2.5,
            );
        }
        if let Some(gpx) = &self.osm_page.gpx {
            for line in &gpx.segments {
                self.osm_page.map.draw_polyline(
                    &painter,
                    canvas,
                    line,
                    egui::Color32::from_rgb(160, 40, 190),
                    2.,
                );
            }
            for line in &gpx.crossing {
                self.osm_page
                    .map
                    .draw_polyline(&painter, canvas, line, egui::Color32::RED, 3.);
            }
            for &(lat, lon) in &gpx.outside {
                self.osm_page
                    .map
                    .draw_point(&painter, canvas, lat, lon, egui::Color32::RED, 4.);
            }
        }
        let layout = super::checkpoints::draw_checkpoint_markers(
            &self.osm_page.map,
            &painter,
            &response,
            &self.checkpoints.data.points,
            &mut self.checkpoints.selected,
            &mut self.checkpoints.focus,
        );
        super::checkpoints::draw_render_counts(ui, self.checkpoints.data.stats.valid, &layout);
        self.draw_checkpoint_details(ui);

        if load_now {
            let path = self.osm_page.path.clone();
            self.load_map_file_for_view(path);
        }
    }

    pub(crate) fn begin_network_load(&mut self) -> u64 {
        self.osm_page.offline = false;
        self.prepare_display_refresh();
        self.osm_page.auto_attempted = true;
        self.osm_page.auto_scope = Some(self.map_display_scope());
        self.osm_page.pending_manual_path = None;
        self.osm_page.map_only_request = false;
        self.osm_page.cancel.cancel();
        self.osm_page.cancel = Arc::new(crate::task_control::Control::default());
        self.run_page.preview_cancel.cancel();
        self.osm_page.campuses.clear();
        self.osm_page.selected_key.clear();
        self.osm_page.request_id = self.osm_page.request_id.wrapping_add(1);
        self.network = None;
        self.run_page.preview = None;
        self.run_page.preview_inputs = None;
        self.run_page.preview_stale = true;
        self.osm_page.busy = true;
        self.osm_page.offline = false;
        self.run_page.check = None;
        self.osm_page.request_id
    }

    fn use_school_roads(&mut self, force: bool) {
        if self.session.is_none() {
            self.osm_page.status = "请先登录，才能识别学校与围栏".into();
            return;
        }
        self.config.osm_path.clear();
        self.osm_page.path.clear();
        let _ = crate::api::model::save_config(&self.config);
        self.refresh_map_roads(force);
    }

    /// 后台线程加载 OSM 路网，成功后经 `net_rx` 回传。
    pub fn load_osm_async(&mut self, path: String) {
        // Reading a local map must never depend on login, location or school APIs.
        let inputs = self.run_page.preview_inputs.clone();
        let campuses = self.osm_page.campuses.clone();
        let selected_key = self.osm_page.selected_key.clone();
        let request_id = self.begin_network_load();
        self.osm_page.path = path.clone();
        self.osm_page.status = format!("加载中：{path} …");
        self.osm_page.pending_manual_path = Some(path.clone());
        let tx = self.net_tx.clone();
        let control = self.osm_page.cancel.clone();
        std::thread::spawn(move || {
            let result = control
                .check()
                .and_then(|_| crate::api::school_roads::load_local_map(&path).map(Arc::new))
                .and_then(|net| {
                    control.check()?;
                    Ok(net)
                });
            let _ = tx.send(NetworkLoad {
                pending: false,
                warning: if inputs.is_none() {
                    "本地路网已读取；路线预览需登录后刷新策略与点位".into()
                } else {
                    String::new()
                },
                offline: false,
                request_id,
                scope: None,
                campuses,
                selected_key,
                inputs,
                result,
            });
        });
    }
}
