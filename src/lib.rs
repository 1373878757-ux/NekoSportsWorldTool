//! Shared desktop and Android application.

mod api;
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod checkpoint;
pub mod cli;
mod crypto;
mod diagnostics;
pub mod location;
#[cfg(feature = "gui")]
mod location_acquisition;
pub mod platform;
pub mod presentation;
mod relaunch;
mod road_status;
pub mod runtime;
mod startup_data;
mod task_control;
pub mod template;
mod textlog;
mod track;
mod update;

#[cfg(feature = "gui")]
mod ui;

#[cfg(all(target_os = "android", feature = "android"))]
mod android;

#[cfg(feature = "gui")]
pub fn run_gui(options: eframe::NativeOptions) -> eframe::Result<()> {
    eframe::run_native(
        presentation::APP_NAME,
        options,
        Box::new(|cc| {
            #[cfg(target_os = "android")]
            android::set_context(&cc.egui_ctx);
            #[cfg(target_os = "android")]
            {
                Ok(Box::new(ui::App::new(cc)))
            }
            #[cfg(not(target_os = "android"))]
            {
                Ok(Box::new(ui::startup::DesktopApp::new(cc)))
            }
        }),
    )
}
