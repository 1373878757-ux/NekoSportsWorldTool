//! 校园点位：POST /api/v560/get/1/distance/1（sportType=4）。
//!
//! runec = 信封(f"{uid}{经度6位}{纬度6位}{起始时间秒级ms整}"，insert→observed 序列化)；
//! sign = MD5(http版URL + 盐)。带按账号、学校、区域及位置隔离的 300s 缓存。

use super::client::ApiClient;
use super::model;
use crate::crypto::envelope::{build_envelope, OuterOrder};
use crate::crypto::sign::md5_url_sign;
use crate::location::Coordinate;
use serde_json::{json, Value};

type PersistPointsCache<'a> =
    dyn FnMut(Coordinate, &model::PointsCacheScope, &[Value]) -> Result<(), String> + 'a;

pub const POINTS_PATH: &str = "/api/v560/get/1/distance/1";

/// 经纬度六位小数字符串。
fn six_digit(v: f64) -> String {
    format!("{v:.6}")
}

/// 拉取实时点位（仅使用有效期内缓存）。anchor=(lat,lng) 请求锚点。
pub fn fetch_points(
    client: &mut ApiClient,
    anchor: Coordinate,
    log: &mut dyn FnMut(&str),
) -> Result<Vec<Value>, String> {
    fetch_points_ext(client, anchor, None, log)
}

/// run_area_id：学校配置了区域时 App 会附带；未配置则不传（与 App 一致）。
pub fn fetch_points_ext(
    client: &mut ApiClient,
    anchor: Coordinate,
    run_area_id: Option<String>,
    log: &mut dyn FnMut(&str),
) -> Result<Vec<Value>, String> {
    fetch_points_ext_observed(
        client,
        anchor,
        run_area_id,
        log,
        &mut |_| {},
        &mut |anchor, scope, points| {
            // Keep the existing business API's best-effort cache persistence.
            let _ = model::save_points_cache(anchor, scope, points);
            Ok(())
        },
    )
}

/// Observe raw point rows for read-only diagnostics without weakening the business parser.
/// This is the same request/cache path; invalid rows still fail route/submit callers.
/// A cancellable caller must serialize cancellation and `persist` with the same gate,
/// then check request validity while holding it before saving. No cache is written
/// internally, so a rejected late response cannot overwrite the current cache.
pub(crate) fn fetch_points_ext_observed(
    client: &mut ApiClient,
    anchor: Coordinate,
    run_area_id: Option<String>,
    log: &mut dyn FnMut(&str),
    observe: &mut dyn FnMut(&[Value]),
    persist: &mut PersistPointsCache<'_>,
) -> Result<Vec<Value>, String> {
    anchor.validate()?;
    let login = client.login.as_ref().ok_or("请先登录后获取点位")?;
    let scope = model::PointsCacheScope {
        uid: login.uid,
        unid: login.unid.clone(),
        run_area_id: run_area_id.clone(),
    };
    // ① TTL 内命中缓存直接返回
    if let Some((ts, pts)) = model::load_points_cache_for(anchor, &scope) {
        log(&format!(
            "[points] 缓存命中（{} 秒前，{} 点）",
            (crate::crypto::envelope::now_ms() - ts) / 1000,
            pts.len()
        ));
        observe(&pts);
        return Ok(pts);
    }
    // ② 请求接口
    let uid = client.login.as_ref().map(|s| s.uid).unwrap_or(0);
    let unid = client
        .login
        .as_ref()
        .map(|s| s.unid.clone())
        .unwrap_or_else(|| "0".into());
    let lat = anchor.latitude;
    let lon = anchor.longitude;
    let url = format!("{}{}", model::HOST, POINTS_PATH);

    let start_ms = crate::crypto::envelope::now_ms();
    let runec_input = format!(
        "{uid}{}{}{}",
        six_digit(lon),
        six_digit(lat),
        (start_ms / 1000) * 1000
    );
    // runec：共用会话 Four，外层 observed 序
    let runec_env = build_envelope(&mut client.session, &runec_input, OuterOrder::Observed);
    let runec = runec_env.json;

    let mut body = json!({
        "sportType": 4,
        "longitude": lon,
        "latitude": lat,
        "sign": md5_url_sign(&url),
        "uuid": uuid::Uuid::new_v4().to_string(),
        "selectedUnid": unid,
        "runec": runec,
    });
    if let Some(area) = run_area_id {
        body["runAreaId"] = json!(area);
    }
    let body = body.to_string();

    let out = client.envelope_request("POST", &url, &body, crate::crypto::header::UA_IOS, &[])?;
    let dec = out.decrypted.ok_or("点位接口响应无法解密")?;
    let business = super::client::check_business(&dec.business)?;
    if let Some(points) =
        super::client::get_field(&business, "pointsResModels").and_then(Value::as_array)
    {
        observe(points);
    }
    let pts = parse_points(&dec.business)?;
    persist(anchor, &scope, &pts)?;
    Ok(pts)
}

pub(crate) fn parse_points(payload: &Value) -> Result<Vec<Value>, String> {
    let payload = super::client::check_business(payload)?;
    // pointsResModels：优先顶层，其次 data 子对象
    let pts: Vec<Value> = super::client::get_field(&payload, "pointsResModels")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    if pts.is_empty() {
        return Err("点位接口未返回有效点位列表".into());
    }
    if points_bd(&pts).len() != pts.len() {
        return Err("点位接口含无效坐标".into());
    }
    Ok(pts)
}

/// 点位中心（BD 系）。
#[allow(dead_code)]
pub fn center_bd(points: &[Value]) -> (f64, f64) {
    let n = points.len().max(1) as f64;
    let lat = points
        .iter()
        .filter_map(|p| p.get("lat").and_then(|v| v.as_f64()))
        .sum::<f64>()
        / n;
    let lon = points
        .iter()
        .filter_map(|p| p.get("lon").and_then(|v| v.as_f64()))
        .sum::<f64>()
        / n;
    (lat, lon)
}

/// 点位 → (lat, lon) BD 系数组（轨迹输入）。
pub fn points_bd(points: &[Value]) -> Vec<(f64, f64)> {
    points.iter().filter_map(super::fence::point_xy).collect()
}

/// Preserve policy order, then add distinct live points in a stable radial order.
pub fn route_points(required: &[(f64, f64)], live: &[Value]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for p in required
        .iter()
        .copied()
        .chain(crate::track::generate_road::radial_order(&points_bd(live)))
    {
        if !out
            .iter()
            .any(|q: &(f64, f64)| (q.0 - p.0).abs() < 1e-6 && (q.1 - p.1).abs() < 1e-6)
        {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod route_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    struct PointCacheFixture(std::path::PathBuf);
    impl PointCacheFixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("points-cancel-cache-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            crate::platform::TEST_DATA_DIR.with(|dir| *dir.borrow_mut() = Some(path.clone()));
            Self(path)
        }
    }
    impl Drop for PointCacheFixture {
        fn drop(&mut self) {
            crate::platform::TEST_DATA_DIR.with(|dir| *dir.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn read_request(stream: &mut std::net::TcpStream) {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 4096];
            let count = stream.read(&mut chunk).unwrap();
            assert_ne!(count, 0, "incomplete loopback request");
            bytes.extend_from_slice(&chunk[..count]);
            if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        assert!(headers.starts_with(&format!("POST {POINTS_PATH} HTTP/1.1\r\n")));
        let content_length: usize = headers
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().unwrap())
            })
            .unwrap();
        while bytes.len() < header_end + content_length {
            let mut chunk = [0_u8; 4096];
            let count = stream.read(&mut chunk).unwrap();
            assert_ne!(count, 0, "incomplete loopback request body");
            bytes.extend_from_slice(&chunk[..count]);
        }
    }

    #[test]
    fn cancelled_late_loopback_response_cannot_overwrite_newer_cache() {
        let fixture = PointCacheFixture::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let (arrived_tx, arrived_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            arrived_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let response = json!({
                "error":10000,
                "pointsResModels":[{"pointId":"late","lat":28.2,"lon":112.9}]
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        });

        let anchor = Coordinate {
            latitude: 28.2,
            longitude: 112.9,
            accuracy: 0.0,
        };
        let scope = model::PointsCacheScope {
            uid: 42,
            unid: "isolated-fixture".into(),
            run_area_id: Some("campus-new".into()),
        };
        let newest = json!({
            "pointId":"newest",
            "lat":28.21,
            "lon":112.91
        });
        let control = Arc::new(crate::task_control::Control::default());
        let commit_gate = Arc::new(Mutex::new(()));
        let root = fixture.0.clone();
        let worker_control = control.clone();
        let worker_gate = commit_gate.clone();
        let worker_scope = scope.clone();
        let worker = std::thread::spawn(move || {
            crate::platform::TEST_DATA_DIR.with(|dir| *dir.borrow_mut() = Some(root));
            crate::runtime::TEST_OFFLINE.with(|offline| offline.set(Some(false)));
            let mut client = ApiClient::new(
                crate::crypto::header::HeaderIdentity::default(),
                Some(model::Session {
                    uid: worker_scope.uid,
                    unid: worker_scope.unid.clone(),
                    token: "isolated-fixture".into(),
                    ..Default::default()
                }),
            );
            client.set_test_host(host);
            let mut persist =
                |anchor: Coordinate, scope: &model::PointsCacheScope, points: &[Value]| {
                    let _commit = worker_gate
                        .lock()
                        .map_err(|_| "cache commit lock poisoned")?;
                    worker_control.check()?;
                    model::save_points_cache(anchor, scope, points)
                };
            let result = fetch_points_ext_observed(
                &mut client,
                anchor,
                worker_scope.run_area_id.clone(),
                &mut |_| {},
                &mut |_| {},
                &mut persist,
            );
            crate::platform::TEST_DATA_DIR.with(|dir| *dir.borrow_mut() = None);
            crate::runtime::TEST_OFFLINE.with(|offline| offline.set(None));
            result
        });

        arrived_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        {
            // Serialize request cancellation with the late worker's cache commit.
            let _commit = commit_gate.lock().unwrap();
            assert!(control.cancel());
            model::save_points_cache(anchor, &scope, std::slice::from_ref(&newest)).unwrap();
        }
        release_tx.send(()).unwrap();
        let late_result = worker.join().unwrap();
        server.join().unwrap();
        assert!(late_result.unwrap_err().contains("已取消"));

        let snapshot = model::read_points_snapshot().unwrap().unwrap();
        assert_eq!(snapshot.points, vec![newest]);
        assert_eq!(snapshot.scope, scope);
    }

    #[test]
    fn direct_envelope_points_decode_json_text_business_data() {
        let point = json!({"lat":"39.9","lon":"116.4","isMustPoint":false});
        let data = json!({"pointsResModels":[point.clone()]});
        let response = json!({"error":10000,"data":data.to_string()});
        let points = parse_points(&response).unwrap();
        assert_eq!(points, vec![point]);
        assert_eq!(points_bd(&points), vec![(39.9, 116.4)]);
        assert!(parse_points(&json!({"error":10000,"data":"{broken"})).is_err());
    }

    #[test]
    fn response_errors_and_invalid_coordinates_are_not_cached_as_success() {
        let point = json!({"lat":39.9,"lon":116.4});
        assert_eq!(
            parse_points(&json!({"error":10000,"data":{"pointsResModels":[point.clone()]}}))
                .unwrap(),
            vec![point.clone()]
        );
        for response in [
            json!({"error":10603,"pointsResModels":[point]}),
            json!({"error":10000,"pointsResModels":[]}),
            json!({"error":10000,"pointsResModels":[{}]}),
        ] {
            assert!(parse_points(&response).is_err());
        }
    }
    #[test]
    fn first_required_point_and_order_survive_deduplication() {
        let first = (39.9, 116.4);
        let second = (39.91, 116.41);
        let live = vec![
            json!({"lat": 39.9,"lon":116.4}),
            json!({"lat":39.92,"lon":116.42}),
            json!({"lat":39.92,"lon":116.42}),
        ];
        assert_eq!(
            route_points(&[first, first, second], &live),
            vec![first, second, (39.92, 116.42)]
        );
    }
}
