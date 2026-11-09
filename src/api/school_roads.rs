//! School-scoped public OSM download. No account credentials are sent to Overpass.
use route_planner::RoadGraph;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 24 * 1024 * 1024;
const TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const ENDPOINT: &str = "https://overpass-api.de/api/interpreter";
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
const REQUEST_INTERVAL: Duration = Duration::from_secs(30);
static LAST_PUBLIC_REQUEST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

fn reserve_public_request(
    last: &std::sync::Mutex<Option<Instant>>,
    now: Instant,
) -> Result<(), String> {
    let mut previous = last
        .lock()
        .map_err(|_| "[road:rate-limit] 请求间隔状态不可用")?;
    if let Some(started) = *previous {
        let elapsed = now.saturating_duration_since(started);
        if elapsed < REQUEST_INTERVAL {
            let remaining = (REQUEST_INTERVAL - elapsed).as_secs() + 1;
            return Err(format!("[road:rate-limit] 为避免频繁请求公共地图服务，请等待 {remaining} 秒后再加载；不会自动重试"));
        }
    }
    *previous = Some(now);
    Ok(())
}

/// GUI file import uses the same OSM parser, with a bounded, local-only read.
#[cfg(any(feature = "gui", test))]
pub(crate) fn load_local_map(path: &str) -> Result<RoadGraph, String> {
    let path = Path::new(path);
    let file =
        std::fs::File::open(path).map_err(|e| format!("[road:parse] 地图文件无法读取：{e}"))?;
    let metadata = file
        .metadata()
        .map_err(|e| format!("[road:parse] 地图文件信息读取失败：{e}"))?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("[road:parse] 请选择不超过 24 MiB 的普通地图文件".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("[road:parse] 地图文件读取失败：{e}"))?;
    parse(&bytes)
}

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_OVERPASS: std::cell::RefCell<Option<(String, Duration)>> = const { std::cell::RefCell::new(None) };
}

pub fn validate_fences(fences: &[Vec<(f64, f64)>]) -> Result<(), String> {
    if fences.is_empty() {
        return Err("当前学校没有有效围栏，无法保证校内路线；请先确认学校围栏".into());
    }
    for ring in fences {
        if ring.len() < 3
            || ring.iter().any(|&(lat, lon)| {
                !lat.is_finite() || !lon.is_finite() || lat.abs() > 85.0 || lon.abs() > 180.0
            })
        {
            return Err("学校围栏坐标无效".into());
        }
        let (oy, ox) = ring[0];
        let area: f64 = ring
            .iter()
            .zip(ring.iter().cycle().skip(1))
            .map(|(&(y, x), &(v, u))| (x - ox) * (v - oy) - (u - ox) * (y - oy))
            .sum();
        if area.abs() < 1e-12 {
            return Err("学校围栏面积无效".into());
        }
    }
    Ok(())
}

// Invert the same forward transform used to align roads, avoiding mixed coordinate systems.
pub fn to_wgs(lat: f64, lon: f64) -> (f64, f64) {
    let (mut y, mut x) = (lat, lon);
    for _ in 0..16 {
        let (by, bx) = crate::track::geom::wgs84_to_bd09(y, x);
        y += lat - by;
        x += lon - bx;
    }
    (y, x)
}

fn query(fences: &[Vec<(f64, f64)>]) -> Result<String, String> {
    validate_fences(fences)?;
    query_coordinates(&fences.iter().flatten().copied().collect::<Vec<_>>(), 0.001)
}

fn query_coordinates(points: &[(f64, f64)], margin: f64) -> Result<String, String> {
    if points.is_empty()
        || points
            .iter()
            .any(|&(a, b)| !a.is_finite() || !b.is_finite() || a.abs() > 85. || b.abs() > 180.)
    {
        return Err("缺少有效点位，不能确定路网下载范围".into());
    }
    let (mut south, mut west, mut north, mut east) = (90f64, 180f64, -90f64, -180f64);
    for &(lat, lon) in points {
        let (y, x) = to_wgs(lat, lon);
        south = south.min(y);
        north = north.max(y);
        west = west.min(x);
        east = east.max(x);
    }
    // Reject unexpectedly huge / geographically separate school areas before downloading.
    if north - south > 0.15 || east - west > 0.2 {
        return Err("路网下载范围跨度过大，请确认当前学校和点位后重试".into());
    }
    let bbox = format!(
        "{:.7},{:.7},{:.7},{:.7}",
        south - margin,
        west - margin,
        north + margin,
        east + margin
    );
    Ok(format!("[out:xml][timeout:25];(way[highway]({bbox});way[building]({bbox});relation[building]({bbox}););(._;>;);out body;"))
}

fn parse(bytes: &[u8]) -> Result<RoadGraph, String> {
    parse_with_log(bytes, &mut |_| {})
}

fn parse_with_log(bytes: &[u8], log: &mut dyn FnMut(&str)) -> Result<RoadGraph, String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("[road:parse] 学校路网响应超过 24 MiB".into());
    }
    let xml = std::str::from_utf8(bytes).map_err(|_| "[road:parse] 学校路网不是有效 UTF-8 XML")?;
    if !xml.contains("<osm") || !xml.contains("</osm>") || xml.contains("<remark") {
        return Err("[road:parse] 路网服务返回错误或不完整 OSM XML，请检查响应内容".into());
    }
    let parsed = route_planner::parse_osm(bytes).map_err(|e| format!("[road:parse] {e}"))?;
    let stats = parsed.stats;
    log(&format!(
        "[road:parse] bytes={} nodes={} ways={} highway_ways={} retained_road_ways={} resolved_highway_ways={} unresolved_road_node_refs={}",
        bytes.len(), stats.parsed_nodes, stats.ways, stats.highway_ways,
        stats.retained_road_ways, stats.resolved_highway_ways, stats.unresolved_road_node_refs
    ));
    if parsed.roads.is_empty() {
        return Err(format!(
            "[road:graph] OSM 中未找到可用的 highway 道路（highway_ways={}，筛选后道路=0）；请核对查询范围和道路标签",
            stats.highway_ways
        ));
    }
    let net = route_planner::load::build_graph(&parsed);
    log_graph("graph", "WGS84", &net, log);
    if net.graph.node_weights().any(|n| {
        !n.coord.lon.is_finite()
            || !n.coord.lat.is_finite()
            || n.coord.lon.abs() > 180.0
            || n.coord.lat.abs() > 90.0
    }) {
        return Err("[road:graph] OSM 道路节点包含无效经纬度".into());
    }
    if net.graph.edge_count() == 0 {
        return Err(format!(
            "[road:graph] OSM 构图后没有可用道路边（筛选后道路={}，缺失节点引用={}）",
            stats.retained_road_ways, stats.unresolved_road_node_refs
        ));
    }
    Ok(net)
}

fn log_graph(stage: &str, crs: &str, net: &RoadGraph, log: &mut dyn FnMut(&str)) {
    let road_ways: std::collections::HashSet<_> =
        net.graph.edge_weights().map(|e| e.way_id).collect();
    log(&format!(
        "[road:{stage}] crs={crs} nodes={} edges={} road_ways={}",
        net.graph.node_count(),
        net.graph.edge_count(),
        road_ways.len()
    ));
}

fn network_error(action: &str, error: &(dyn std::error::Error + 'static)) -> String {
    // OS timeout messages are localized and differ between macOS and Windows.
    // Preserve the diagnostic text, but classify the underlying I/O error.
    let mut current = Some(error);
    while let Some(source) = current {
        if source
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::TimedOut)
        {
            return format!("{action}超时：{error}");
        }
        current = source.source();
    }
    format!("{action}失败：{error}")
}

fn response_summary(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(240)
        .collect()
}

fn download(query: &str, log: &mut dyn FnMut(&str)) -> Result<Vec<u8>, String> {
    crate::runtime::ensure_network_allowed()?;
    #[cfg(test)]
    let fixture = TEST_OVERPASS.with(|value| value.borrow().clone());
    #[cfg(test)]
    let (endpoint, timeout) = fixture
        .as_ref()
        .map(|(endpoint, timeout)| {
            assert!(endpoint.starts_with("http://127.0.0.1:"));
            (endpoint.as_str(), *timeout)
        })
        .unwrap_or((ENDPOINT, Duration::from_secs(45)));
    #[cfg(not(test))]
    let (endpoint, timeout) = (ENDPOINT, Duration::from_secs(45));
    if endpoint == ENDPOINT {
        reserve_public_request(&LAST_PUBLIC_REQUEST, Instant::now())?;
    }
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let started = Instant::now();
    log(&format!(
        "[road:http] request={request_id} 开始请求 Overpass timeout_ms={}",
        timeout.as_millis()
    ));
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = match agent
        .post(endpoint)
        .set("User-Agent", "NekoSportsWorldTool/0.3.0 (school-road-map)")
        .send_form(&[("data", query)])
    {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let mut body = Vec::new();
            let read = response.into_reader().take(1024).read_to_end(&mut body);
            let summary = response_summary(&body);
            let error = format!(
                "[road:http] request={request_id} 自动获取学校路网失败：HTTP {status} elapsed_ms={} 响应摘要={}{}",
                started.elapsed().as_millis(),
                if summary.is_empty() { "（空）" } else { &summary },
                read.err().map(|e| format!("；错误响应读取失败：{e}")).unwrap_or_default()
            );
            log(&error);
            return Err(error);
        }
        Err(error) => {
            let error = format!(
                "[road:http] request={request_id} {} elapsed_ms={}",
                network_error("自动获取学校路网", &error),
                started.elapsed().as_millis()
            );
            log(&error);
            return Err(error);
        }
    };
    let status = response.status();
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            let error = format!(
                "[road:http] request={request_id} HTTP {status} {} elapsed_ms={}",
                network_error("读取学校路网", &e),
                started.elapsed().as_millis()
            );
            log(&error);
            error
        })?;
    log(&format!(
        "[road:http] request={request_id} HTTP {status} bytes={} elapsed_ms={}",
        bytes.len(),
        started.elapsed().as_millis()
    ));
    Ok(bytes)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Cache {
    key: String,
    loaded_ms: i64,
    xml: String,
    #[serde(default)]
    school: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    fences: Vec<Vec<(f64, f64)>>,
    #[serde(default)]
    points: Vec<(f64, f64)>,
}

fn cached_network(
    dir: &Path,
    school: &str,
    fences: &[Vec<(f64, f64)>],
    refresh: bool,
    now: i64,
    fetch: impl FnOnce(&str, &mut dyn FnMut(&str)) -> Result<Vec<u8>, String>,
    log: &mut dyn FnMut(&str),
) -> Result<RoadGraph, String> {
    cached_route_network(dir, school, fences, &[], refresh, now, fetch, log)
}

fn cached_route_network(
    dir: &Path,
    school: &str,
    fences: &[Vec<(f64, f64)>],
    points: &[(f64, f64)],
    refresh: bool,
    now: i64,
    fetch: impl FnOnce(&str, &mut dyn FnMut(&str)) -> Result<Vec<u8>, String>,
    log: &mut dyn FnMut(&str),
) -> Result<RoadGraph, String> {
    if school.trim().is_empty() {
        return Err("登录信息缺少学校编号，请重新登录".into());
    }
    let q = if fences.is_empty() {
        query_coordinates(points, 0.002)?
    } else {
        query(fences)?
    };
    let key = route_scope_key(school, fences, points);
    let file = dir.join(format!("{key}.json"));
    if !refresh
        && std::fs::metadata(&file)
            .map(|m| m.len() < MAX_BYTES * 2)
            .unwrap_or(false)
    {
        if let Ok(text) = std::fs::read_to_string(&file) {
            if let Ok(cache) = serde_json::from_str::<Cache>(&text) {
                if cache.key == key && (0..TTL_MS).contains(&(now - cache.loaded_ms)) {
                    match parse_with_log(cache.xml.as_bytes(), log) {
                        Ok(net) => {
                            log("[road:cache] 使用当前学校及下载范围对应的有效路网缓存；不发送 HTTP 请求");
                            return Ok(net);
                        }
                        Err(error) => log(&format!("[road:cache] 缓存不可用：{error}")),
                    }
                }
            }
        }
    }
    log("[road] 正在下载当前学校区域的 OpenStreetMap 路网…");
    let bytes = fetch(&q, log)?;
    let net = parse_with_log(&bytes, log)?;
    // Reject empty in-school coverage before accepting a download into the cache.
    let mut aligned = net.clone();
    crate::track::generate_road::align_network(&mut aligned);
    log_graph("align", "BD-09", &aligned, log);
    let clipped = crate::track::generate_road::apply_fences(&aligned, fences);
    log(&format!(
        "[road:clip] fence_rings={}（0 表示未施加校园边界）",
        fences.len()
    ));
    log_graph("clip", "BD-09", &clipped, log);
    if clipped.graph.edge_count() == 0 {
        return Err(format!(
            "[road:clip] 下载的路网在学校围栏内没有可用道路（裁剪前 nodes={} edges={}，裁剪后 nodes={} edges=0）；请核对 OSM 覆盖、边界和坐标对齐，不能向校外补足距离",
            aligned.graph.node_count(), aligned.graph.edge_count(), clipped.graph.node_count()
        ));
    }
    let cache = Cache {
        key,
        loaded_ms: now,
        xml: String::from_utf8(bytes).map_err(|e| e.to_string())?,
        school: school.into(),
        name: String::new(),
        fences: fences.to_vec(),
        points: points.to_vec(),
    };
    // Cache is optional: read-only installation directories must not block a valid download.
    let saved = (|| -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        // Interrupted/concurrent writes are rejected by the parser on the next read.
        std::fs::write(
            &file,
            serde_json::to_vec(&cache).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = saved {
        log(&format!("[road] 本次路网可用，但缓存未保存：{e}"));
    }
    Ok(net)
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub key: String,
    pub school: String,
    pub name: String,
    pub loaded_ms: i64,
    pub bytes: u64,
    pub fences: Vec<Vec<(f64, f64)>>,
    pub points: Vec<(f64, f64)>,
}
pub fn scope_key(school: &str, fences: &[Vec<(f64, f64)>]) -> String {
    // About centimetre precision: stable across JSON floating-point round trips.
    let normalized: Vec<Vec<(i64, i64)>> = fences
        .iter()
        .map(|f| {
            f.iter()
                .map(|&(a, b)| ((a * 1e7).round() as i64, (b * 1e7).round() as i64))
                .collect()
        })
        .collect();
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(school, normalized)).unwrap())
    )
}

fn route_scope_key(school: &str, fences: &[Vec<(f64, f64)>], points: &[(f64, f64)]) -> String {
    if !fences.is_empty() {
        return scope_key(school, fences);
    }
    let normalized: Vec<_> = points
        .iter()
        .map(|&(a, b)| ((a * 1e7).round() as i64, (b * 1e7).round() as i64))
        .collect();
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&("points-v1", school, normalized)).unwrap())
    )
}
fn cache_dir() -> std::path::PathBuf {
    crate::platform::data_dir().join("school_roads")
}
fn cache_path(key: &str) -> Result<std::path::PathBuf, String> {
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("无效缓存标识".into());
    }
    Ok(cache_dir().join(format!("{key}.json")))
}
pub fn entries() -> Vec<Entry> {
    let Ok(files) = std::fs::read_dir(cache_dir()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for f in files.flatten() {
        let Some(key) = f
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if let Ok((cache, bytes)) = read_cache_record(&key) {
            out.push(cache_entry(&cache, bytes));
        }
    }
    out.sort_by(|a, b| b.loaded_ms.cmp(&a.loaded_ms));
    out
}
fn read_cache_record(key: &str) -> Result<(Cache, u64), String> {
    let path = cache_path(key)?;
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_BYTES * 2 {
        return Err("缓存过大".into());
    }
    let cache: Cache = serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if cache.key != key {
        return Err("缓存标识不一致".into());
    }
    Ok((cache, meta.len()))
}
fn cache_entry(c: &Cache, bytes: u64) -> Entry {
    Entry {
        key: c.key.clone(),
        school: c.school.clone(),
        name: c.name.clone(),
        loaded_ms: c.loaded_ms,
        bytes,
        fences: c.fences.clone(),
        points: c.points.clone(),
    }
}
pub fn read_cache(key: &str) -> Result<(RoadGraph, Entry), String> {
    let (c, bytes) = read_cache_record(key)?;
    Ok((parse(c.xml.as_bytes())?, cache_entry(&c, bytes)))
}
pub fn clear_cache(key: &str) -> Result<(), String> {
    std::fs::remove_file(cache_path(key)?).map_err(|e| e.to_string())
}
pub fn save_context(school: &str, name: &str, fences: &[Vec<(f64, f64)>], points: &[(f64, f64)]) {
    let key = route_scope_key(school, fences, points);
    let Ok(path) = cache_path(&key) else {
        return;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    if let Ok(mut cache) = serde_json::from_slice::<Cache>(&bytes) {
        cache.name = name.into();
        cache.points = points.to_vec();
        if let Ok(bytes) = serde_json::to_vec(&cache) {
            let _ = std::fs::write(path, bytes);
        }
    }
}

pub fn load_for_school_cancellable(
    path: &str,
    school: &str,
    fences: &[Vec<(f64, f64)>],
    refresh: bool,
    control: &crate::task_control::Control,
    log: &mut dyn FnMut(&str),
) -> Result<RoadGraph, String> {
    control.check()?;
    if !path.trim().is_empty() {
        let net = crate::track::generate_road::load_network_path(path)?;
        control.check()?;
        return Ok(net);
    }
    validate_fences(fences)?;
    cached_network(
        &cache_dir(),
        school,
        fences,
        refresh,
        crate::crypto::envelope::now_ms(),
        |q, log| {
            control.check()?;
            let bytes = download(q, log)?;
            control.check()?;
            Ok(bytes)
        },
        log,
    )
}

/// With no server boundary, download near real school checkpoints. This extent
/// is only a public-map query, never a campus fence or a route constraint.
pub fn load_for_route_cancellable(
    path: &str,
    school: &str,
    fences: &[Vec<(f64, f64)>],
    points: &[(f64, f64)],
    refresh: bool,
    control: &crate::task_control::Control,
    log: &mut dyn FnMut(&str),
) -> Result<RoadGraph, String> {
    if !path.trim().is_empty() || !fences.is_empty() {
        return load_for_school_cancellable(path, school, fences, refresh, control, log);
    }
    control.check()?;
    log("[road] 无学校围栏：按当前学校真实点位获取附近道路；下载范围不是学校边界");
    cached_route_network(
        &cache_dir(),
        school,
        fences,
        points,
        refresh,
        crate::crypto::envelope::now_ms(),
        |q, log| {
            control.check()?;
            let bytes = download(q, log)?;
            control.check()?;
            Ok(bytes)
        },
        log,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_request_interval_is_shared_and_does_not_retry_or_extend_on_rejection() {
        let gate = std::sync::Mutex::new(None);
        let start = Instant::now();
        assert!(reserve_public_request(&gate, start).is_ok());
        assert!(
            reserve_public_request(&gate, start + Duration::from_secs(1))
                .unwrap_err()
                .contains("[road:rate-limit]")
        );
        assert!(reserve_public_request(&gate, start + Duration::from_secs(29)).is_err());
        assert!(reserve_public_request(&gate, start + REQUEST_INTERVAL).is_ok());
    }

    #[test]
    fn local_import_checks_size_contents_and_coordinates_without_http() {
        let root = std::env::temp_dir().join(format!("map-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("地图.osm");
        std::fs::write(&path, fixture().1).unwrap();
        assert_eq!(
            load_local_map(path.to_str().unwrap())
                .unwrap()
                .graph
                .edge_count(),
            2
        );
        for invalid in [
            b"<html>not a map</html>".as_slice(),
            b"\0\xffPBF",
            b"<osm></osm>",
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert!(load_local_map(path.to_str().unwrap()).is_err());
        }
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        assert!(load_local_map(path.to_str().unwrap())
            .err()
            .unwrap()
            .contains("24 MiB"));
        assert!(load_local_map(root.to_str().unwrap()).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    struct LoopbackOverride {
        overpass: Option<(String, Duration)>,
        offline: Option<bool>,
    }

    impl LoopbackOverride {
        fn new(endpoint: String, timeout: Duration) -> Self {
            assert!(endpoint.starts_with("http://127.0.0.1:"));
            let overpass = TEST_OVERPASS.with(|value| value.replace(Some((endpoint, timeout))));
            let offline = crate::runtime::TEST_OFFLINE.with(|value| value.replace(Some(false)));
            Self { overpass, offline }
        }
    }

    impl Drop for LoopbackOverride {
        fn drop(&mut self) {
            TEST_OVERPASS.with(|value| *value.borrow_mut() = self.overpass.take());
            crate::runtime::TEST_OFFLINE.with(|value| value.set(self.offline));
        }
    }

    fn loopback_response(
        status: &str,
        body: &[u8],
        delay: Duration,
    ) -> (String, std::thread::JoinHandle<usize>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/api/interpreter", listener.local_addr().unwrap());
        let status = status.to_owned();
        let body = body.to_vec();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let n = stream.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..n]);
                let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..end]);
                let content_length = headers
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if request.len() >= end + 4 + content_length {
                    break;
                }
            }
            assert!(request.starts_with(b"POST /api/interpreter "));
            assert!(String::from_utf8_lossy(&request).contains("data="));
            std::thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            // A deliberately timed-out client may already have closed its socket.
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
            1
        });
        (endpoint, server)
    }

    fn fixture() -> (Vec<Vec<(f64, f64)>>, Vec<u8>) {
        let fences = vec![vec![
            (39.899, 116.399),
            (39.899, 116.403),
            (39.903, 116.403),
            (39.903, 116.399),
        ]
        .into_iter()
        .map(|(a, b)| crate::track::geom::wgs84_to_bd09(a, b))
        .collect()];
        let xml=br#"<osm version="0.6"><node id="1" lat="39.9" lon="116.4"/><node id="2" lat="39.901" lon="116.401"/><way id="1"><nd ref="1"/><nd ref="2"/><tag k="highway" v="footway"/></way></osm>"#.to_vec();
        (fences, xml)
    }

    #[test]
    fn loopback_502_is_reported_without_an_automatic_retry() {
        let (endpoint, server) =
            loopback_response("502 Bad Gateway", b"fixture gateway error", Duration::ZERO);
        let _guard = LoopbackOverride::new(endpoint, Duration::from_secs(2));
        let mut logs = Vec::new();
        let error = download("[out:xml];out;", &mut |s| logs.push(s.to_owned())).unwrap_err();
        assert!(error.contains("502"), "{error}");
        assert!(error.starts_with("[road:http]"), "{error}");
        assert!(error.contains("fixture gateway error"), "{error}");
        assert!(logs.iter().any(|s| s.contains("elapsed_ms=")));
        assert_eq!(server.join().unwrap(), 1);
    }

    #[test]
    fn loopback_timeout_finishes_without_waiting_for_the_server() {
        let (endpoint, server) =
            loopback_response("200 OK", b"<osm></osm>", Duration::from_millis(250));
        let _guard = LoopbackOverride::new(endpoint, Duration::from_millis(50));
        let started = std::time::Instant::now();
        let error = download("[out:xml];out;", &mut |_| {}).unwrap_err();
        assert!(error.contains("自动获取学校路网超时"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(server.join().unwrap(), 1);
    }

    #[test]
    fn timeout_kind_does_not_depend_on_the_operating_system_message() {
        let timeout: ureq::Error =
            std::io::Error::new(std::io::ErrorKind::TimedOut, "localized socket message").into();
        let message = network_error("自动获取学校路网", &timeout);
        assert!(message.starts_with("自动获取学校路网超时："));
        assert!(message.contains("localized socket message"));
        assert!(crate::road_status::explain(&message).starts_with("[网络超时]"));

        let refused: ureq::Error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused").into();
        assert!(network_error("自动获取学校路网", &refused).starts_with("自动获取学校路网失败："));
    }

    #[test]
    fn loopback_empty_osm_is_not_accepted_into_the_cache() {
        let dir = std::env::temp_dir().join(format!("empty-road-{}", uuid::Uuid::new_v4()));
        let (endpoint, server) = loopback_response("200 OK", b"<osm></osm>", Duration::ZERO);
        let _guard = LoopbackOverride::new(endpoint, Duration::from_secs(2));
        let error = cached_network(
            &dir,
            "fixture-school",
            &fixture().0,
            false,
            1000,
            download,
            &mut |_| {},
        )
        .err()
        .unwrap();
        assert!(error.contains("未找到可用的 highway 道路"), "{error}");
        assert!(!dir.exists());
        assert_eq!(server.join().unwrap(), 1);
    }

    #[test]
    fn loopback_downloaded_roads_are_reused_without_a_second_request() {
        let dir = std::env::temp_dir().join(format!("cached-road-{}", uuid::Uuid::new_v4()));
        let (fences, xml) = fixture();
        let (endpoint, server) = loopback_response("200 OK", &xml, Duration::ZERO);
        let _guard = LoopbackOverride::new(endpoint, Duration::from_secs(2));
        let first = cached_network(
            &dir,
            "fixture-school",
            &fences,
            false,
            1000,
            download,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(server.join().unwrap(), 1);
        let second = cached_network(
            &dir,
            "fixture-school",
            &fences,
            false,
            1001,
            |_, _| panic!("a fresh road cache must not send another request"),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(first.graph.edge_count(), second.graph.edge_count());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn parsing_errors_empty_highways_and_missing_nodes_have_separate_diagnostics() {
        for xml in [b"not XML".as_slice(), b"<osm><way></osm>"] {
            assert!(parse(xml).err().unwrap().starts_with("[road:parse]"));
        }
        let mut logs = Vec::new();
        let error = parse_with_log(b"<osm></osm>", &mut |s| logs.push(s.to_owned()))
            .err()
            .unwrap();
        assert!(error.starts_with("[road:graph]"));
        assert!(logs.iter().any(|s| s.contains("highway_ways=0")));

        let missing = br#"<osm><node id="1" lat="39.9" lon="116.4"/>
          <way id="1"><nd ref="1"/><nd ref="99"/><tag k="highway" v="footway"/></way></osm>"#;
        let error = parse(missing).err().unwrap();
        assert!(error.starts_with("[road:graph]"), "{error}");
        assert!(error.contains("缺失节点引用=1"), "{error}");

        let filtered = br#"<osm><node id="1" lat="39.9" lon="116.4"/>
          <node id="2" lat="39.901" lon="116.401"/>
          <way id="1"><nd ref="1"/><nd ref="2"/><tag k="highway" v="motorway"/></way></osm>"#;
        assert!(parse(filtered).err().unwrap().contains("highway_ways=1"));
    }

    #[test]
    fn loaded_graph_and_empty_boundary_clip_are_logged_without_changing_coordinates() {
        let dir = std::env::temp_dir().join(format!("road-stage-{}", uuid::Uuid::new_v4()));
        let (fences, xml) = fixture();
        let mut logs = Vec::new();
        let net = cached_network(
            &dir,
            "diagnostic-school",
            &fences,
            false,
            1000,
            |_, _| Ok(xml.clone()),
            &mut |s| logs.push(s.to_owned()),
        )
        .unwrap();
        for expected in [
            "highway_ways=1 retained_road_ways=1",
            "[road:graph] crs=WGS84 nodes=2 edges=2",
            "[road:align] crs=BD-09 nodes=2 edges=2",
            "[road:clip] crs=BD-09 nodes=2 edges=2",
        ] {
            assert!(logs.iter().any(|s| s.contains(expected)), "{logs:?}");
        }
        assert_eq!(net.graph.node_weights().next().unwrap().coord.lat, 39.9);
        let outside = vec![vec![
            (39.91, 116.41),
            (39.91, 116.414),
            (39.914, 116.414),
            (39.914, 116.41),
        ]
        .into_iter()
        .map(|(lat, lon)| crate::track::geom::wgs84_to_bd09(lat, lon))
        .collect()];
        let key = route_scope_key("diagnostic-school", &outside, &[]);
        logs.clear();
        let error = cached_network(
            &dir,
            "diagnostic-school",
            &outside,
            false,
            1001,
            |_, _| Ok(xml),
            &mut |s| logs.push(s.to_owned()),
        )
        .err()
        .unwrap();
        assert!(error.starts_with("[road:clip]"), "{error}");
        assert!(error.contains("裁剪前 nodes=2 edges=2"), "{error}");
        assert!(error.contains("裁剪后 nodes=0 edges=0"), "{error}");
        assert!(!dir.join(format!("{key}.json")).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn diagnostic_response_summary_is_bounded_and_has_no_control_characters() {
        let body = format!("\n\r\t服务容量已满 {}\u{1b}[0m", "测试".repeat(500));
        let summary = response_summary(body.as_bytes());
        assert!(summary.starts_with("服务容量已满 "));
        assert_eq!(summary.chars().count(), 240);
        assert!(!summary.chars().any(char::is_control));
    }

    #[test]
    fn offline_diagnostic_download_stops_before_transport_and_request_log() {
        let previous = crate::runtime::TEST_OFFLINE.with(|value| value.replace(Some(true)));
        let mut logs = Vec::new();
        let result = download("unused", &mut |s| logs.push(s.to_owned()));
        crate::runtime::TEST_OFFLINE.with(|value| value.set(previous));
        assert_eq!(result.unwrap_err(), crate::runtime::OFFLINE_MESSAGE);
        assert!(logs.is_empty());
    }
    #[test]
    fn coordinate_round_trip_and_query_scope() {
        for p in [(39.9, 116.4), (22.5, 113.9), (31.2, 121.5)] {
            let bd = crate::track::geom::wgs84_to_bd09(p.0, p.1);
            let w = to_wgs(bd.0, bd.1);
            assert!((w.0 - p.0).abs() < 1e-8 && (w.1 - p.1).abs() < 1e-8);
        }
        let q = query(&fixture().0).unwrap();
        assert!(q.contains("39.8980000,116.3980000,39.9040000,116.4040000"));
        assert!(query(&[]).is_err());
        assert!(validate_fences(&[vec![(0., 0.); 3]]).is_err());
    }
    #[test]
    fn cache_is_school_and_fence_scoped_expires_and_propagates_failure() {
        let dir = std::env::temp_dir().join(format!("school-roads-{}", uuid::Uuid::new_v4()));
        let (f, xml) = fixture();
        let get =
            |school: &str, fences: &[Vec<(f64, f64)>], refresh, now, bytes: Option<Vec<u8>>| {
                cached_network(
                    &dir,
                    school,
                    fences,
                    refresh,
                    now,
                    |_, _| bytes.ok_or("offline".into()),
                    &mut |_| {},
                )
            };
        assert!(get("a", &f, false, 1000, Some(xml.clone())).is_ok());
        assert!(get("a", &f, false, 1001, None).is_ok());
        assert!(get("b", &f, false, 1001, None).is_err());
        let mut changed = f.clone();
        changed[0][0].0 -= 0.00001;
        assert!(get("a", &changed, false, 1001, None).is_err());
        assert!(get("a", &f, false, 1000 + TTL_MS, None).is_err());
        assert!(get("a", &f, false, 999, None).is_err());
        assert!(get("a", &f, true, 1001, None).is_err());
        assert!(get("a", &f, true, 1002, Some(xml)).is_ok());
        assert!(get("a", &f, false, 1002 + TTL_MS - 1, None).is_ok());
        assert!(parse(b"<osm></osm>").is_err());
        assert!(parse(b"<osm><remark>timeout</remark></osm>").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn no_boundary_download_is_point_scoped_without_inventing_fences() {
        let dir = std::env::temp_dir().join(format!("point-roads-{}", uuid::Uuid::new_v4()));
        let (_, xml) = fixture();
        let points = [(39.9, 116.4), (39.901, 116.401)]
            .map(|(a, b)| crate::track::geom::wgs84_to_bd09(a, b));
        let net = cached_route_network(
            &dir,
            "a",
            &[],
            &points,
            false,
            1000,
            |q, _| {
                assert!(q.contains("39.8980000,116.3980000,39.9030000,116.4030000"));
                Ok(xml)
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(net.graph.edge_count() > 0);
        assert!(net.boundaries.is_empty());
        let key = route_scope_key("a", &[], &points);
        let cache: Cache =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{key}.json"))).unwrap())
                .unwrap();
        assert!(cache.fences.is_empty());
        assert_eq!(cache.points.len(), 2);
        let cached = |school, pts: &[(f64, f64)], now| {
            cached_route_network(
                &dir,
                school,
                &[],
                pts,
                false,
                now,
                |_, _| Err("offline".into()),
                &mut |_| {},
            )
        };
        assert!(cached("a", &points, 1001).is_ok());
        assert!(cached("b", &points, 1001).is_err());
        assert!(cached("a", &points[..1], 1001).is_err());
        assert!(cached("a", &points, 1000 + TTL_MS).is_err());
        assert!(query_coordinates(&[], 0.002).is_err());
        assert!(query_coordinates(&[(f64::NAN, 116.4)], 0.002).is_err());
        assert!(query_coordinates(&[(39., 116.), (40., 116.)], 0.002).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn manual_map_loading_needs_neither_boundary_nor_school_id() {
        let path = std::env::temp_dir().join(format!("manual-road-{}.osm", uuid::Uuid::new_v4()));
        std::fs::write(&path, fixture().1).unwrap();
        let net = load_for_route_cancellable(
            path.to_str().unwrap(),
            "",
            &[],
            &[],
            false,
            &crate::task_control::Control::default(),
            &mut |_| {},
        )
        .unwrap();
        assert!(net.graph.edge_count() > 0);
        assert!(net.boundaries.is_empty());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn saved_maps_can_be_listed_loaded_offline_and_deleted_safely() {
        let root = std::env::temp_dir().join(format!("school-manager-{}", uuid::Uuid::new_v4()));
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(root.clone()));
        let (fences, xml) = fixture();
        cached_network(
            &root.join("school_roads"),
            "test-school",
            &fences,
            false,
            1000,
            |_, _| Ok(xml),
            &mut |_| {},
        )
        .unwrap();
        save_context("test-school", "接口校区名", &fences, &[(39.9, 116.4)]);
        let list = entries();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "接口校区名");
        assert!(list[0].bytes > 0);
        let (_, entry) = read_cache(&list[0].key).unwrap();
        assert_eq!(scope_key("", &entry.fences), scope_key("", &fences));
        assert_eq!(entry.points.len(), 1);
        assert!(clear_cache("../other").is_err());
        clear_cache(&list[0].key).unwrap();
        assert!(entries().is_empty());
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = None);
        std::fs::remove_dir_all(root).unwrap();
    }
}
