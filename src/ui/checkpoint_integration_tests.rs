//! Isolated regression evidence for independent display layers. Only loopback HTTP.
use super::*;
use crate::checkpoint::{CacheState, CheckpointData, CheckpointSource};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

struct Root(std::path::PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("checkpoint-map-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(path.clone()));
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
fn wire(lat: f64, lon: f64) -> Value {
    let (a, b) = crate::track::geom::wgs84_to_bd09(lat, lon);
    json!({"lat":a,"lon":b})
}
fn points() -> Vec<Value> {
    let mut first = wire(39.9, 116.4);
    first["id"] = json!(1001);
    first["pointName"] = json!("东区打卡点");
    let mut same = first.clone();
    same["id"] = json!(1002);
    vec![first, same, wire(39.901, 116.401)]
}
fn app(root: &Root, ctx: &egui::Context, host: String) -> App {
    let (lat, lon) = crate::track::geom::wgs84_to_bd09(39.9, 116.4);
    let mut result = App::from_state(
        ctx,
        HeaderIdentity {
            city: "离线测试城市".into(),
            anchor_lat: lat,
            anchor_lon: lon,
            ..Default::default()
        },
        Session {
            uid: 42,
            unid: "77".into(),
            token: "synthetic-test-only".into(),
            ..Default::default()
        },
        Config {
            route_mode: "road".into(),
            update_check: "off".into(),
            ..Default::default()
        },
    );
    result.route_fixture = Some((host, root.0.clone()));
    result
}
fn cache(app: &App) {
    cache_scope(
        app.identity.anchor_coordinate().unwrap(),
        model::PointsCacheScope {
            uid: 42,
            unid: "77".into(),
            run_area_id: None,
        },
        &points(),
    );
}
fn cache_scope(
    anchor: crate::location::Coordinate,
    scope: model::PointsCacheScope,
    values: &[Value],
) {
    model::save_points_cache(anchor, &scope, values).unwrap();
}
fn wait(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(6);
    while app.osm_page.busy || app.checkpoints.busy {
        app.poll_messages();
        assert!(
            Instant::now() < deadline,
            "job timeout: {}",
            app.osm_page.status
        );
        std::thread::sleep(Duration::from_millis(4));
    }
    app.poll_messages();
}
pub(super) fn mock_overpass(
    status: &'static str,
    body: &'static str,
    delay: Duration,
) -> (String, std::thread::JoinHandle<usize>) {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(6);
        let (mut socket, _) = loop {
            match server.accept() {
                Ok(x) => break x,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(4));
                }
                Err(e) => panic!("{e}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buf = [0u8; 4096];
        let n = socket.read(&mut buf).unwrap();
        assert!(n > 0);
        assert!(String::from_utf8_lossy(&buf[..n]).starts_with("POST / HTTP/1.1"));
        std::thread::sleep(delay);
        let _ = write!(
            socket,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        // No retry should establish a second connection.
        std::thread::sleep(Duration::from_millis(80));
        assert!(server.accept().is_err());
        1
    });
    (url, handle)
}
fn fence(with: bool) -> Value {
    if with {
        json!({"error":10000,"data":{"geoFences":[{"campusId":"test","points":[
        wire(39.899,116.399),wire(39.899,116.402),wire(39.902,116.402),wire(39.902,116.399)]}]}})
    } else {
        json!({"error":10000,"data":{"geoFences":null}})
    }
}

fn stamp_cache(root: &Root, timestamp: i64) -> Vec<u8> {
    let path = root.0.join("points_cache.json");
    let mut document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["ts"] = json!(timestamp);
    let bytes = document.to_string().into_bytes();
    std::fs::write(path, &bytes).unwrap();
    bytes
}

fn api_replies(point_rows: Vec<Value>) -> Vec<(&'static str, Value)> {
    vec![
        (crate::api::fence::FENCE_PATH, fence(false)),
        (crate::api::points::POINTS_PATH, point_response(point_rows)),
    ]
}
fn data_shape(data: &CheckpointData) {
    assert_eq!(data.stats.raw, 3);
    assert_eq!(data.stats.parsed, 3);
    assert_eq!(data.stats.valid, 3);
    assert_eq!(data.points[0].platform_id, Some(json!(1001)));
    assert_eq!(data.points[0].name.as_deref(), Some("东区打卡点"));
    assert!(data.points[2].platform_id.is_none());
    assert!(data.points[2].name.is_none());
    assert_ne!(data.points[0].internal_id, data.points[1].internal_id);
}

fn point_response(rows: Vec<Value>) -> Value {
    json!({"error":10000,"data":{"pointsResModels":rows}})
}

fn mixed_points() -> Vec<Value> {
    let mut valid = wire(39.9, 116.4);
    valid["id"] = json!(2001);
    valid["pointName"] = json!("有效点");
    vec![
        valid,
        json!({"id":2002,"pointName":"非法点","lat":0,"lon":0}),
    ]
}

fn mock_school_paused_points(
    release: std::sync::mpsc::Receiver<()>,
) -> (
    String,
    std::sync::mpsc::Receiver<()>,
    std::thread::JoinHandle<usize>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    let (received_tx, received_rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut count = 0;
        for (path, response, pause) in [
            (crate::api::fence::FENCE_PATH, fence(false), false),
            (
                crate::api::points::POINTS_PATH,
                point_response(points()),
                true,
            ),
        ] {
            let deadline = Instant::now() + Duration::from_secs(6);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "request missing: {path}");
                        std::thread::sleep(Duration::from_millis(4));
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut buffer = [0u8; 4096];
                let size = stream.read(&mut buffer).unwrap();
                assert_ne!(size, 0, "incomplete request headers");
                bytes.extend_from_slice(&buffer[..size]);
                if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            assert!(headers.starts_with(&format!("POST {path} HTTP/1.1\r\n")));
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < header_end + length {
                let mut buffer = [0u8; 4096];
                let size = stream.read(&mut buffer).unwrap();
                assert_ne!(size, 0, "incomplete request body");
                bytes.extend_from_slice(&buffer[..size]);
            }
            if pause {
                received_tx.send(()).unwrap();
                release.recv_timeout(Duration::from_secs(6)).unwrap();
            }
            let body = response.to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            count += 1;
        }
        count
    });
    (host, received_rx, handle)
}

#[test]
fn cached_checkpoints_survive_real_502_timeout_and_empty_osm_with_or_without_boundary() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    for with_boundary in [true, false] {
        for (label, status, body, delay) in [
            (
                "502",
                "502 Bad Gateway",
                "Overpass at capacity",
                Duration::ZERO,
            ),
            (
                "timeout",
                "200 OK",
                "<osm></osm>",
                Duration::from_millis(1_200),
            ),
            (
                "empty",
                "200 OK",
                "<osm version=\"0.6\"></osm>",
                Duration::ZERO,
            ),
        ] {
            let root = Root::new();
            let ctx = egui::Context::default();
            let mut replies = vec![(crate::api::fence::FENCE_PATH, fence(with_boundary))];
            if !with_boundary {
                replies.push((
                    crate::api::policy::POLICY_PATH,
                    json!({"error":10000,"data":{"timestamp":123,"policy":1,"mustPoints":[]}}),
                ));
            }
            let (host, school) = super::layout_tests::route_refresh_tests::mock_school(replies);
            let (url, roads) = mock_overpass(status, body, delay);
            let mut app = app(&root, &ctx, host);
            cache(&app);
            // Leave enough time for a Windows loopback connection under CI
            // load; the delayed response still exceeds this client timeout.
            let request_timeout = if label == "timeout" {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(5)
            };
            app.route_overpass_fixture = Some((url, request_timeout));
            app.refresh_school_route(false);
            wait(&mut app);
            assert_eq!(school.join().unwrap(), if with_boundary { 1 } else { 2 });
            assert_eq!(roads.join().unwrap(), 1);
            data_shape(&app.checkpoints.data);
            if with_boundary {
                assert!(app.config.school_campus.is_some());
                // Poll another frame after automatic campus selection; it must
                // not be mistaken for a user selecting a different campus.
                app.poll_messages();
                data_shape(&app.checkpoints.data);
            }
            assert_eq!(app.boundary.fences.is_empty(), !with_boundary);
            assert!(app.network.is_none());
            assert!(app.run_page.preview_inputs.is_none());
            assert!(
                app.checkpoints.data.error.is_none(),
                "road errors cannot become checkpoint errors"
            );
            let road_status = app.osm_page.status.to_ascii_lowercase();
            let status_matches = match label {
                "502" => road_status.contains("502"),
                "timeout" => road_status.contains("[网络超时]"),
                _ => app.osm_page.status.contains("可用"),
            };
            assert!(status_matches, "{}", app.osm_page.status);
            assert!(!app.run_busy);
            assert!(!root.0.join("submission_receipts").exists());
            // Restart/re-read doesn't need OSM or route inputs.
            crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(true)));
            let mut restarted = app_from_offline(&root, &ctx);
            restarted.refresh_checkpoint_layer();
            wait(&mut restarted);
            data_shape(&restarted.checkpoints.data);
            crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
        }
    }
}
fn app_from_offline(root: &Root, ctx: &egui::Context) -> App {
    let mut app = app(root, ctx, "http://127.0.0.1:1".into());
    app.session = None;
    app
}
#[test]
fn offline_fresh_stale_corrupted_missing_cache_never_uses_http_or_a_real_session() {
    let root = Root::new();
    let ctx = egui::Context::default();
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(true)));
    for state in [
        CacheState::Fresh,
        CacheState::Stale,
        CacheState::Corrupted,
        CacheState::Missing,
    ] {
        let mut app = app_from_offline(&root, &ctx);
        if state == CacheState::Missing {
            let _ = std::fs::remove_file(root.0.join("points_cache.json"));
        } else if state == CacheState::Corrupted {
            std::fs::write(root.0.join("points_cache.json"), b"{broken").unwrap();
        } else {
            cache(&app);
            if state == CacheState::Stale {
                let file = root.0.join("points_cache.json");
                let mut doc: Value =
                    serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
                doc["ts"] = json!(crate::crypto::envelope::now_ms() - 300001);
                std::fs::write(file, doc.to_string()).unwrap();
            }
        }
        app.refresh_checkpoint_layer();
        app.refresh_checkpoint_layer();
        wait(&mut app);
        assert_eq!(app.checkpoints.data.cache_state, state);
        if matches!(state, CacheState::Fresh | CacheState::Stale) {
            data_shape(&app.checkpoints.data);
        } else {
            assert!(app.checkpoints.data.points.is_empty());
        }
        assert!(!root.0.join("session.json").exists());
        assert!(app.network.is_none());
    }
}
#[test]
fn cancelled_late_and_old_session_layer_results_cannot_replace_current_points() {
    use super::checkpoint_jobs::{LayerLoad, LayerUpdate};
    let root = Root::new();
    let ctx = egui::Context::default();
    let mut app = app(&root, &ctx, "http://127.0.0.1:1".into());
    let first = app.begin_checkpoint_load();
    app.cancel_checkpoint_load();
    app.layer_tx
        .send(LayerLoad {
            request: first,
            update: LayerUpdate::Data(CheckpointData::from_points(
                points(),
                CheckpointSource::PlatformApi,
                None,
                CacheState::Fresh,
            )),
        })
        .unwrap();
    app.poll_messages();
    assert_eq!(app.checkpoints.data.stats.valid, 0);
    let old = app.begin_checkpoint_load();
    let current = app.begin_checkpoint_load();
    app.layer_tx
        .send(LayerLoad {
            request: old,
            update: LayerUpdate::Finished,
        })
        .unwrap();
    app.poll_messages();
    assert!(app.checkpoints.busy);
    app.layer_tx
        .send(LayerLoad {
            request: current.clone(),
            update: LayerUpdate::Data(CheckpointData::from_points(
                points(),
                CheckpointSource::PlatformApi,
                None,
                CacheState::Fresh,
            )),
        })
        .unwrap();
    app.layer_tx
        .send(LayerLoad {
            request: current,
            update: LayerUpdate::Finished,
        })
        .unwrap();
    app.poll_messages();
    data_shape(&app.checkpoints.data);
    assert!(!app.checkpoints.busy);
    app.session.as_mut().unwrap().token = "another-test-token".into();
    app.poll_messages();
    assert_eq!(app.checkpoints.data.stats.valid, 0);
}
#[test]
fn fence_and_policy_errors_do_not_erase_cached_checkpoint_layer() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    for failure in ["fence", "policy"] {
        let root = Root::new();
        let ctx = egui::Context::default();
        let replies = if failure == "fence" {
            vec![(
                crate::api::fence::FENCE_PATH,
                json!({"error":11016,"message":"fixture fence failure"}),
            )]
        } else {
            vec![
                (crate::api::fence::FENCE_PATH, fence(false)),
                (
                    crate::api::policy::POLICY_PATH,
                    json!({"error":11016,"message":"fixture policy failure"}),
                ),
            ]
        };
        let (host, server) = super::layout_tests::route_refresh_tests::mock_school(replies);
        let mut app = app(&root, &ctx, host);
        cache(&app);
        app.refresh_school_route(false);
        wait(&mut app);
        assert_eq!(
            server.join().unwrap(),
            if failure == "fence" { 1 } else { 2 }
        );
        data_shape(&app.checkpoints.data);
        assert!(app.network.is_none());
        assert!(!app.run_busy);
    }
}

#[test]
fn repeated_fresh_cache_refreshes_never_request_points_or_rewrite_the_cache() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    let root = Root::new();
    let ctx = egui::Context::default();
    let (host, server) = super::layout_tests::route_refresh_tests::mock_school(vec![
        (crate::api::fence::FENCE_PATH, fence(false)),
        (crate::api::fence::FENCE_PATH, fence(false)),
    ]);
    let mut app = app(&root, &ctx, host);
    cache(&app);
    let before = std::fs::read(root.0.join("points_cache.json")).unwrap();
    for _ in 0..2 {
        app.refresh_checkpoint_layer();
        wait(&mut app);
        data_shape(&app.checkpoints.data);
        assert_eq!(app.checkpoints.data.cache_state, CacheState::Fresh);
        assert_eq!(
            app.checkpoints.data.points[0].source,
            CheckpointSource::PlatformCache
        );
        assert!(app.checkpoints.data.error.is_none());
        assert!(app.checkpoints.last_response_stats.is_none());
    }
    assert_eq!(
        server.join().unwrap(),
        2,
        "only the two fence requests are expected"
    );
    assert_eq!(
        std::fs::read(root.0.join("points_cache.json")).unwrap(),
        before
    );
    assert!(app.network.is_none());
    assert!(app.run_page.preview_inputs.is_none());
}

#[test]
fn stale_cache_survives_invalid_api_rows_and_keeps_response_statistics_separate() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    let root = Root::new();
    let ctx = egui::Context::default();
    let (host, server) =
        super::layout_tests::route_refresh_tests::mock_school(api_replies(mixed_points()));
    let mut app = app(&root, &ctx, host);
    cache(&app);
    let stale_bytes = stamp_cache(
        &root,
        crate::crypto::envelope::now_ms() - model::POINTS_TTL_MS,
    );

    app.refresh_checkpoint_layer();
    wait(&mut app);

    assert_eq!(server.join().unwrap(), 2);
    data_shape(&app.checkpoints.data);
    assert_eq!(app.checkpoints.data.cache_state, CacheState::Stale);
    assert!(app
        .checkpoints
        .data
        .error
        .as_deref()
        .unwrap()
        .contains("无效坐标"));
    let stats = app.checkpoints.last_response_stats.as_ref().unwrap();
    assert_eq!((stats.raw, stats.valid, stats.invalid), (2, 1, 1));
    assert_eq!(app.checkpoints.data.stats.raw, 3);
    assert_eq!(
        std::fs::read(root.0.join("points_cache.json")).unwrap(),
        stale_bytes
    );
    assert!(app.network.is_none());
    assert!(app.run_page.preview_inputs.is_none());
}

#[test]
fn missing_cache_with_mixed_api_rows_displays_only_valid_map_coordinates_without_saving() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    let root = Root::new();
    let ctx = egui::Context::default();
    let (host, server) =
        super::layout_tests::route_refresh_tests::mock_school(api_replies(mixed_points()));
    let mut app = app(&root, &ctx, host);

    app.refresh_checkpoint_layer();
    wait(&mut app);

    assert_eq!(server.join().unwrap(), 2);
    assert_eq!(
        (
            app.checkpoints.data.stats.raw,
            app.checkpoints.data.stats.valid
        ),
        (2, 1)
    );
    assert_eq!(app.checkpoints.data.stats.invalid, 1);
    assert_eq!(app.checkpoints.data.coordinates().len(), 1);
    assert_eq!(
        app.checkpoints.data.points[0].source,
        CheckpointSource::PlatformApi
    );
    assert_eq!(
        app.checkpoints
            .last_response_stats
            .as_ref()
            .unwrap()
            .invalid,
        1
    );
    assert_eq!(app.checkpoints.data.cache_state, CacheState::Error);
    assert!(app
        .checkpoints
        .data
        .error
        .as_deref()
        .unwrap()
        .contains("无效坐标"));
    assert!(!root.0.join("points_cache.json").exists());
    assert!(app.network.is_none());
    assert!(app.run_page.preview_inputs.is_none());
    assert!(!app.run_busy);
}

#[test]
fn disappearing_same_scope_file_retains_memory_but_mismatched_scope_clears_it() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    for mismatch in [false, true] {
        let root = Root::new();
        let ctx = egui::Context::default();
        let replies = vec![
            (crate::api::fence::FENCE_PATH, fence(false)),
            (
                crate::api::fence::FENCE_PATH,
                json!({"error":11016,"message":"fixture fence failure"}),
            ),
        ];
        let (host, server) = super::layout_tests::route_refresh_tests::mock_school(replies);
        let mut app = app(&root, &ctx, host);
        cache(&app);

        app.refresh_checkpoint_layer();
        wait(&mut app);
        data_shape(&app.checkpoints.data);
        if mismatch {
            cache_scope(
                app.identity.anchor_coordinate().unwrap(),
                model::PointsCacheScope {
                    uid: 999,
                    unid: "77".into(),
                    run_area_id: None,
                },
                &points(),
            );
        } else {
            std::fs::remove_file(root.0.join("points_cache.json")).unwrap();
        }
        app.refresh_checkpoint_layer();
        wait(&mut app);

        assert_eq!(server.join().unwrap(), 2);
        if mismatch {
            assert_eq!(app.checkpoints.data.stats.valid, 0);
            assert!(
                app.checkpoints.scope_note.contains("不匹配"),
                "scope_note={:?}",
                app.checkpoints.scope_note
            );
        } else {
            data_shape(&app.checkpoints.data);
            assert_eq!(app.checkpoints.data.cache_state, CacheState::Missing);
            assert!(app
                .checkpoints
                .data
                .error
                .as_deref()
                .unwrap()
                .contains("fixture fence failure"));
        }
        assert!(app.network.is_none());
        assert!(app.run_page.preview_inputs.is_none());
    }
}

#[test]
fn fresh_display_transitions_to_stale_when_ttl_expires() {
    let root = Root::new();
    let ctx = egui::Context::default();
    let mut app = app_from_offline(&root, &ctx);
    app.checkpoints.data = CheckpointData::from_points(
        points(),
        CheckpointSource::PlatformCache,
        Some(crate::crypto::envelope::now_ms() - model::POINTS_TTL_MS),
        CacheState::Fresh,
    );

    app.poll_checkpoint_results();

    assert_eq!(app.checkpoints.data.cache_state, CacheState::Stale);
    assert_eq!(app.checkpoints.data.stats.valid, 3);
}

#[test]
fn fourth_uncached_refresh_is_limited_and_busy_repeats_do_not_start_more_requests() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    let root = Root::new();
    let ctx = egui::Context::default();
    let invalid = vec![json!({"id":3001,"lat":0,"lon":0})];
    let mut replies = Vec::new();
    for _ in 0..3 {
        replies.extend(api_replies(invalid.clone()));
    }
    replies.push((crate::api::fence::FENCE_PATH, fence(false)));
    let (host, server) = super::layout_tests::route_refresh_tests::mock_school(replies);
    let mut app = app(&root, &ctx, host);

    for _ in 0..3 {
        app.refresh_checkpoint_layer();
        wait(&mut app);
        assert!(!root.0.join("points_cache.json").exists());
    }
    app.refresh_checkpoint_layer();
    wait(&mut app);

    assert_eq!(server.join().unwrap(), 7);
    assert!(app
        .checkpoints
        .data
        .error
        .as_deref()
        .unwrap()
        .contains("300 秒内最多 3 次"));
    assert_eq!(app.checkpoints.data.stats.valid, 0);
    assert!(!root.0.join("points_cache.json").exists());
    assert!(app.network.is_none());
}

#[test]
fn repeated_busy_refresh_is_ignored_and_cancelled_late_reply_cannot_save_or_publish() {
    crate::runtime::TEST_OFFLINE.with(|v| v.set(Some(false)));
    let root = Root::new();
    let ctx = egui::Context::default();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (host, received, server) = mock_school_paused_points(release_rx);
    let mut app = app(&root, &ctx, host);

    app.refresh_checkpoint_layer();
    received.recv_timeout(Duration::from_secs(6)).unwrap();
    assert!(app.checkpoints.busy);
    app.refresh_checkpoint_layer();
    app.refresh_checkpoint_layer();
    assert!(app.checkpoints.busy);
    app.cancel_checkpoint_load();
    release_tx.send(()).unwrap();

    assert_eq!(server.join().unwrap(), 2);
    app.poll_messages();
    assert!(!app.checkpoints.busy);
    assert_eq!(app.checkpoints.data.stats.valid, 0);
    assert!(!root.0.join("points_cache.json").exists());
    assert!(app.network.is_none());
    assert!(app.run_page.preview_inputs.is_none());
    assert!(!app.run_busy);
}
