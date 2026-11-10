//! 设备信息页：iOS/Android 单选 + 6 字段 + 整套随机 + 持久化。
//! ⚠ 设备 ID 固定复用（防 10121 风控）；随机后必须手动「保存」才生效。

use super::{mobile, theme, App};
use crate::crypto::header::HeaderIdentity;
use crate::location_acquisition::{BrowserLocationRequest, LocationFix};
use eframe::egui;

const IOS_OS_POOL: [&str; 5] = ["17.5.1", "18.0.1", "18.1", "18.2", "26.5.2"];
const ANDROID_MODEL_POOL: [&str; 8] = [
    "22081212C", "OPPO PGBM10", "Redmi K60", "HUAWEI Mate 40", "vivo V2309A",
    "Pixel 8", "SM-S9210", "OnePlus ACE3",
];
const ANDROID_OS_POOL: [&str; 4] = ["12", "13", "14", "15"];

#[derive(Default)]
pub struct DevicePage {
    saved_flash: f32,
    save_error: Option<String>,
    location_request: Option<BrowserLocationRequest>,
    location_snapshot: Option<(f64, f64)>,
    location_input_ids: [Option<egui::Id>; 2],
    location_input_interacted: bool,
    location_notice: Option<(String, bool)>,
    location_fix: Option<LocationFix>,
}

impl DevicePage {
    fn cancel_location(&mut self) {
        self.location_request = None;
        self.location_snapshot = None;
        self.location_input_interacted = false;
    }

    fn observe_coordinate_input(&mut self, index: usize, response: &egui::Response) {
        self.location_input_ids[index] = Some(response.id);
        if self.location_snapshot.is_some()
            && (response.clicked()
                || response.dragged()
                || response.has_focus()
                || response.lost_focus())
        {
            self.location_input_interacted = true;
        }
    }

    fn coordinate_input_conflicts(&self, ctx: &egui::Context) -> bool {
        // DragValue can still commit its buffered text on the frame it loses focus.
        self.location_input_interacted
            || ctx.memory(|memory| {
                self.location_input_ids
                    .iter()
                    .flatten()
                    .any(|id| memory.has_focus(*id) || memory.had_focus_last_frame(*id))
            })
    }
}

impl App {
    fn save_device_settings(&mut self) -> Result<(), String> {
        validate_location_edit(&self.identity, &self.device_buf)?;
        crate::api::model::save_identity(&self.device_buf)?;
        // Commit runtime identity only after the original persistence operation succeeds.
        self.identity = self.device_buf.clone();
        self.device_page.cancel_location();
        Ok(())
    }

    pub(super) fn poll_device_location(&mut self) {
        let result = self
            .device_page
            .location_request
            .as_ref()
            .and_then(BrowserLocationRequest::try_result);
        if let Some(result) = result {
            self.device_page.location_request = None;
            self.finish_device_location(result);
        }
    }

    fn finish_device_location(&mut self, result: Result<LocationFix, String>) {
        let Some((latitude, longitude)) = self.device_page.location_snapshot.take() else {
            return;
        };
        let input_conflicts = self.device_page.coordinate_input_conflicts(&self.ctx);
        self.device_page.location_input_interacted = false;
        let result = result.and_then(|fix| Ok((fix, fix.platform_coordinate()?)));
        match result {
            Ok((fix, coordinate)) => {
                if input_conflicts
                    || latitude.to_bits() != self.device_buf.anchor_lat.to_bits()
                    || longitude.to_bits() != self.device_buf.anchor_lon.to_bits()
                {
                    self.device_page.location_notice = Some((
                        "定位期间坐标已有手动修改，已保留你的输入；需要时可重新获取当前位置"
                            .into(),
                        true,
                    ));
                    return;
                }
                self.device_buf.anchor_lat = coordinate.latitude;
                self.device_buf.anchor_lon = coordinate.longitude;
                self.device_page.location_fix = Some(fix);
                self.device_page.save_error = None;
                self.device_page.location_notice = Some((
                    "已回填当前位置，请核对城市名称与定位精度，点击“保存”生效".into(),
                    false,
                ));
            }
            Err(error) => self.device_page.location_notice = Some((error, true)),
        }
    }

    fn draw_location_controls(&mut self, ui: &mut egui::Ui, page: &mut DevicePage) {
        ui.add_space(8.0);
        let offline = crate::runtime::offline();
        mobile::row(ui, |ui| {
            if ui
                .add_enabled(
                    !offline && page.location_request.is_none(),
                    egui::Button::new("获取当前位置"),
                )
                .clicked()
            {
                let ctx = ui.ctx().clone();
                match BrowserLocationRequest::start(move || ctx.request_repaint()) {
                    Ok(request) => {
                        page.location_snapshot =
                            Some((self.device_buf.anchor_lat, self.device_buf.anchor_lon));
                        page.location_input_interacted = false;
                        page.location_notice = Some((
                            "请在默认浏览器打开的本机定位页中点击授权，获取结果后返回软件".into(),
                            false,
                        ));
                        ui.ctx().open_url(egui::OpenUrl::new_tab(request.url()));
                        page.location_request = Some(request);
                    }
                    Err(error) => page.location_notice = Some((error, true)),
                }
            }
            if let Some(request) = &page.location_request {
                ui.label("等待定位授权…");
                ui.hyperlink_to("重新打开定位页", request.url());
                if ui.button("取消定位").clicked() {
                    page.cancel_location();
                    page.location_notice = Some(("定位已取消".into(), false));
                }
            }
        });
        if offline {
            ui.colored_label(theme::text_dim(), "离线模式下请手动填写城市与坐标。");
        } else {
            ui.colored_label(
                theme::text_dim(),
                "通过默认浏览器授权本机定位；换地点后可再次获取。定位后请核对城市名称并保存。",
            );
        }
        if let Some((notice, error)) = &page.location_notice {
            ui.colored_label(
                if *error { theme::warn() } else { theme::text_dim() },
                notice,
            );
        }
        if let Some(fix) = page.location_fix {
            if fix.platform_coordinate().is_ok_and(|coordinate| {
                (coordinate.latitude - self.device_buf.anchor_lat).abs() < 1e-9
                    && (coordinate.longitude - self.device_buf.anchor_lon).abs() < 1e-9
            }) {
                ui.colored_label(
                    if fix.wgs84.accuracy > 1000.0 {
                        theme::warn()
                    } else {
                        theme::text_dim()
                    },
                    format!(
                        "定位来源：浏览器定位（WGS84），精度约 ±{:.0} 米；输入框已转换为 BD-09。",
                        fix.wgs84.accuracy
                    ),
                );
            } else {
                page.location_fix = None;
            }
        }
    }
    #[cfg(target_os = "android")]
    pub(super) fn poll_device_info(&mut self) {
        let Some(reply) = crate::android::take_device_info() else { return };
        let info = match reply.result {
            Ok(Some(info)) => info,
            Ok(None) => {
                self.status = "未读取本机信息，可在设备信息页手动填写或再次读取".into();
                return;
            }
            Err(error) => { self.status = format!("× {error}"); return; }
        };
        let identity = if reply.initial { &self.identity } else { &self.device_buf };
        match crate::api::model::identity_with_device_info(identity, &info) {
            Ok(updated) => {
                self.device_buf = updated;
                if reply.initial {
                    match crate::api::model::save_identity(&self.device_buf) {
                        Ok(()) => {
                            self.identity = self.device_buf.clone();
                            crate::android::complete_device_info();
                            self.status = format!("已读取 {} {} / Android {}；UUID 保持不变",
                                info.manufacturer, info.model, info.os_version);
                        }
                        Err(error) => {
                            self.tab = 5;
                            self.status = format!("× 保存失败：{error}，请在设备页重新保存");
                        }
                    }
                } else {
                    self.status = format!("已读取 {} {}；点击保存后生效，UUID 保持不变", info.manufacturer, info.model);
                }
            }
            Err(error) => self.status = format!("× {error}"),
        }
    }

    pub fn draw_device(&mut self, ui: &mut egui::Ui) {
        let mut page = std::mem::take(&mut self.device_page);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_device_inner(ui, &mut page));
        self.device_page = page;
    }

    fn draw_device_inner(&mut self, ui: &mut egui::Ui, page: &mut DevicePage) {
        ui.add_space(6.0);

        mobile::row(ui, |ui| {
            ui.radio_value(&mut self.device_buf.platform, "ios".to_string(), "iOS");
            ui.radio_value(&mut self.device_buf.platform, "android".to_string(), "Android");
            ui.separator();
            let platform = self.device_buf.platform.clone();
            if ui.button("随机生成").clicked() {
                randomize(&mut self.device_buf, &platform);
                self.status = "已生成随机设备（未保存——手动保存后才生效）".into();
            }
        });

        #[cfg(target_os = "android")]
        if ui.button("读取本机信息").clicked() {
            crate::android::request_device_info(false);
        }

        ui.add_space(6.0);
        let is_ios = self.device_buf.platform != "android";
        let id_label = if is_ios { "DeviceId（UUID 大写）" } else { "DeviceId（Android）" };
        let idfa_label = if is_ios { "IDFA（可空）" } else { "IMEI（可空）" };
        let name_label = if is_ios { "设备名" } else { "机型" };
        if mobile::compact_ui(ui) {
            ui.vertical(|ui| {
                ui.label(id_label);
                mobile::text_edit(ui, "device_id", &mut self.device_buf.device_id, crate::platform::InputKind::Text, ui.available_width());
                ui.label(idfa_label);
                mobile::text_edit(ui, "device_idfa", &mut self.device_buf.idfa, crate::platform::InputKind::Text, ui.available_width());
                ui.label("系统版本：");
                mobile::text_edit(ui, "device_os", &mut self.device_buf.os_version, crate::platform::InputKind::Text, ui.available_width());
                ui.label(name_label);
                mobile::text_edit(ui, "device_name", &mut self.device_buf.device_name, crate::platform::InputKind::Text, ui.available_width());
                if !is_ios {
                    ui.label("品牌（本地展示，可空）：");
                    mobile::text_edit(ui, "device_manufacturer", &mut self.device_buf.manufacturer, crate::platform::InputKind::Text, ui.available_width());
                }
                ui.label("城市：");
                mobile::text_edit(ui, "device_city", &mut self.device_buf.city, crate::platform::InputKind::Text, ui.available_width());
                ui.label("定位锚点纬度：");
                let latitude = mobile::drag_f64(
                    ui,
                    "device_lat",
                    &mut self.device_buf.anchor_lat,
                    -90.0..=90.0,
                    0.00001,
                    6,
                );
                page.observe_coordinate_input(0, &latitude);
                ui.label("定位锚点经度：");
                let longitude = mobile::drag_f64(
                    ui,
                    "device_lon",
                    &mut self.device_buf.anchor_lon,
                    -180.0..=180.0,
                    0.00001,
                    6,
                );
                page.observe_coordinate_input(1, &longitude);
            });
        } else {
            egui::Grid::new("device_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(id_label);
                    mobile::text_edit(ui, "device_id", &mut self.device_buf.device_id, crate::platform::InputKind::Text, 340.0);
                    ui.end_row();
                    ui.label(idfa_label);
                    mobile::text_edit(ui, "device_idfa", &mut self.device_buf.idfa, crate::platform::InputKind::Text, 340.0);
                    ui.end_row();
                    ui.label("系统版本：");
                    mobile::text_edit(ui, "device_os", &mut self.device_buf.os_version, crate::platform::InputKind::Text, 120.0);
                    ui.end_row();
                    ui.label(name_label);
                    mobile::text_edit(ui, "device_name", &mut self.device_buf.device_name, crate::platform::InputKind::Text, 200.0);
                    ui.end_row();
                    if !is_ios {
                        ui.label("品牌（本地展示，可空）：");
                        mobile::text_edit(ui, "device_manufacturer", &mut self.device_buf.manufacturer, crate::platform::InputKind::Text, 200.0);
                        ui.end_row();
                    }
                    ui.label("城市：");
                    mobile::text_edit(ui, "device_city", &mut self.device_buf.city, crate::platform::InputKind::Text, 120.0);
                    ui.end_row();
                    ui.label("定位锚点纬度：");
                    let latitude = mobile::drag_f64(
                        ui,
                        "device_lat",
                        &mut self.device_buf.anchor_lat,
                        -90.0..=90.0,
                        0.00001,
                        6,
                    );
                    page.observe_coordinate_input(0, &latitude);
                    ui.end_row();
                    ui.label("定位锚点经度：");
                    let longitude = mobile::drag_f64(
                        ui,
                        "device_lon",
                        &mut self.device_buf.anchor_lon,
                        -180.0..=180.0,
                        0.00001,
                        6,
                    );
                    page.observe_coordinate_input(1, &longitude);
                    ui.end_row();
                });
        }

        self.draw_location_controls(ui, page);
        ui.add_space(4.0);
        ui.colored_label(
            theme::text_dim(),
            "定位锚点填写顺序：先填纬度，再填经度；地图常见的“经度,纬度”格式需要调换后填写。",
        );
        ui.colored_label(
            theme::text_dim(),
            "手动填写使用 BD-09 坐标；“获取当前位置”会自动转换浏览器返回的 WGS84 坐标。",
        );
        ui.colored_label(
            theme::text_dim(),
            "修改城市或定位锚点后，请点击“保存”；跑步流程只使用已保存的身份信息。",
        );
        let location_changed = location_fields_changed(&self.identity, &self.device_buf);
        if let Some(error) = &page.save_error {
            ui.colored_label(
                theme::warn(),
                format!("配置状态：保存失败，当前生效值未改变。{error}"),
            );
        } else if self.device_buf != self.identity {
            ui.colored_label(theme::warn(), "配置状态：尚未保存");
        } else if self.identity.location_configuration_issue().is_some() {
            ui.colored_label(theme::warn(), "配置状态：已保存，但定位配置无效");
        } else {
            ui.colored_label(theme::ok(), "配置状态：已保存");
        }
        if location_changed {
            if let Some(issue) = self.device_buf.location_configuration_issue() {
                ui.colored_label(theme::warn(), format!("待保存的定位配置无效：{issue}"));
            } else {
                ui.colored_label(
                    theme::warn(),
                    "检测到城市或定位锚点有未保存修改，当前跑步仍会使用上次保存的值。",
                );
            }
        }

        ui.add_space(8.0);
        mobile::row(ui, |ui| {
            if ui.add(theme::primary_btn("保存")).clicked() {
                match self.save_device_settings() {
                    Ok(()) => {
                        page.cancel_location();
                        page.location_notice = Some((
                            "设备信息已保存，换地点后可重新获取位置并保存".into(),
                            false,
                        ));
                        page.save_error = None;
                        self.status = "√ 设备身份已保存并生效".into();
                        self.log.push("√  设备信息已保存，后续请求使用已保存配置");
                        page.saved_flash = 2.0;
                    }
                    Err(e) => {
                        page.save_error = Some(e.clone());
                        self.status = format!("× 保存失败：{e}；当前生效值未改变");
                    }
                }
            }
            if ui.button("撤销修改").clicked() {
                page.cancel_location();
                page.location_fix = None;
                page.location_notice = None;
                self.device_buf = self.identity.clone();
                page.save_error = None;
            }
            if page.saved_flash > 0.0 {
                ui.colored_label(theme::ok(), "已保存");
            }
        });

        ui.add_space(12.0);
        ui.separator();
        ui.colored_label(
            theme::warn(),
            "风控提示：设备 ID 参与服务端会话校验，固定复用可显著降低 10121（设备风险）概率；",
        );
        ui.colored_label(
            theme::warn(),
            "不要每次启动都换设备；「随机生成」只改界面缓冲区，手动「保存」后才写入  生效。",
        );
        ui.add_space(4.0);
        ui.label(format!(
            "当前生效身份：{} / {} / {} / 城市 {} / 锚点({:.6},{:.6})",
            if self.identity.platform == "android" { "Android" } else { "iOS" },
            self.identity.device_name,
            self.identity.os_version,
            self.identity.city,
            self.identity.anchor_lat,
            self.identity.anchor_lon,
        ));
        if let Some(issue) = self.identity.location_configuration_issue() {
            ui.colored_label(theme::warn(), format!("当前保存的定位配置未就绪：{issue}"));
        } else {
            ui.colored_label(
                theme::ok(),
                "当前保存的定位配置通过本地格式校验；请自行核对实际所在地",
            );
        }
        if page.saved_flash > 0.0 {
            page.saved_flash -= ui.ctx().input(|i| i.stable_dt);
        }
        if self.identity.platform == "android" && !self.identity.manufacturer.is_empty() {
            ui.label(format!("当前保存品牌：{}", self.identity.manufacturer));
        }
    }
}

fn location_fields_changed(saved: &HeaderIdentity, pending: &HeaderIdentity) -> bool {
    saved.city.trim() != pending.city.trim()
        || !pending.anchor_lat.is_finite()
        || !pending.anchor_lon.is_finite()
        || (saved.anchor_lat - pending.anchor_lat).abs() > 1e-9
        || (saved.anchor_lon - pending.anchor_lon).abs() > 1e-9
}

fn validate_location_edit(saved: &HeaderIdentity, pending: &HeaderIdentity) -> Result<(), String> {
    // An unchanged legacy default may still be saved with unrelated device edits.
    // The existing run guard continues to reject it until location is configured.
    if pending.city.trim().is_empty() {
        return Err("城市不能为空".into());
    }
    pending.anchor_coordinate()?;
    if location_fields_changed(saved, pending) {
        if let Some(issue) = pending.location_configuration_issue() {
            return Err(issue);
        }
    }
    Ok(())
}

/// 整套随机：uuid v4 设备 ID（大写）；机型/系统按平台池抽取。
fn randomize(buf: &mut HeaderIdentity, platform: &str) {
    buf.manufacturer.clear();
    buf.device_id = uuid::Uuid::new_v4().to_string().to_uppercase();
    buf.app_install_time = HeaderIdentity::fresh_install_time(platform);
    if platform == "android" {
        let i = (rand::random::<f64>() * ANDROID_MODEL_POOL.len() as f64) as usize
            % ANDROID_MODEL_POOL.len();
        let j =
            (rand::random::<f64>() * ANDROID_OS_POOL.len() as f64) as usize % ANDROID_OS_POOL.len();
        buf.device_name = ANDROID_MODEL_POOL[i].into();
        buf.os_version = ANDROID_OS_POOL[j].into();
        buf.idfa = String::new();
        buf.platform = "android".into();
    } else {
        let k = (rand::random::<f64>() * IOS_OS_POOL.len() as f64) as usize % IOS_OS_POOL.len();
        buf.device_name = "iPhone".into();
        buf.os_version = IOS_OS_POOL[k].into();
        buf.idfa = uuid::Uuid::new_v4().to_string().to_uppercase();
        buf.platform = "ios".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct IsolatedConfig {
        root: std::path::PathBuf,
        old_root: Option<std::path::PathBuf>,
        old_offline: Option<bool>,
    }
    impl IsolatedConfig {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("device-config-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            let old_root = crate::platform::TEST_DATA_DIR.with(|p| p.replace(Some(root.clone())));
            let old_offline = crate::runtime::TEST_OFFLINE.with(|p| p.replace(Some(true)));
            Self {
                root,
                old_root,
                old_offline,
            }
        }
    }
    impl Drop for IsolatedConfig {
        fn drop(&mut self) {
            crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = self.old_root.take());
            crate::runtime::TEST_OFFLINE.with(|p| p.set(self.old_offline));
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    fn offline_app() -> App {
        let ctx = egui::Context::default();
        App::from_state(
            &ctx,
            HeaderIdentity {
                city: "合成测试城市".into(),
                anchor_lat: 39.9,
                anchor_lon: 116.4,
                device_id: "synthetic-device".into(),
                app_install_time: 123,
                mac_address: "02:00:00:00:00:01".into(),
                ..Default::default()
            },
            crate::api::model::Session::default(),
            crate::api::model::Config::default(),
        )
    }

    #[test]
    fn explicit_save_updates_disk_runtime_and_reload_without_changing_device_identity() {
        let isolated = IsolatedConfig::new();
        let mut app = offline_app();
        let id = app.identity.device_id.clone();
        app.device_buf.city = "哈尔滨市".into();
        app.device_buf.anchor_lat = 45.75;
        app.device_buf.anchor_lon = 126.63;
        assert_ne!(app.identity, app.device_buf);
        assert!(!isolated.root.join("identity.json").exists());
        app.save_device_settings().unwrap();
        let stored: HeaderIdentity =
            serde_json::from_slice(&std::fs::read(isolated.root.join("identity.json")).unwrap())
                .unwrap();
        assert_eq!(stored, app.identity);
        assert_eq!(crate::api::model::load_identity(), app.identity);
        assert_eq!(app.identity.device_id, id);
        assert_eq!(app.identity.anchor_lat, 45.75);
        assert_eq!(app.identity.anchor_lon, 126.63);
        assert!(!isolated.root.join("session.json").exists());
        assert!(!isolated.root.join("submission_receipts").exists());
    }

    #[test]
    fn invalid_location_and_real_io_save_failure_preserve_runtime_and_existing_data() {
        let isolated = IsolatedConfig::new();
        let mut app = offline_app();
        app.save_device_settings().unwrap();
        let original = app.identity.clone();
        let path = isolated.root.join("identity.json");
        let original_bytes = std::fs::read(&path).unwrap();
        app.device_buf.city.clear();
        assert!(app.save_device_settings().is_err());
        assert_eq!(app.identity, original);
        assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
        app.device_buf = original.clone();
        app.device_buf.city = "哈尔滨市".into();
        app.device_buf.anchor_lat = 45.75;
        app.device_buf.anchor_lon = 126.63;
        let blocked_root = isolated.root.join("not-a-directory");
        std::fs::write(&blocked_root, b"existing data").unwrap();
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(blocked_root.clone()));
        assert!(app.save_device_settings().is_err());
        assert_eq!(app.identity, original);
        assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
        assert_eq!(std::fs::read(&blocked_root).unwrap(), b"existing data");
        assert_eq!(app.device_buf.city, "哈尔滨市");
    }

    #[test]
    fn location_changes_are_detected_until_saved() {
        let saved = HeaderIdentity::default();
        let mut pending = saved.clone();
        assert!(!location_fields_changed(&saved, &pending));

        pending.city = "北京市".into();
        assert!(location_fields_changed(&saved, &pending));

        pending = saved.clone();
        pending.anchor_lat += 0.000001;
        assert!(location_fields_changed(&saved, &pending));
    }

    fn acquired_fix(latitude: f64, longitude: f64) -> LocationFix {
        LocationFix {
            wgs84: crate::location::Coordinate::new(latitude, longitude, 12.0).unwrap(),
            observed_at_ms: 1_799_999_999_000,
        }
    }

    #[test]
    fn acquired_position_is_pending_until_explicit_save_and_can_change_cities() {
        let isolated = IsolatedConfig::new();
        let mut app = offline_app();
        let original = app.identity.clone();
        for (city, lat, lon) in [("哈尔滨市", 45.75, 126.63), ("大连市", 38.914, 121.614)] {
            let saved_before = app.identity.clone();
            let disk_before = std::fs::read(isolated.root.join("identity.json")).ok();
            app.device_buf.city = city.into();
            app.device_page.location_snapshot =
                Some((app.device_buf.anchor_lat, app.device_buf.anchor_lon));
            let fix = acquired_fix(lat, lon);
            app.finish_device_location(Ok(fix));
            assert_eq!(
                app.device_buf.anchor_lat,
                fix.platform_coordinate().unwrap().latitude
            );
            assert_eq!(app.device_buf.device_id, original.device_id);
            assert_eq!(app.identity, saved_before);
            assert_eq!(
                std::fs::read(isolated.root.join("identity.json")).ok(),
                disk_before
            );
            app.save_device_settings().unwrap();
            let mut reloaded = crate::api::model::load_identity();
            // serde_json's default floating parser may differ by one ULP.
            // Check coordinate precision separately and all identity fields exactly.
            assert!((reloaded.anchor_lat - app.identity.anchor_lat).abs() < 1e-10);
            assert!((reloaded.anchor_lon - app.identity.anchor_lon).abs() < 1e-10);
            reloaded.anchor_lat = app.identity.anchor_lat;
            reloaded.anchor_lon = app.identity.anchor_lon;
            assert_eq!(reloaded, app.identity);
            assert_eq!(app.identity.city, city);
        }
        assert!(!isolated.root.join("session.json").exists());
        assert!(!isolated.root.join("submission_receipts").exists());
    }

    #[test]
    fn failed_cancelled_and_late_location_results_preserve_manual_input_and_saved_identity() {
        let _isolated = IsolatedConfig::new();
        let mut app = offline_app();
        let original = app.identity.clone();
        app.device_page.location_snapshot =
            Some((app.device_buf.anchor_lat, app.device_buf.anchor_lon));
        app.finish_device_location(Err("定位权限被拒绝".into()));
        assert_eq!(app.device_buf, original);
        assert_eq!(app.identity, original);

        app.device_page.location_snapshot =
            Some((app.device_buf.anchor_lat, app.device_buf.anchor_lon));
        app.device_buf.anchor_lat += 0.01;
        let edited = app.device_buf.clone();
        app.finish_device_location(Ok(acquired_fix(45.75, 126.63)));
        assert_eq!(app.device_buf, edited);
        assert!(app
            .device_page
            .location_notice
            .as_ref()
            .unwrap()
            .0
            .contains("手动修改"));

        app.device_page.location_snapshot =
            Some((app.device_buf.anchor_lat, app.device_buf.anchor_lon));
        app.device_page.cancel_location();
        app.finish_device_location(Ok(acquired_fix(45.75, 126.63)));
        assert_eq!(app.device_buf, edited);
        assert_eq!(app.identity, original);
    }

    #[cfg(not(target_os = "android"))]
    fn device_frame(
        app: &mut App,
        ctx: &egui::Context,
        width: f32,
        events: Vec<egui::Event>,
        result: Option<(LocationFix, Option<egui::Id>)>,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 1400.0),
                )),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                if let Some((fix, surrender)) = result {
                    if let Some(id) = surrender {
                        ctx.memory_mut(|memory| memory.surrender_focus(id));
                        assert!(ctx.memory(|memory| memory.had_focus_last_frame(id)));
                    }
                    app.finish_device_location(Ok(fix));
                }
                egui::CentralPanel::default().show(ctx, |ui| app.draw_device(ui));
            },
        );
    }

    #[cfg(not(target_os = "android"))]
    fn start_coordinate_edit(
        app: &mut App,
        ctx: &egui::Context,
        width: f32,
        index: usize,
    ) -> egui::Id {
        device_frame(app, ctx, width, vec![], None);
        device_frame(app, ctx, width, vec![], None);
        let id = app.device_page.location_input_ids[index].unwrap();
        let position = ctx.read_response(id).unwrap().rect.center();
        app.device_page.location_snapshot =
            Some((app.device_buf.anchor_lat, app.device_buf.anchor_lon));
        for pressed in [true, false] {
            device_frame(
                app,
                ctx,
                width,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                None,
            );
        }
        assert!(ctx.memory(|memory| memory.has_focus(id)));
        device_frame(app, ctx, width, vec![egui::Event::Text("40e".into())], None);
        assert_eq!(
            ctx.data(|data| data.get_temp::<String>(id)).as_deref(),
            Some("40e")
        );
        id
    }

    #[test]
    #[cfg(not(target_os = "android"))]
    fn late_location_cannot_mix_coordinates_with_an_active_egui_edit() {
        let _isolated = IsolatedConfig::new();
        for width in [440.0, 1000.0] {
            for index in 0..2 {
                let mut app = offline_app();
                let ctx = app.ctx.clone();
                let original = (app.device_buf.anchor_lat, app.device_buf.anchor_lon);
                let id = start_coordinate_edit(&mut app, &ctx, width, index);
                assert_eq!(
                    (app.device_buf.anchor_lat, app.device_buf.anchor_lon),
                    original
                );

                device_frame(
                    &mut app,
                    &ctx,
                    width,
                    vec![],
                    Some((acquired_fix(45.75, 126.63), None)),
                );
                assert_eq!(
                    (app.device_buf.anchor_lat, app.device_buf.anchor_lon),
                    original
                );
                assert_eq!(
                    ctx.data(|data| data.get_temp::<String>(id)).as_deref(),
                    Some("40e")
                );
                assert!(app
                    .device_page
                    .location_notice
                    .as_ref()
                    .unwrap()
                    .0
                    .contains("手动修改"));

                device_frame(
                    &mut app,
                    &ctx,
                    width,
                    vec![egui::Event::Text("-1".into())],
                    None,
                );
                device_frame(
                    &mut app,
                    &ctx,
                    width,
                    vec![egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    None,
                );
                device_frame(&mut app, &ctx, width, vec![], None);
                let expected = if index == 0 {
                    (4.0, original.1)
                } else {
                    (original.0, 4.0)
                };
                assert_eq!(
                    (app.device_buf.anchor_lat, app.device_buf.anchor_lon),
                    expected
                );
                assert_eq!((app.identity.anchor_lat, app.identity.anchor_lon), original);
            }
        }
    }

    #[test]
    #[cfg(not(target_os = "android"))]
    fn late_location_respects_coordinate_focus_lost_in_the_same_frame() {
        let _isolated = IsolatedConfig::new();
        let mut app = offline_app();
        let ctx = app.ctx.clone();
        let original = (app.device_buf.anchor_lat, app.device_buf.anchor_lon);
        let id = start_coordinate_edit(&mut app, &ctx, 1000.0, 0);
        // Exercise the focus check independently of the interaction history.
        app.device_page.location_input_interacted = false;
        device_frame(
            &mut app,
            &ctx,
            1000.0,
            vec![],
            Some((acquired_fix(45.75, 126.63), Some(id))),
        );
        assert_eq!(
            (app.device_buf.anchor_lat, app.device_buf.anchor_lon),
            original
        );
        assert!(app
            .device_page
            .location_notice
            .as_ref()
            .unwrap()
            .0
            .contains("手动修改"));
    }

    #[test]
    fn location_edit_validation_preserves_unrelated_device_edits_and_rejects_mismatch() {
        let saved = HeaderIdentity::default();
        let mut pending = saved.clone();
        pending.device_name = "Manual device name".into();
        assert!(validate_location_edit(&saved, &pending).is_ok());
        assert!(pending.has_unconfigured_default_location());

        pending.city = "哈尔滨市".into();
        assert!(validate_location_edit(&saved, &pending)
            .unwrap_err()
            .contains("锚点仍是内置大连默认坐标"));
        assert_eq!(saved.city, "大连市");
        pending.anchor_lat = 45.75;
        pending.anchor_lon = 126.63;
        assert!(validate_location_edit(&saved, &pending).is_ok());
        pending.city = "  ".into();
        assert!(validate_location_edit(&saved, &pending).is_err());
        pending.city = "哈尔滨市".into();
        pending.anchor_lat = f64::NAN;
        assert!(validate_location_edit(&saved, &pending).is_err());
        pending.anchor_lat = 126.63;
        pending.anchor_lon = 45.75;
        assert!(validate_location_edit(&saved, &pending).is_err());
    }

    #[test]
    fn random_device_does_not_keep_a_previously_imported_brand() {
        for platform in ["android", "ios"] {
            let mut identity = HeaderIdentity {
                manufacturer: "Previously imported brand".into(),
                ..Default::default()
            };
            randomize(&mut identity, platform);
            assert!(identity.manufacturer.is_empty(), "random {platform} identity must not retain a different device's brand");
        }
    }
}
