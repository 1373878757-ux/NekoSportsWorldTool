//! Read-only platform checkpoints. This WGS84 view never requests OSM or plans a route.

use super::map::MapState;
use super::{theme, App};
use crate::checkpoint::{CheckpointViewModel, ValidationStatus};
use eframe::egui;
use egui::{Color32, Pos2, Rect};
use std::collections::HashSet;

const CHECKPOINT_COLOR: Color32 = Color32::from_rgb(240, 180, 0);
const SELECTED_COLOR: Color32 = Color32::from_rgb(139, 158, 255);

#[derive(Debug, Default)]
pub(super) struct MarkerLayout {
    /// One entry for every valid entity in the viewport, including co-located entities.
    pub visible: Vec<(usize, Pos2)>,
    pub outside_viewport: usize,
    pub icon_positions: usize,
}

fn project_coordinates(
    map: &MapState,
    canvas: Rect,
    coordinates: impl IntoIterator<Item = (usize, (f64, f64))>,
) -> MarkerLayout {
    let mut layout = MarkerLayout::default();
    let mut positions = HashSet::new();
    for (index, (lat, lon)) in coordinates {
        let position = map.to_screen(canvas, lat, lon);
        if canvas.contains(position) {
            positions.insert((position.x.round() as i32, position.y.round() as i32));
            layout.visible.push((index, position));
        } else {
            layout.outside_viewport += 1;
        }
    }
    layout.icon_positions = positions.len();
    layout
}

fn hit_test(layout: &MarkerLayout, position: Pos2, selected: Option<usize>) -> Option<usize> {
    let mut hits: Vec<_> = layout
        .visible
        .iter()
        .filter_map(|(index, screen)| {
            let distance = screen.distance(position);
            (distance <= 10.0).then_some((*index, distance))
        })
        .collect();
    hits.sort_by(|a, b| a.1.total_cmp(&b.1));
    let Some((_, closest)) = hits.first().copied() else {
        return None;
    };
    let colocated: Vec<_> = hits
        .into_iter()
        .take_while(|(_, distance)| *distance <= closest + 1.0)
        .map(|(index, _)| index)
        .collect();
    if colocated.len() < 2 {
        return colocated.first().copied();
    }
    let current =
        selected.and_then(|selected| colocated.iter().position(|index| *index == selected));
    colocated
        .get(current.map_or(0, |index| (index + 1) % colocated.len()))
        .copied()
}

fn platform_id(point: &CheckpointViewModel) -> String {
    match point.platform_id.as_ref() {
        Some(serde_json::Value::String(id)) => id.clone(),
        Some(id) => id.to_string(),
        None => "未提供".into(),
    }
}

fn coordinate_text(latitude: Option<f64>, longitude: Option<f64>) -> String {
    match (latitude, longitude) {
        (Some(lat), Some(lon)) => format!("纬度 {lat:.8}，经度 {lon:.8}"),
        _ => "未提供完整坐标".into(),
    }
}

/// Apply a user request to this canvas only; route previews keep their separate BD09 viewport.
pub(super) fn fit_checkpoint_view(
    map: &mut MapState,
    canvas: Rect,
    points: &[CheckpointViewModel],
    selected: Option<&str>,
    fit_all: &mut bool,
    focus: &mut bool,
) {
    if std::mem::take(fit_all) {
        let coordinates: Vec<_> = points.iter().filter_map(|p| p.map_coordinate()).collect();
        map.fit(canvas, &coordinates);
    }
    if std::mem::take(focus) {
        if let Some(coordinate) = points
            .iter()
            .find(|point| Some(point.internal_id.as_str()) == selected)
            .and_then(|point| point.map_coordinate())
        {
            map.fit(canvas, &[coordinate]);
        }
    }
}

pub(super) fn draw_checkpoint_markers(
    map: &MapState,
    painter: &egui::Painter,
    response: &egui::Response,
    points: &[CheckpointViewModel],
    selected: &mut Option<String>,
    focus: &mut bool,
) -> MarkerLayout {
    let canvas = response.rect;
    let painter = painter.with_clip_rect(canvas);
    let layout = project_coordinates(
        map,
        canvas,
        points
            .iter()
            .enumerate()
            .filter_map(|(index, point)| point.map_coordinate().map(|coord| (index, coord))),
    );
    let selected_index = points
        .iter()
        .position(|p| selected.as_deref() == Some(&p.internal_id));
    let hovered = response
        .hover_pos()
        .and_then(|position| hit_test(&layout, position, selected_index));
    if response.clicked() {
        if let Some(index) = hovered {
            *selected = Some(points[index].internal_id.clone());
            *focus = true;
        }
    }
    for &(index, position) in &layout.visible {
        let is_selected = selected.as_deref() == Some(points[index].internal_id.as_str());
        painter.circle_filled(position, 5.0, CHECKPOINT_COLOR);
        painter.circle_stroke(
            position,
            5.0,
            egui::Stroke::new(1.0, theme::map_background()),
        );
        if is_selected {
            painter.circle_stroke(position, 9.0, egui::Stroke::new(2.0, SELECTED_COLOR));
            painter.text(
                position + egui::vec2(12.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                points[index].display_name(),
                egui::FontId::proportional(13.0),
                SELECTED_COLOR,
            );
        }
    }
    if let Some(index) = hovered {
        response.clone().on_hover_ui(|ui| {
            let point = &points[index];
            ui.label(point.display_name());
            ui.label(format!("平台编号：{}", platform_id(point)));
            ui.label("平台打卡点（只读）");
        });
    }
    layout
}

pub(super) fn draw_render_counts(ui: &mut egui::Ui, valid: usize, layout: &MarkerLayout) {
    ui.small(format!(
        "地图数据层 {valid} 个 · 视口内绘制 {} 个 · 屏幕标记位置 {} 处 · 视口外 {} 个",
        layout.visible.len(),
        layout.icon_positions,
        layout.outside_viewport,
    ));
    if layout.icon_positions < layout.visible.len() {
        ui.small("部分点位标记重叠；各平台记录均保留，可通过列表逐个选中。");
    }
}

impl App {
    pub(super) fn draw_checkpoint_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.strong("平台打卡点（只读）");
            if ui
                .add_enabled(
                    !self.checkpoints.busy,
                    egui::Button::new(if crate::runtime::offline() {
                        "重读点位缓存"
                    } else {
                        "刷新平台打卡点"
                    }),
                )
                .clicked()
            {
                self.refresh_checkpoint_layer();
            }
            if self.checkpoints.busy && ui.button("取消点位加载").clicked() {
                self.cancel_checkpoint_load();
            }
            if ui
                .add_enabled(
                    self.checkpoints.data.stats.valid > 0,
                    egui::Button::new("显示全部点位"),
                )
                .clicked()
            {
                self.checkpoints.fit_all = true;
            }
            if ui
                .add_enabled(
                    self.checkpoints.selected.is_some(),
                    egui::Button::new("定位选中点位"),
                )
                .clicked()
            {
                self.checkpoints.focus = true;
            }
        });
        let data = &self.checkpoints.data;
        ui.label(format!(
            "打卡点：{} 个有效 · 缓存：{}{}",
            data.stats.valid,
            data.cache_state.label(),
            if self.checkpoints.busy {
                " · 正在读取"
            } else {
                ""
            },
        ));
        if !self.checkpoints.scope_note.is_empty() {
            ui.small(&self.checkpoints.scope_note);
        }
        if let Some(error) = &data.error {
            ui.colored_label(theme::warn(), format!("打卡点：{error}"));
        }
        ui.horizontal_wrapped(|ui| {
            let roads = if self.osm_page.busy {
                "正在加载".to_string()
            } else if self.network.is_some() {
                "已加载".to_string()
            } else if self.osm_page.status.is_empty() {
                "未加载".to_string()
            } else {
                self.osm_page.status.clone()
            };
            ui.label(format!("道路：{roads}"));
            let boundary = if self.boundary.status.is_empty() {
                "未加载"
            } else {
                &self.boundary.status
            };
            ui.label(format!("围栏：{boundary}"));
            ui.label(if self.run_page.preview_busy {
                "路线：生成中"
            } else if self.run_page.preview.is_some() {
                "路线：已生成"
            } else {
                "路线：未生成"
            });
        });
    }

    pub(super) fn draw_checkpoint_details(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("打卡点列表、详情与完整性统计")
            .id_salt("checkpoint_details")
            .show(ui, |ui| {
                let data = &self.checkpoints.data;
                let stats = &data.stats;
                if let Some(response) = &self.checkpoints.last_response_stats {
                    ui.label(format!(
                        "本次接口响应：原始 {} · 解析 {} · 有效 {} · 异常 {}",
                        response.raw, response.parsed, response.valid, response.invalid,
                    ));
                }
                ui.label(format!(
                    "当前显示来源：平台原始 {} · 成功解析 {} · 有效坐标 {} · 异常 {}",
                    stats.raw, stats.parsed, stats.valid, stats.invalid,
                ));
                ui.small(format!(
                    "编号缺失 {} · 名称缺失 {} · 重复编号 {} · 重合坐标 {}（不按坐标去重）",
                    stats.missing_ids,
                    stats.missing_names,
                    stats.duplicate_ids,
                    stats.overlapping_coordinates,
                ));
                egui::ScrollArea::vertical()
                    .id_salt("checkpoint_list")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for point in &data.points {
                            let mut label = point.display_name().to_string();
                            if point.platform_id.is_some() {
                                label.push_str(&format!(" · 编号 {}", platform_id(point)));
                            }
                            if point.map_coordinate().is_none() {
                                label.push_str(" · 坐标无效（不绘制）");
                            }
                            if ui
                                .selectable_label(
                                    self.checkpoints.selected.as_deref()
                                        == Some(point.internal_id.as_str()),
                                    label,
                                )
                                .clicked()
                            {
                                self.checkpoints.selected = Some(point.internal_id.clone());
                                self.checkpoints.focus = true;
                            }
                        }
                    });
                if let Some(point) = data.points.iter().find(|point| {
                    Some(point.internal_id.as_str()) == self.checkpoints.selected.as_deref()
                }) {
                    ui.separator();
                    ui.strong("打卡点详情");
                    ui.label(format!("名称：{}", point.display_name()));
                    if point
                        .name
                        .as_deref()
                        .is_none_or(|name| name.trim().is_empty())
                    {
                        ui.small("名称为界面回退文案；平台未提供非空名称。");
                    }
                    ui.label(format!("平台编号：{}", platform_id(point)));
                    ui.label(format!(
                        "原始坐标（{}）：{}",
                        point.raw_crs.label(),
                        coordinate_text(point.raw_latitude, point.raw_longitude),
                    ));
                    ui.label(format!(
                        "地图坐标（{}）：{}",
                        point.map_crs.label(),
                        coordinate_text(point.map_latitude, point.map_longitude),
                    ));
                    ui.label(format!(
                        "数据来源：{} · 缓存状态：{}",
                        point.source.label(),
                        data.cache_state.label(),
                    ));
                    if let Some(cached_at) = point.cached_at {
                        let time = chrono::DateTime::from_timestamp_millis(cached_at)
                            .map(|t| {
                                t.with_timezone(&chrono::Local)
                                    .format("%Y-%m-%d %H:%M:%S")
                                    .to_string()
                            })
                            .unwrap_or_else(|| cached_at.to_string());
                        ui.label(format!("缓存时间：{time}"));
                    }
                    match &point.validation_status {
                        ValidationStatus::Valid => {
                            ui.label("坐标验证：有效");
                        }
                        ValidationStatus::Invalid(reason) => {
                            ui.colored_label(theme::warn(), format!("坐标验证：{reason}"));
                        }
                    }
                    ui.collapsing("平台原始元数据", |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("checkpoint_raw_metadata")
                            .max_height(120.0)
                            .show(ui, |ui| {
                                ui.monospace(
                                    serde_json::to_string_pretty(&point.raw).unwrap_or_default(),
                                );
                            });
                    });
                }
            });
    }

    /// A separate WGS84 map is available in classic mode and before road/route success.
    pub(super) fn draw_checkpoint_overview(&mut self, ui: &mut egui::Ui) {
        self.draw_checkpoint_controls(ui);
        super::map::legend(
            ui,
            &[
                ("平台打卡点", CHECKPOINT_COLOR),
                ("选中点位", SELECTED_COLOR),
                ("道路", theme::road()),
                ("校园围栏", Color32::from_rgb(180, 118, 0)),
            ],
        );
        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width().max(200.0), 240.0),
            egui::Sense::click_and_drag(),
        );
        let canvas = response.rect;
        let map = &mut self.run_page.checkpoint_map;
        let fences: Vec<Vec<_>> = self
            .boundary
            .fences
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|&(lat, lon)| crate::api::school_roads::to_wgs(lat, lon))
                    .collect()
            })
            .collect();
        if !self.run_page.checkpoint_fitted {
            let mut bounds = self.checkpoints.data.coordinates();
            bounds.extend(fences.iter().flatten().copied());
            if bounds.is_empty() {
                if let Some(net) = &self.network {
                    bounds.extend(net.graph.node_weights().map(|n| (n.coord.lat, n.coord.lon)));
                }
            }
            if !bounds.is_empty() {
                map.fit(canvas, &bounds);
                self.run_page.checkpoint_fitted = true;
            }
        }
        fit_checkpoint_view(
            map,
            canvas,
            &self.checkpoints.data.points,
            self.checkpoints.selected.as_deref(),
            &mut self.checkpoints.fit_all,
            &mut self.checkpoints.focus,
        );
        map.interact(ui, canvas);
        painter.rect_filled(canvas, egui::Rounding::ZERO, theme::map_background());
        if let Some(net) = &self.network {
            map.draw_network(&painter, canvas, net);
        }
        for fence in &fences {
            map.draw_polygon(&painter, canvas, fence, Color32::from_rgb(180, 118, 0), 2.0);
        }
        let layout = draw_checkpoint_markers(
            map,
            &painter,
            &response,
            &self.checkpoints.data.points,
            &mut self.checkpoints.selected,
            &mut self.checkpoints.focus,
        );
        if self.checkpoints.data.stats.valid == 0 && self.network.is_none() && fences.is_empty() {
            painter.text(
                canvas.center(),
                egui::Align2::CENTER_CENTER,
                "当前数据目录尚无可显示的合法打卡点；点位查看无需 OSM 路网。",
                egui::FontId::proportional(13.0),
                theme::text_dim(),
            );
        }
        draw_render_counts(ui, self.checkpoints.data.stats.valid, &layout);
        self.draw_checkpoint_details(ui);
        ui.small("拖拽平移 · 滚轮缩放 · 点击点位查看详情；平台点位只读，与本地自定义选点分离。");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut App, context: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 1100.0))),
                events,
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    app.draw_checkpoint_overview(ui);
                });
            },
        )
    }

    fn click(app: &mut App, context: &egui::Context, position: Pos2) {
        for pressed in [true, false] {
            frame(
                app,
                context,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }

    fn text_position(output: &egui::FullOutput, prefix: &str) -> Option<Pos2> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with(prefix) => {
                Some(Rect::from_min_size(text.pos, text.galley.size()).center())
            }
            _ => None,
        })
    }

    #[test]
    fn egui_draws_all_cached_entities_and_opens_metadata_without_roads() {
        use crate::checkpoint::{CacheState, CheckpointData, CheckpointSource};
        use serde_json::json;

        let context = egui::Context::default();
        let mut app = App::from_state(
            &context,
            crate::api::model::HeaderIdentity::default(),
            crate::api::model::Session::default(),
            crate::api::model::Config::default(),
        );
        let point = |id, name, lat, lon| {
            let (lat, lon) = crate::track::geom::wgs84_to_bd09(lat, lon);
            json!({"id": id, "name":name, "lat":lat,"lon":lon})
        };
        app.checkpoints.data = CheckpointData::from_points(
            vec![
                point(1001, "东区打卡点", 39.9, 116.4),
                point(1002, "同址打卡点", 39.9, 116.4),
                point(1003, "西区打卡点", 39.901, 116.401),
            ],
            CheckpointSource::PlatformCache,
            Some(1_800_000_000_000),
            CacheState::Fresh,
        );
        app.osm_page.status = "HTTP 502".into();
        assert!(app.network.is_none());
        assert!(app.run_page.preview_inputs.is_none());
        frame(&mut app, &context, vec![]);
        let output = frame(&mut app, &context, vec![]);
        let markers: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Circle(circle)
                    if circle.fill == CHECKPOINT_COLOR && circle.radius == 5.0 =>
                {
                    assert!(clipped.clip_rect.contains(circle.center));
                    Some(circle.center)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            markers.len(),
            3,
            "each entity must submit one visible marker"
        );
        assert!(text_position(
            &output,
            "地图数据层 3 个 · 视口内绘制 3 个 · 屏幕标记位置 2 处 · 视口外 0 个"
        )
        .is_some());
        assert!(text_position(&output, "道路：HTTP 502").is_some());
        let header = text_position(&output, "打卡点列表、详情与完整性统计").unwrap();
        click(&mut app, &context, header);
        click(&mut app, &context, markers[0]);
        let output = frame(&mut app, &context, vec![]);
        assert_eq!(
            app.checkpoints.selected.as_deref(),
            Some(app.checkpoints.data.points[0].internal_id.as_str())
        );
        assert!(text_position(&output, "名称：东区打卡点").is_some());
        assert!(text_position(&output, "平台编号：1001").is_some());
        assert!(text_position(&output, "地图坐标（WGS84）").is_some());
        assert!(text_position(&output, "原始坐标（BD-09）").is_some());
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Circle(circle) if circle.stroke.color == SELECTED_COLOR)));
        let selected = app.checkpoints.data.points[0].map_coordinate().unwrap();
        assert!((app.run_page.checkpoint_map.center.0 - selected.0).abs() < 1e-9);
        assert!((app.run_page.checkpoint_map.center.1 - selected.1).abs() < 1e-9);

        let canvas = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == theme::map_background() && rect.rect.height() == 240.0 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .unwrap();
        let old_scale = app.run_page.checkpoint_map.scale;
        let center = canvas.center();
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(center),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, 60.0),
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(app.run_page.checkpoint_map.scale > old_scale);
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(center),
                egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let dragged = center + egui::vec2(30.0, 20.0);
        frame(&mut app, &context, vec![egui::Event::PointerMoved(dragged)]);
        frame(
            &mut app,
            &context,
            vec![egui::Event::PointerButton {
                pos: dragged,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert!(app.run_page.checkpoint_map.offset.length() > 20.0);
        assert_eq!(app.checkpoints.data.stats.valid, 3);
        assert!(app.network.is_none());
        assert!(app.run_page.preview_inputs.is_none());
    }

    #[test]
    fn viewport_counts_keep_distinct_colocated_entities() {
        let canvas = Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 240.0));
        let mut map = MapState::default();
        map.fit(canvas, &[(28.2, 112.9)]);
        let layout = project_coordinates(
            &map,
            canvas,
            [(0, (28.2, 112.9)), (1, (28.2, 112.9)), (2, (29.0, 113.0))],
        );
        assert_eq!(layout.visible.len(), 2);
        assert_eq!(layout.icon_positions, 1);
        assert_eq!(layout.outside_viewport, 1);
        assert_eq!(
            layout.visible.iter().map(|p| p.0).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn selection_follows_pan_and_zoom_without_changing_coordinates() {
        let canvas = Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 240.0));
        let mut map = MapState::default();
        let coordinates = [(7, (28.2, 112.9)), (9, (28.2001, 112.9001))];
        map.fit(canvas, &coordinates.map(|(_, coord)| coord));
        map.pan(egui::vec2(12.0, 5.0));
        map.zoom(-20.0);
        let layout = project_coordinates(&map, canvas, coordinates);
        assert_eq!(layout.visible.len(), 2);
        let screen = map.to_screen(canvas, coordinates[1].1 .0, coordinates[1].1 .1);
        assert_eq!(hit_test(&layout, screen, None), Some(9));
        assert_eq!(hit_test(&layout, canvas.left_top(), None), None);
        assert_eq!(coordinates[1].1, (28.2001, 112.9001));
    }

    #[test]
    fn colocated_points_cycle_selection_without_dropping_entities() {
        let canvas = Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 240.0));
        let mut map = MapState::default();
        let same = (28.2, 112.9);
        map.fit(canvas, &[same, same]);
        let layout = project_coordinates(&map, canvas, [(0, same), (1, same)]);
        assert_eq!(
            hit_test(&layout, map.to_screen(canvas, same.0, same.1), None),
            Some(0)
        );
        assert_eq!(
            hit_test(&layout, map.to_screen(canvas, same.0, same.1), Some(0)),
            Some(1)
        );
        assert_eq!(
            hit_test(&layout, map.to_screen(canvas, same.0, same.1), Some(1)),
            Some(0)
        );
    }
}
