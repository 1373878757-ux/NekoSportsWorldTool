//! User-initiated, one-shot browser geolocation. Only a loopback listener is used;
//! this module does not call school/account APIs or persist any configuration.

use crate::location::Coordinate;
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REQUEST_LIFETIME: Duration = Duration::from_secs(120);
const MAX_HEADER_BYTES: usize = 8192;
const MAX_BODY_BYTES: usize = 2048;
const MAX_FIX_AGE_MS: i64 = 120_000;
const MAX_CLOCK_AHEAD_MS: i64 = 30_000;
const PAGE: &str = include_str!("../assets/location/authorize.html");

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LocationFix {
    pub wgs84: Coordinate,
    pub observed_at_ms: i64,
}

impl LocationFix {
    /// The existing platform-point request uses the project's BD-09 convention.
    /// Convert a browser's WGS84 fix exactly once at this input boundary.
    pub fn platform_coordinate(self) -> Result<Coordinate, String> {
        let (latitude, longitude) =
            crate::track::geom::wgs84_to_bd09(self.wgs84.latitude, self.wgs84.longitude);
        Coordinate::new(latitude, longitude, self.wgs84.accuracy)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum BrowserReply {
    Position {
        latitude: f64,
        longitude: f64,
        accuracy: f64,
        timestamp_ms: i64,
    },
    Error {
        code: u32,
    },
}

fn parse_reply(bytes: &[u8], now_ms: i64) -> Result<LocationFix, String> {
    match serde_json::from_slice::<BrowserReply>(bytes)
        .map_err(|_| "定位响应格式无效，请重新获取或手动填写位置".to_owned())?
    {
        BrowserReply::Position {
            latitude,
            longitude,
            accuracy,
            timestamp_ms,
        } => {
            let wgs84 = Coordinate::new(latitude, longitude, accuracy)?;
            if timestamp_ms <= 0
                || now_ms.saturating_sub(timestamp_ms) > MAX_FIX_AGE_MS
                || timestamp_ms.saturating_sub(now_ms) > MAX_CLOCK_AHEAD_MS
            {
                return Err("定位结果已过期或时间无效，请重新获取当前位置".into());
            }
            let fix = LocationFix {
                wgs84,
                observed_at_ms: timestamp_ms,
            };
            fix.platform_coordinate()?;
            Ok(fix)
        }
        BrowserReply::Error { code } => Err(match code {
            1 => "定位权限被拒绝，请在系统及浏览器中允许定位后重新获取，也可手动填写位置",
            2 => "当前设备无法提供位置，请检查系统定位服务，也可手动填写位置",
            3 => "获取当前位置超时，请稍后重新获取，也可手动填写位置",
            _ => "当前浏览器不支持定位，请换用支持定位的浏览器，也可手动填写位置",
        }
        .into()),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

pub(crate) struct BrowserLocationRequest {
    url: String,
    receiver: mpsc::Receiver<Result<LocationFix, String>>,
    cancelled: Arc<AtomicBool>,
    worker: std::thread::Thread,
}

impl BrowserLocationRequest {
    pub fn start(wake: impl FnOnce() + Send + 'static) -> Result<Self, String> {
        if crate::runtime::offline() {
            return Err("离线模式下请手动填写位置".into());
        }
        Self::listen(REQUEST_LIFETIME, wake)
    }

    fn listen(lifetime: Duration, wake: impl FnOnce() + Send + 'static) -> Result<Self, String> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| "无法打开本机定位回传端口，请手动填写位置".to_owned())?;
        listener
            .set_nonblocking(true)
            .map_err(|_| "无法准备本机定位回传，请手动填写位置".to_owned())?;
        let address = listener
            .local_addr()
            .map_err(|_| "无法读取本机定位回传地址".to_owned())?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let origin = format!("http://{address}");
        let path = format!("/location/{token}/");
        let url = format!("{origin}{path}");
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("location-acquisition".into())
            .spawn(move || {
                let start = Instant::now();
                let page = PAGE
                    .replace("{{APP_NAME}}", crate::presentation::APP_NAME)
                    .replace("{{NONCE}}", &token);
                let result = loop {
                    if worker_cancelled.load(Ordering::Acquire) {
                        drop(listener);
                        return;
                    }
                    if start.elapsed() >= lifetime {
                        break Err("定位请求已超时，请点击“获取当前位置”重新开始".into());
                    }
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            if let Some(result) = serve(
                                &mut stream,
                                &origin,
                                &path,
                                &token,
                                &page,
                                &worker_cancelled,
                            ) {
                                break result;
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            // This is a bounded background wait for browser I/O. It
                            // neither blocks the GUI nor causes GUI repaint polling.
                            std::thread::park_timeout(Duration::from_millis(40));
                        }
                        Err(_) => break Err("本机定位回传失败，请重新获取或手动填写位置".into()),
                    }
                };
                drop(listener);
                if !worker_cancelled.load(Ordering::Acquire) {
                    let _ = sender.send(result);
                    wake();
                }
            })
            .map_err(|_| "无法启动定位任务，请手动填写位置".to_owned())?;
        Ok(Self {
            url,
            receiver,
            cancelled,
            worker: worker.thread().clone(),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn try_result(&self) -> Option<Result<LocationFix, String>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("定位任务已结束，请重新获取位置".into()))
            }
        }
    }
}

impl Drop for BrowserLocationRequest {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.worker.unpark();
    }
}

fn read_request(stream: &mut TcpStream, cancelled: &AtomicBool) -> Option<(String, Vec<u8>)> {
    stream.set_nonblocking(false).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(300)))
        .ok()?;
    let started = Instant::now();
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 2048];
    loop {
        if cancelled.load(Ordering::Acquire) || started.elapsed() >= Duration::from_secs(1) {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(length) => bytes.extend_from_slice(&chunk[..length]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => return None,
        }
        if bytes.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
            return None;
        }
        let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") else {
            if bytes.len() > MAX_HEADER_BYTES {
                return None;
            }
            continue;
        };
        if end > MAX_HEADER_BYTES {
            return None;
        }
        let header = std::str::from_utf8(&bytes[..end]).ok()?;
        if header.split("\r\n").skip(1).any(|line| {
            line.split_once(':')
                .is_some_and(|(name, _)| name.eq_ignore_ascii_case("transfer-encoding"))
        }) {
            return None;
        }
        let length = match header_value(header, "content-length") {
            Some(value) => value.parse::<usize>().ok()?,
            None => 0,
        };
        if length > MAX_BODY_BYTES {
            return None;
        }
        if bytes.len() >= end + 4 + length {
            return Some((header.to_owned(), bytes[end + 4..end + 4 + length].to_vec()));
        }
    }
}

// Reject duplicate headers rather than choosing one conflicting origin/host.
fn header_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    let mut values = header.split("\r\n").skip(1).filter_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then_some(value.trim())
    });
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &str, token: &str) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nPermissions-Policy: geolocation=(self)\r\nContent-Security-Policy: default-src 'none'; script-src 'nonce-{token}'; style-src 'nonce-{token}'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}

fn serve(
    stream: &mut TcpStream,
    origin: &str,
    path: &str,
    token: &str,
    page: &str,
    cancelled: &AtomicBool,
) -> Option<Result<LocationFix, String>> {
    let Some((header, body)) = read_request(stream, cancelled) else {
        respond(
            stream,
            "400 Bad Request",
            "text/plain",
            "Invalid request",
            token,
        );
        return None;
    };
    let mut line = header.lines().next()?.split_whitespace();
    let method = line.next()?;
    let request_path = line.next()?;
    if header_value(&header, "host") != origin.strip_prefix("http://") {
        respond(stream, "403 Forbidden", "text/plain", "Invalid host", token);
        return None;
    }
    if method == "GET" && request_path == path {
        respond(stream, "200 OK", "text/html", page, token);
        return None;
    }
    if method != "POST" || request_path != format!("{path}result") {
        respond(stream, "404 Not Found", "text/plain", "Not found", token);
        return None;
    }
    if header_value(&header, "origin") != Some(origin)
        || header_value(&header, "content-type") != Some("application/json")
        || header_value(&header, "content-length").is_none()
    {
        respond(
            stream,
            "403 Forbidden",
            "text/plain",
            "Invalid origin or content type",
            token,
        );
        return None;
    }
    let result = parse_reply(&body, now_ms());
    respond(
        stream,
        "200 OK",
        "application/json",
        "{\"received\":true}",
        token,
    );
    Some(result)
}

#[cfg(test)]
mod tests;
