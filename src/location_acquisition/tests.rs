use super::*;

const NOW: i64 = 1_799_999_999_000;

fn position(latitude: f64, longitude: f64, accuracy: f64, timestamp: i64) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "kind": "position", "latitude": latitude, "longitude": longitude,
        "accuracy": accuracy, "timestamp_ms": timestamp
    }))
    .unwrap()
}

#[test]
fn positions_in_different_cities_convert_once_without_changing_the_raw_fix() {
    for (latitude, longitude) in [(45.75, 126.63), (38.914, 121.614), (23.13, 113.26)] {
        let fix = parse_reply(&position(latitude, longitude, 15.0, NOW), NOW).unwrap();
        assert_eq!(fix.wgs84.latitude, latitude);
        assert_eq!(fix.wgs84.longitude, longitude);
        let converted = fix.platform_coordinate().unwrap();
        let expected = crate::track::geom::wgs84_to_bd09(latitude, longitude);
        assert_eq!((converted.latitude, converted.longitude), expected);
        assert_eq!(converted.accuracy, 15.0);
        assert_eq!(fix.platform_coordinate().unwrap(), converted);
    }
}

#[test]
fn invalid_stale_future_and_malformed_fixes_are_rejected() {
    for bytes in [
        position(91.0, 120.0, 10.0, NOW),
        position(45.0, 181.0, 10.0, NOW),
        position(45.0, 120.0, -1.0, NOW),
        position(45.0, 120.0, 10.0, NOW - MAX_FIX_AGE_MS - 1),
        position(45.0, 120.0, 10.0, NOW + MAX_CLOCK_AHEAD_MS + 1),
        position(45.0, 120.0, 10.0, 0),
        b"{broken".to_vec(),
        b"{\"kind\":\"position\",\"latitude\":null}".to_vec(),
    ] {
        assert!(parse_reply(&bytes, NOW).is_err());
    }
}

#[test]
fn browser_permission_unavailable_and_timeout_have_distinct_errors() {
    for (code, expected) in [
        (1, "权限"),
        (2, "无法提供位置"),
        (3, "超时"),
        (0, "不支持定位"),
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({"kind":"error", "code":code})).unwrap();
        assert!(parse_reply(&bytes, NOW).unwrap_err().contains(expected));
    }
}

fn send(
    request: &BrowserLocationRequest,
    method: &str,
    suffix: &str,
    origin: &str,
    body: &[u8],
) -> String {
    let url = request.url.strip_prefix("http://").unwrap();
    let (host, path) = url.split_once('/').unwrap();
    let mut stream = TcpStream::connect(host).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let header = format!(
        "{method} /{path}{suffix} HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    reply
}

fn origin(request: &BrowserLocationRequest) -> String {
    format!(
        "http://{}",
        request
            .url
            .strip_prefix("http://")
            .unwrap()
            .split('/')
            .next()
            .unwrap()
    )
}

#[test]
fn one_shot_callback_requires_correct_path_origin_and_host_then_closes() {
    let request = BrowserLocationRequest::listen(Duration::from_secs(5), || {}).unwrap();
    let origin = origin(&request);
    let body = position(45.75, 126.63, 15.0, now_ms());
    let page = send(&request, "GET", "", &origin, b"");
    assert!(page.starts_with("HTTP/1.1 200"));
    assert!(page.contains(crate::presentation::APP_NAME));
    assert!(!page.contains("{{NONCE}}"));
    assert!(page.contains("Content-Security-Policy:"));
    assert!(send(&request, "POST", "wrong", &origin, &body).starts_with("HTTP/1.1 404"));
    assert!(send(
        &request,
        "POST",
        "result",
        "https://untrusted.invalid",
        &body
    )
    .starts_with("HTTP/1.1 403"));
    assert!(request.try_result().is_none());
    assert!(send(&request, "POST", "result", &origin, &body).starts_with("HTTP/1.1 200"));
    let result = request
        .receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(result.wgs84.latitude, 45.75);
    let host = origin.strip_prefix("http://").unwrap();
    assert!(TcpStream::connect(host).is_err());
}

#[test]
fn cancellation_drops_listener_and_late_reply_cannot_deliver() {
    let request = BrowserLocationRequest::listen(Duration::from_secs(5), || {}).unwrap();
    let worker = request.worker.clone();
    let host = origin(&request).strip_prefix("http://").unwrap().to_owned();
    request.cancelled.store(true, Ordering::Release);
    worker.unpark();
    assert!(matches!(
        request.receiver.recv_timeout(Duration::from_secs(2)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(TcpStream::connect(host).is_err());
}

#[test]
fn request_expires_without_retrying_or_waiting_for_a_browser() {
    let request = BrowserLocationRequest::listen(Duration::from_millis(20), || {}).unwrap();
    assert!(request
        .receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap_err()
        .contains("超时"));
}

#[test]
fn duplicate_security_headers_are_rejected_and_page_requests_no_external_assets() {
    assert_eq!(
        header_value("GET / HTTP/1.1\r\nHost: a\r\nhost: b", "host"),
        None
    );
    assert_eq!(
        header_value("POST / HTTP/1.1\r\nOrigin: a\r\nOrigin: a", "origin"),
        None
    );
    assert!(PAGE.contains("getCurrentPosition"));
    assert!(PAGE.contains("maximumAge: 0"));
    assert!(PAGE.contains("timeout: 20000"));
    assert!(!PAGE.contains("watchPosition"));
    assert!(!PAGE.contains("https://"));
    assert!(!PAGE.contains("http://"));
}

#[test]
fn offline_mode_prevents_even_starting_a_location_listener() {
    let previous = crate::runtime::TEST_OFFLINE.with(|flag| flag.replace(Some(true)));
    let result = BrowserLocationRequest::start(|| panic!("offline request must not start"));
    crate::runtime::TEST_OFFLINE.with(|flag| flag.set(previous));
    assert!(matches!(result, Err(error) if error.contains("离线")));
}

#[test]
fn callback_rejects_rebound_host_oversized_bodies_and_chunked_requests() {
    let request = BrowserLocationRequest::listen(Duration::from_secs(5), || {}).unwrap();
    let url = request.url.strip_prefix("http://").unwrap();
    let (host, path) = url.split_once('/').unwrap();
    for (headers, expected) in [
        ("Host: untrusted.invalid\r\nContent-Length: 0".to_owned(), "403"),
        (format!("Host: {host}\r\nContent-Length: 4096"), "400"),
        (format!("Host: {host}\r\nTransfer-Encoding: chunked"), "400"),
        (format!("Host: {host}\r\nOrigin: http://{host}\r\nOrigin: http://{host}\r\nContent-Type: application/json\r\nContent-Length: 0"), "403"),
    ] {
        let mut stream = TcpStream::connect(host).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        stream.write_all(format!("POST /{path}result HTTP/1.1\r\n{headers}\r\n\r\n").as_bytes()).unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with(&format!("HTTP/1.1 {expected}")), "{reply}");
        assert!(request.try_result().is_none());
    }
}
