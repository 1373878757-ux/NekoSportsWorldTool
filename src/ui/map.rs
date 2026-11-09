//! 地图画布绘制原语：等距投影 + 拖拽平移 + 滚轮缩放。
//!
//! 供「路网」页与「跑步」页复用，纯离线绘制折线/多边形/点。

use super::theme;
use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

use crate::track::geom::{MET_PER_DEG_LAT, MET_PER_DEG_LNG};

/// 地图视图状态（世界坐标为度）。
pub struct MapState {
    /// 参考中心（lat, lon）。
    pub center: (f64, f64),
    /// 屏幕每米像素（缩放）。
    pub scale: f32,
    /// 平移偏移（像素）。
    pub offset: Vec2,
    /// 最近是否被拖拽过（供外部判断）。
    pub dragged: bool,
}

impl Default for MapState {
    fn default() -> Self {
        MapState {
            center: (0.0, 0.0),
            scale: 0.3,
            offset: Vec2::ZERO,
            dragged: false,
        }
    }
}

impl MapState {
    pub fn world_bounds(&self, rect: Rect) -> (route_planner::Coord, route_planner::Coord) {
        let x = |screen: f32| {
            self.center.1
                + (screen - rect.center().x - self.offset.x) as f64
                    / (self.scale as f64 * MET_PER_DEG_LNG)
        };
        let y = |screen: f32| {
            self.center.0
                - (screen - rect.center().y - self.offset.y) as f64
                    / (self.scale as f64 * MET_PER_DEG_LAT)
        };
        (
            route_planner::Coord::new(x(rect.left()), y(rect.bottom())),
            route_planner::Coord::new(x(rect.right()), y(rect.top())),
        )
    }
    /// 依据一组点（lat, lon）自动适配视野。
    pub fn fit(&mut self, rect: Rect, pts: &[(f64, f64)]) {
        let pts: Vec<_> = pts
            .iter()
            .copied()
            .filter(|(lat, lon)| {
                lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0
            })
            .collect();
        if pts.is_empty() {
            return;
        }
        let latitude = pts.iter().map(|point| point.0);
        let longitude = pts.iter().map(|point| point.1);
        self.center = (
            (latitude.clone().fold(f64::INFINITY, f64::min)
                + latitude.fold(f64::NEG_INFINITY, f64::max))
                * 0.5,
            (longitude.clone().fold(f64::INFINITY, f64::min)
                + longitude.fold(f64::NEG_INFINITY, f64::max))
                * 0.5,
        );
        let (mut minx, mut maxx, mut miny, mut maxy) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for (la, lo) in &pts {
            let x = (lo - self.center.1) * MET_PER_DEG_LNG;
            let y = (la - self.center.0) * MET_PER_DEG_LAT;
            minx = minx.min(x);
            maxx = maxx.max(x);
            miny = miny.min(y);
            maxy = maxy.max(y);
        }
        let w = (maxx - minx).max(40.0);
        let h = (maxy - miny).max(40.0);
        let sx = rect.width() as f64 / w;
        let sy = rect.height() as f64 / h;
        self.scale = ((sx.min(sy)) * 0.92) as f32;
        self.offset = Vec2::ZERO;
    }

    /// Shared pan/zoom operations keep every layer in the same screen transform.
    pub fn pan(&mut self, delta: Vec2) {
        if delta.is_finite() {
            self.offset += delta;
        }
    }

    pub fn zoom(&mut self, scroll: f32) {
        if scroll.is_finite() {
            self.scale = (self.scale * (scroll * 0.0015).exp()).clamp(0.002, 200.0);
        }
    }

    /// 度坐标 → 屏幕坐标。
    pub fn to_screen(&self, rect: Rect, lat: f64, lon: f64) -> Pos2 {
        let x = (lon - self.center.1) * MET_PER_DEG_LNG * self.scale as f64;
        let y = -(lat - self.center.0) * MET_PER_DEG_LAT * self.scale as f64;
        let c = rect.center();
        Pos2::new(
            (c.x as f64 + x + self.offset.x as f64) as f32,
            (c.y as f64 + y + self.offset.y as f64) as f32,
        )
    }

    /// 处理拖拽/缩放交互（需在分配好 rect 后调用）。
    pub fn interact(&mut self, ui: &egui::Ui, rect: Rect) {
        self.dragged = false;
        let (drag, scroll) = ui.input(|i| {
            let hover = i
                .pointer
                .interact_pos()
                .map(|p| rect.contains(p))
                .unwrap_or(false);
            let dragging = hover && i.pointer.primary_down() && i.pointer.is_decidedly_dragging();
            (dragging, if hover { i.raw_scroll_delta.y } else { 0.0 })
        });
        if drag {
            let d = ui.input(|i| i.pointer.delta());
            self.pan(d);
            self.dragged = true;
        }
        if scroll != 0.0 {
            self.zoom(scroll);
        }
    }

    pub fn draw_polyline(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        pts: &[(f64, f64)],
        color: Color32,
        width: f32,
    ) {
        if pts.len() < 2 {
            return;
        }
        let (a, b) = self.world_bounds(rect);
        let (mut minlat, mut maxlat, mut minlon, mut maxlon) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for &(lat, lon) in pts {
            minlat = minlat.min(lat);
            maxlat = maxlat.max(lat);
            minlon = minlon.min(lon);
            maxlon = maxlon.max(lon);
        }
        if maxlon < a.lon || minlon > b.lon || maxlat < a.lat || minlat > b.lat {
            return;
        }
        let ps: Vec<Pos2> = pts
            .iter()
            .map(|(la, lo)| self.to_screen(rect, *la, *lo))
            .collect();
        painter.add(egui::Shape::line(ps, Stroke::new(width, color)));
    }

    pub fn draw_polygon(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        pts: &[(f64, f64)],
        color: Color32,
        width: f32,
    ) {
        if pts.len() < 3 {
            return;
        }
        let mut ps: Vec<Pos2> = pts
            .iter()
            .map(|(la, lo)| self.to_screen(rect, *la, *lo))
            .collect();
        ps.push(ps[0]);
        painter.add(egui::Shape::closed_line(ps, Stroke::new(width, color)));
    }

    pub fn draw_point(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        lat: f64,
        lon: f64,
        color: Color32,
        radius: f32,
    ) {
        let c = self.to_screen(rect, lat, lon);
        painter.circle_filled(c, radius, color);
    }

    /// Draw the unmodified WGS84 OSM graph, without school API data or route planning.
    pub fn draw_network(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        net: &route_planner::RoadGraph,
    ) {
        let road_color = theme::road();
        let (min, max) = self.world_bounds(rect);
        let visible = net.visible_edges(min, max);
        let stride = if self.scale < 0.1 {
            (visible.len() / 4000).max(1)
        } else {
            1
        };
        for edge in visible.iter().step_by(stride) {
            self.draw_polyline(
                painter,
                rect,
                &[(edge[0].lat, edge[0].lon), (edge[1].lat, edge[1].lon)],
                road_color,
                1.0,
            );
        }
        if stride > 1 {
            painter.text(
                rect.left_top() + egui::vec2(6., 6.),
                egui::Align2::LEFT_TOP,
                "缩小视野：道路显示已简化",
                egui::FontId::proportional(12.),
                road_color,
            );
        }
        for building in &net.buildings {
            let points: Vec<_> = building.iter().map(|c| (c.lat, c.lon)).collect();
            self.draw_polygon(painter, rect, &points, theme::building(), 1.0);
        }
    }
}

/// 收集一组折线/环的所有点，用于 fit。
pub fn collect_bounds(edges: &[Vec<(f64, f64)>]) -> Vec<(f64, f64)> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for e in edges.iter().take(400) {
        for p in e {
            pts.push(*p);
        }
    }
    pts
}

/// 地图图例：颜色方块 + 说明文字。
pub fn legend(ui: &mut egui::Ui, items: &[(&str, Color32)]) {
    ui.horizontal_wrapped(|ui| {
        ui.label("图例：");
        for (label, color) in items {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::Rounding::same(2.0), *color);
            ui.label(*label);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 240.0))
    }

    #[test]
    fn fit_single_checkpoint_centers_without_zero_scale() {
        let mut map = MapState::default();
        let point = (28.2, 112.9);
        map.fit(canvas(), &[point]);
        assert_eq!(map.center, point);
        assert!(map.scale.is_finite() && map.scale > 0.0);
        assert_eq!(map.to_screen(canvas(), point.0, point.1), canvas().center());
    }

    #[test]
    fn fit_keeps_all_valid_points_inside_the_canvas() {
        let mut map = MapState::default();
        // Deliberately asymmetric density: an arithmetic-mean center clips the outlier.
        let points = [
            (28.2, 112.9),
            (28.2, 112.9),
            (28.2, 112.9),
            (28.203, 112.904),
        ];
        map.fit(canvas(), &points);
        for (lat, lon) in points {
            assert!(canvas().contains(map.to_screen(canvas(), lat, lon)));
        }
    }

    #[test]
    fn pan_moves_layers_together_and_zoom_scales_about_center() {
        let mut map = MapState::default();
        map.fit(canvas(), &[(28.2, 112.9)]);
        let point = (28.2001, 112.9001);
        let before = map.to_screen(canvas(), point.0, point.1);
        let delta = Vec2::new(32.0, -12.0);
        map.pan(delta);
        assert!((map.to_screen(canvas(), point.0, point.1) - before - delta).length() < 0.001);
        let old_scale = map.scale;
        let center = canvas().center() + map.offset;
        let distance = map.to_screen(canvas(), point.0, point.1) - center;
        map.zoom(100.0);
        let expected = distance * (map.scale / old_scale);
        assert!((map.to_screen(canvas(), point.0, point.1) - center - expected).length() < 0.001);
        assert_eq!(map.center, (28.2, 112.9));
    }
}
