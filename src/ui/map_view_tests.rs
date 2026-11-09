//! Read-only map/UI regression tests with independent files and loopback HTTP.
use super::*;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Root(std::path::PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("map-view-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(path.clone()));
        crate::runtime::TEST_OFFLINE.with(|p| p.set(Some(false)));
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = None);
        crate::runtime::TEST_OFFLINE.with(|p| p.set(None));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fence_reply() -> serde_json::Value {
    json!({"error":10000,"data":{"geoFences":[]}})
}
fn point_reply() -> serde_json::Value {
    let (lat, lon) = crate::track::geom::wgs84_to_bd09(39.9, 116.4);
    json!({"error":10000,"data":{"pointsResModels":[{"id":1,"pointName":"合成打卡点","lat":lat,"lon":lon}]}})
}
fn app(root: &Root, ctx: &egui::Context, host: String) -> App {
    let (lat, lon) = crate::track::geom::wgs84_to_bd09(39.9, 116.4);
    let mut app = App::from_state(
        ctx,
        HeaderIdentity {
            city: "合成测试城市".into(),
            anchor_lat: lat,
            anchor_lon: lon,
            ..Default::default()
        },
        Session {
            uid: 42,
            unid: "77".into(),
            token: "loopback-only".into(),
            ..Default::default()
        },
        Config {
            route_mode: "legacy".into(),
            update_check: "off".into(),
            ..Default::default()
        },
    );
    app.route_fixture = Some((host, root.0.clone()));
    app.tab = 7;
    app
}
fn wait(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while app.osm_page.busy || app.checkpoints.busy {
        app.poll_messages();
        assert!(Instant::now() < deadline, "{}", app.osm_page.status);
        std::thread::sleep(Duration::from_millis(4));
    }
    app.poll_messages();
}

#[test]
fn map_opens_and_loads_points_and_roads_without_policy_or_changing_running_mode() {
    let root = Root::new();
    let ctx = egui::Context::default();
    let (host, school) = route_refresh_tests::mock_school(vec![
        (crate::api::fence::FENCE_PATH, fence_reply()),
        (crate::api::points::POINTS_PATH, point_reply()),
    ]);
    let (url, roads) = super::super::checkpoint_integration_tests::mock_overpass(
        "200 OK",
        LOCAL_OSM,
        Duration::ZERO,
    );
    let mut app = app(&root, &ctx, host);
    app.route_overpass_fixture = Some((url, Duration::from_secs(5)));
    frame(&mut app, &ctx, 1000., 900., vec![]);
    wait(&mut app);
    assert_eq!(school.join().unwrap(), 2);
    assert_eq!(roads.join().unwrap(), 1);
    assert!(app.network.is_some());
    assert_eq!(app.checkpoints.data.stats.valid, 1);
    assert_eq!(app.config.route_mode, "legacy");
    assert_eq!(
        app.run_page.route_mode,
        crate::track::generate_road::RouteMode::Legacy
    );
    assert!(app.run_page.preview_inputs.is_none());
    assert!(!app.run_busy && !app.ai_busy);
    let request_id = app.osm_page.request_id;
    for _ in 0..4 {
        app.tab = 0;
        frame(&mut app, &ctx, 1000., 900., vec![]);
        app.tab = 7;
        frame(&mut app, &ctx, 1000., 900., vec![]);
    }
    assert_eq!(app.osm_page.request_id, request_id);
    assert!(!root.0.join("submission_receipts").exists());
}

#[test]
fn viewing_a_local_map_in_road_mode_only_refreshes_checkpoint_metadata() {
    let root = Root::new();
    let ctx = egui::Context::default();
    let path = root.0.join("map.osm");
    std::fs::write(&path, LOCAL_OSM).unwrap();
    let (host, school) = route_refresh_tests::mock_school(vec![
        (crate::api::fence::FENCE_PATH, fence_reply()),
        (crate::api::points::POINTS_PATH, point_reply()),
    ]);
    let mut app = app(&root, &ctx, host);
    app.run_page.route_mode = crate::track::generate_road::RouteMode::Road;
    app.config.route_mode = "road".into();
    app.load_map_file_for_view(path.display().to_string());
    wait(&mut app);
    assert_eq!(school.join().unwrap(), 2);
    assert_eq!(app.checkpoints.data.stats.valid, 1);
    assert!(app.network.is_some());
    assert!(app.run_page.preview_inputs.is_none());
    assert!(!app.run_busy && !app.ai_busy);
}

#[test]
fn a_failed_reload_keeps_the_last_map_and_checkpoints_but_cannot_supply_a_route() {
    for (status, body, delay) in [
        ("502 Bad Gateway", "at capacity", Duration::ZERO),
        ("200 OK", "<osm></osm>", Duration::ZERO),
        ("200 OK", "<osm></osm>", Duration::from_millis(1500)),
    ] {
        let root = Root::new();
        let ctx = egui::Context::default();
        let (host, school) = route_refresh_tests::mock_school(vec![
            (crate::api::fence::FENCE_PATH, fence_reply()),
            (crate::api::points::POINTS_PATH, point_reply()),
        ]);
        let (url, first) = super::super::checkpoint_integration_tests::mock_overpass(
            "200 OK",
            LOCAL_OSM,
            Duration::ZERO,
        );
        let mut app = app(&root, &ctx, host);
        app.route_overpass_fixture = Some((url, Duration::from_secs(5)));
        app.refresh_map_roads(false);
        wait(&mut app);
        assert_eq!(school.join().unwrap(), 2);
        assert_eq!(first.join().unwrap(), 1);
        let original = app.displayed_road_network().unwrap();
        let (host, school) =
            route_refresh_tests::mock_school(vec![(crate::api::fence::FENCE_PATH, fence_reply())]);
        let (url, second) =
            super::super::checkpoint_integration_tests::mock_overpass(status, body, delay);
        app.route_fixture = Some((host, root.0.clone()));
        app.route_overpass_fixture = Some((
            url,
            if delay.is_zero() {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(1)
            },
        ));
        app.refresh_map_roads(true);
        assert!(app.network.is_none());
        assert!(Arc::ptr_eq(
            &original,
            &app.displayed_road_network().unwrap()
        ));
        wait(&mut app);
        assert_eq!(school.join().unwrap(), 1);
        assert_eq!(second.join().unwrap(), 1);
        assert!(app.network.is_none());
        assert!(app.run_page.preview_inputs.is_none());
        assert!(Arc::ptr_eq(
            &original,
            &app.displayed_road_network().unwrap()
        ));
        assert_eq!(app.checkpoints.data.stats.valid, 1);
        let id = app.osm_page.request_id;
        let output = frame(&mut app, &ctx, 1200., 1000., vec![]);
        assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("上次成功加载的地图"))));
        assert_eq!(
            app.osm_page.request_id, id,
            "failure must not start another request on redraw"
        );
        app.identity.anchor_lat += 1.;
        assert!(
            app.displayed_road_network().is_none(),
            "a different location cannot inherit the old map"
        );
    }
}

#[test]
fn advanced_file_controls_are_hidden_and_invalid_import_preserves_the_saved_source() {
    let root = Root::new();
    let ctx = egui::Context::default();
    let path = root.0.join("valid.osm");
    std::fs::write(&path, LOCAL_OSM).unwrap();
    let mut app = App::from_state(
        &ctx,
        HeaderIdentity::default(),
        Session::default(),
        Config {
            osm_path: path.display().to_string(),
            ..Default::default()
        },
    );
    app.tab = 7;
    app.load_map_file_for_view(path.display().to_string());
    wait(&mut app);
    let original = app.displayed_road_network().unwrap();
    let saved_bytes = std::fs::read(root.0.join("config.json")).unwrap();
    let output = frame(&mut app, &ctx, 1100., 1000., vec![]);
    assert!(label_position(&output, "加载路网").is_none());
    assert!(label_position(&output, "重新下载路网").is_none());
    let invalid = root.0.join("bad.osm");
    std::fs::write(&invalid, b"<html>not a map</html>").unwrap();
    app.load_map_file_for_view(invalid.display().to_string());
    wait(&mut app);
    assert_eq!(app.config.osm_path, path.display().to_string());
    assert_eq!(
        std::fs::read(root.0.join("config.json")).unwrap(),
        saved_bytes
    );
    assert!(Arc::ptr_eq(
        &original,
        &app.displayed_road_network().unwrap()
    ));
    assert!(app.network.is_none());
}
