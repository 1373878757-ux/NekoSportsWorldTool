//! Desktop-only, single-window local initialization and startup presentation.

use super::{fonts, theme, App};
use crate::presentation::{SIGNATURE, TAGLINE};
use eframe::egui;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

const MIN_DISPLAY: Duration = Duration::from_millis(800);
const FADE_DURATION: Duration = Duration::from_millis(250);
const INITIALIZATION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Timeline {
    fade_started: Option<Duration>,
}

impl Timeline {
    /// None means the presentation has finished. Initialization never times out
    /// into success; only an actual successful local result permits transition.
    fn opacity(&mut self, elapsed: Duration, initialized: bool) -> Option<f32> {
        if !initialized || elapsed < MIN_DISPLAY {
            return Some(1.0);
        }
        let start = *self.fade_started.get_or_insert(elapsed);
        let progress = elapsed.saturating_sub(start).as_secs_f32() / FADE_DURATION.as_secs_f32();
        (progress < 1.0).then(|| 1.0 - smooth(progress))
    }
}

fn smooth(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

type InitResult = Result<crate::startup_data::InitialState, String>;

pub struct DesktopApp {
    started: Instant,
    timeline: Timeline,
    receiver: Option<Receiver<InitResult>>,
    app: Option<App>,
    error: Option<String>,
    font_loaded: Option<String>,
    background_started: bool,
}

impl DesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let font_loaded = fonts::install(&cc.egui_ctx);
        let mut app = Self {
            started: Instant::now(),
            timeline: Timeline::default(),
            receiver: None,
            app: None,
            error: None,
            font_loaded,
            background_started: false,
        };
        app.start_initialization(&cc.egui_ctx);
        app
    }

    fn replace_attempt(&mut self, receiver: Receiver<InitResult>) {
        self.started = Instant::now();
        self.timeline = Timeline::default();
        self.error = None;
        self.app = None;
        self.background_started = false;
        // A fresh receiver also ensures a retired attempt cannot change a retry.
        self.receiver = Some(receiver);
    }

    fn start_initialization(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        self.replace_attempt(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(crate::startup_data::initialize)
                .map_err(|_| "本地初始化意外中止；请重试或退出后检查部署报告。".to_owned())
                .and_then(|result| result.map_err(|error| error.to_string()));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn poll_initialization(&mut self, ctx: &egui::Context) {
        self.poll_initialization_at(ctx, self.started.elapsed());
    }

    fn poll_initialization_at(&mut self, ctx: &egui::Context, elapsed: Duration) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) if elapsed < INITIALIZATION_TIMEOUT => return,
            Err(TryRecvError::Empty) => {
                Err("本地初始化已超过 30 秒；请检查磁盘和数据目录后重试，或退出程序。".to_owned())
            }
            Err(TryRecvError::Disconnected) => {
                Err("初始化线程已退出，尚未返回可用的本地状态。".to_owned())
            }
        };
        self.receiver = None;
        match result {
            Ok(initial) => {
                self.app = Some(App::from_initialized_state(
                    ctx,
                    initial.identity,
                    initial.session,
                    initial.config,
                    self.font_loaded.clone(),
                ));
            }
            Err(error) => {
                eprintln!("NSWT_GUI_INITIALIZATION=error: {error}");
                self.error = Some(error);
            }
        }
    }

    fn draw_error(&mut self, ctx: &egui::Context) {
        let mut retry = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let width = ui.available_width().min(560.0);
                ui.add_space((ui.available_height() * 0.25).max(16.0));
                ui.vertical_centered(|ui| {
                    ui.set_max_width(width);
                    ui.heading(TAGLINE);
                    ui.add_space(16.0);
                    ui.colored_label(theme::err(), "本地初始化失败");
                    if let Some(error) = &self.error {
                        ui.label(error);
                    }
                    ui.add_space(8.0);
                    ui.label("原有文件已保留。修复目录权限或数据问题后，可重试初始化。");
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        retry = ui.add(theme::primary_btn("重试")).clicked();
                        if ui.button("退出程序").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(24.0);
                    ui.label(
                        egui::RichText::new(SIGNATURE)
                            .small()
                            .color(theme::text_dim()),
                    );
                });
            });
        });
        if retry {
            self.start_initialization(ctx);
        }
    }
}

impl eframe::App for DesktopApp {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        theme::background().to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.poll_initialization(ctx);
        if self.error.is_some() {
            self.draw_error(ctx);
            return;
        }
        let elapsed = self.started.elapsed();
        let opacity = self.timeline.opacity(elapsed, self.app.is_some());
        if let Some(app) = &mut self.app {
            app.interactive = opacity.is_none();
            app.update(ctx, frame);
            if opacity.is_none() && !self.background_started {
                // The main UI has now drawn. Network checks cannot hold Splash.
                app.start_background_services();
                self.background_started = true;
                eprintln!("NSWT_GUI_INITIALIZATION=ready");
                ctx.request_repaint();
            }
        } else {
            egui::CentralPanel::default().show(ctx, |_| {});
        }
        if let Some(opacity) = opacity {
            draw_presentation(ctx, elapsed, opacity);
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

fn draw_presentation(ctx: &egui::Context, elapsed: Duration, opacity: f32) {
    let rect = ctx.screen_rect();
    // The foreground area intercepts input while the main UI fades in behind it.
    egui::Area::new(egui::Id::new("startup_presentation"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .movable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(rect.size(), egui::Sense::click_and_drag());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, theme::background().gamma_multiply(opacity));
            let seconds = elapsed.as_secs_f32();
            let appear = smooth(seconds / 0.4) * opacity;
            let center = rect.center() - egui::vec2(0.0, 12.0);
            let font_size = (rect.width() / 9.0).clamp(24.0, 46.0);
            painter.text(
                center + egui::vec2(0.0, (1.0 - smooth(seconds / 0.4)) * 5.0),
                egui::Align2::CENTER_CENTER,
                TAGLINE,
                egui::FontId::proportional(font_size),
                theme::text().gamma_multiply(appear),
            );
            let line = egui::Rect::from_center_size(
                center + egui::vec2(0.0, 52.0),
                egui::vec2(108.0, 2.0),
            );
            painter.rect_filled(line, 1.0, theme::border().gamma_multiply(appear));
            let position = (seconds * 1.4).sin() * 0.5 + 0.5;
            let left = line.left() + position * (line.width() - 30.0);
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(left, line.top()), egui::vec2(30.0, 2.0)),
                1.0,
                theme::accent().gamma_multiply(appear),
            );
            painter.text(
                egui::pos2(rect.center().x, rect.bottom() - 30.0),
                egui::Align2::CENTER_CENTER,
                SIGNATURE,
                egui::FontId::proportional((rect.width() / 29.0).clamp(10.0, 13.0)),
                theme::text_dim().gamma_multiply(smooth((seconds - 0.1) / 0.3) * opacity),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_initialization_still_observes_minimum_and_fade() {
        let mut timeline = Timeline::default();
        assert_eq!(
            timeline.opacity(Duration::from_millis(100), true),
            Some(1.0)
        );
        assert_eq!(timeline.opacity(MIN_DISPLAY, true), Some(1.0));
        assert_eq!(
            timeline.opacity(Duration::from_millis(925), true),
            Some(0.5)
        );
        assert_eq!(timeline.opacity(Duration::from_millis(1050), true), None);
    }

    #[test]
    fn slow_or_failed_initialization_never_advances_by_time_alone() {
        let mut timeline = Timeline::default();
        assert_eq!(timeline.opacity(Duration::from_secs(60), false), Some(1.0));
        assert_eq!(timeline.opacity(Duration::from_secs(61), true), Some(1.0));
        assert_eq!(timeline.opacity(Duration::from_millis(61250), true), None);
        let mut retry = Timeline::default();
        assert_eq!(retry.opacity(Duration::ZERO, false), Some(1.0));
    }

    fn pending_app(receiver: Receiver<InitResult>) -> DesktopApp {
        DesktopApp {
            started: Instant::now(),
            timeline: Timeline::default(),
            receiver: Some(receiver),
            app: None,
            error: None,
            font_loaded: None,
            background_started: false,
        }
    }

    #[test]
    fn timeout_reports_failure_and_a_retry_ignores_the_retired_attempt() {
        let ctx = egui::Context::default();
        let (old_tx, old_rx) = mpsc::channel();
        let mut desktop = pending_app(old_rx);
        desktop.poll_initialization_at(&ctx, INITIALIZATION_TIMEOUT - Duration::from_millis(1));
        assert!(desktop.error.is_none());
        desktop.poll_initialization_at(&ctx, INITIALIZATION_TIMEOUT);
        assert!(desktop.error.as_ref().unwrap().contains("30"));
        assert!(desktop.app.is_none());
        assert!(old_tx.send(Err("retired result".into())).is_err());
        let (tx, rx) = mpsc::channel();
        desktop.replace_attempt(rx);
        assert!(desktop.error.is_none());
        tx.send(Ok(crate::startup_data::InitialState {
            identity: crate::api::model::HeaderIdentity::default(),
            session: crate::api::model::Session::default(),
            config: crate::api::model::Config::default(),
        }))
        .unwrap();
        desktop.poll_initialization_at(&ctx, Duration::from_millis(1));
        assert!(desktop.app.is_some());
        assert!(desktop.error.is_none());
        assert!(!desktop.background_started);
    }

    #[test]
    fn disconnected_or_failed_initialization_is_a_visible_error() {
        let ctx = egui::Context::default();
        let (tx, rx) = mpsc::channel();
        drop(tx);
        let mut desktop = pending_app(rx);
        desktop.poll_initialization_at(&ctx, Duration::ZERO);
        assert!(desktop.error.is_some());
        assert!(desktop.app.is_none());
        let (tx, rx) = mpsc::channel();
        desktop.replace_attempt(rx);
        tx.send(Err("fixture directory is read-only".into()))
            .unwrap();
        desktop.poll_initialization_at(&ctx, Duration::ZERO);
        assert_eq!(
            desktop.error.as_deref(),
            Some("fixture directory is read-only")
        );
    }

    #[test]
    fn main_window_controls_do_not_accept_clicks_during_presentation() {
        let ctx = egui::Context::default();
        let mut app = App::from_state(
            &ctx,
            crate::api::model::HeaderIdentity::default(),
            crate::api::model::Session::default(),
            crate::api::model::Config::default(),
        );
        app.interactive = false;
        let run_frame = |app: &mut App, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(880.0, 720.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.draw_shell(ctx);
                    draw_presentation(ctx, Duration::from_millis(900), 0.5);
                },
            )
        };
        let _ = run_frame(&mut app, vec![]);
        let output = run_frame(&mut app, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    (text.galley.text() == "关于")
                        .then(|| egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                } else {
                    None
                }
            })
            .expect("main About tab remains painted underneath presentation");
        for pressed in [true, false] {
            let _ = run_frame(
                &mut app,
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
        assert_eq!(
            app.tab, 0,
            "presentation must prevent hidden controls from activating"
        );
    }

    #[test]
    fn presentation_contains_only_approved_text_and_fits_small_and_hidpi_views() {
        for (width, height, pixels_per_point) in [
            (320.0, 480.0, 1.0),
            (880.0, 720.0, 1.5),
            (1920.0, 1080.0, 2.0),
        ] {
            let ctx = egui::Context::default();
            fonts::install(&ctx);
            ctx.set_pixels_per_point(pixels_per_point);
            // Areas need a sizing pass before producing their visible shapes.
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, height),
                    )),
                    ..Default::default()
                },
                |ctx| draw_presentation(ctx, Duration::from_millis(500), 1.0),
            );
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, height),
                    )),
                    ..Default::default()
                },
                |ctx| draw_presentation(ctx, Duration::from_millis(500), 1.0),
            );
            let texts: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|clipped| {
                    if let egui::Shape::Text(text) = &clipped.shape {
                        Some((clipped, text))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(texts.len(), 2);
            assert!(texts.iter().any(|(_, text)| text.galley.text() == TAGLINE));
            assert!(texts
                .iter()
                .any(|(_, text)| text.galley.text() == SIGNATURE));
            for (clipped, text) in texts {
                let bounds = egui::Rect::from_min_size(text.pos, text.galley.size());
                assert!(
                    clipped.clip_rect.contains_rect(bounds),
                    "clipped presentation text at {width}×{height}"
                );
            }
        }
    }
}
