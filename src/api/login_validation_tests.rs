//! Synthetic credentials and loopback responses only; no account or CAPTCHA service is used.
use super::*;
use crate::crypto::decrypt::Decrypted;

fn credentials() -> Value {
    json!({
        "lotNumber": "fixture-lot",
        "captchaOutput": "fixture-output",
        "passToken": "fixture-pass",
        "genTime": "1700000000",
    })
}

fn response(http_status: u16, business: Option<Value>) -> RequestOutcome {
    RequestOutcome {
        http_status,
        decrypted: business.map(|business| Decrypted {
            plaintext: String::new(),
            business,
            expected_md5: String::new(),
            actual_md5: String::new(),
        }),
        raw_len: 0,
        raw_head: "private-response-must-not-be-logged".into(),
    }
}

#[test]
fn complete_credentials_preserve_the_existing_platform_payload() {
    let creds = credentials();
    let body: Value =
        serde_json::from_str(&geevalidate_body(&creds, "fixture-uuid", "fixture-user").unwrap())
            .unwrap();
    assert_eq!(
        body,
        json!({
            "lotNumber": "fixture-lot",
            "captchaOutput": "fixture-output",
            "passToken": "fixture-pass",
            "genTime": "1700000000",
            "isOffline": false,
            "osType": 1,
            "businessType": 0,
            "uuid": "fixture-uuid",
            "username": "fixture-user",
        })
    );
    assert_eq!(creds, credentials());
}

#[test]
fn timestamp_number_is_preserved_without_changing_credential_values() {
    let mut creds = credentials();
    creds["genTime"] = json!(1700000000_u64);
    let body: Value =
        serde_json::from_str(&geevalidate_body(&creds, "uuid", "user").unwrap()).unwrap();
    assert_eq!(body["genTime"], creds["genTime"]);
}

#[test]
fn missing_null_empty_and_invalid_credentials_are_rejected_before_transport() {
    struct OfflineGuard(Option<bool>);
    impl Drop for OfflineGuard {
        fn drop(&mut self) {
            crate::runtime::TEST_OFFLINE.with(|state| state.set(self.0));
        }
    }
    let _guard = OfflineGuard(crate::runtime::TEST_OFFLINE.with(|state| state.replace(Some(true))));
    let mut client = ApiClient::new(crate::crypto::header::HeaderIdentity::default(), None);
    let mut logs = Vec::new();
    for field in ["lotNumber", "captchaOutput", "passToken", "genTime"] {
        let mut values = vec![
            None,
            Some(json!(null)),
            Some(json!("")),
            Some(json!(" \n\t")),
        ];
        values.extend([
            Some(json!(false)),
            Some(json!({})),
            Some(json!([])),
            Some(json!(-1)),
        ]);
        if field != "genTime" {
            values.push(Some(json!(1700000000)));
        }
        for value in values {
            let mut creds = credentials();
            if let Some(value) = value {
                creds[field] = value;
            } else {
                creds.as_object_mut().unwrap().remove(field);
            }
            let error =
                verify_captcha_credentials(&mut client, &creds, "uuid", "user", &mut |line| {
                    logs.push(line.to_owned());
                })
                .unwrap_err();
            assert!(error.contains("验证码凭证不完整"), "{error}");
            assert!(error.contains(field), "{error}");
            assert!(!error.contains("fixture-pass"));
            // A network attempt would instead return runtime::OFFLINE_MESSAGE.
            assert_ne!(error, crate::runtime::OFFLINE_MESSAGE);
        }
    }
    assert!(
        logs.is_empty(),
        "incomplete credentials must not be reported as passed"
    );
}

#[test]
fn only_the_explicit_success_code_allows_login_to_continue() {
    assert_eq!(
        geevalidate_result(&response(200, Some(json!({"error":10000})))).unwrap(),
        GeeValidation::Passed
    );
    assert_eq!(
        geevalidate_result(&response(200, Some(json!({"error":10003})))).unwrap(),
        GeeValidation::Rejected
    );
    for code in [0, 10001, 10175, 429, 500] {
        let error = geevalidate_result(&response(
            200,
            Some(json!({"error":code,"message":"拒绝登录"})),
        ))
        .unwrap_err();
        assert!(error.contains(&code.to_string()), "{error}");
        assert!(error.contains("拒绝登录"));
        assert!(error.contains("已停止"));
    }
}

#[test]
fn malformed_business_codes_do_not_allow_login_or_expose_the_response() {
    for business in [
        json!({}),
        json!({"error":null}),
        json!({"error":"10000"}),
        json!({"error":true}),
        json!({"error":10000.5}),
        json!({"data":{"error":10000}}),
        json!([]),
        json!(null),
    ] {
        let error = geevalidate_result(&response(200, Some(business))).unwrap_err();
        assert!(error.contains("缺少有效业务码"), "{error}");
        assert!(!error.contains("private-response"));
    }
}

#[test]
fn http_and_decryption_errors_stop_validation_without_exposing_raw_content() {
    for status in [199, 301, 400, 401, 429, 502] {
        let error =
            geevalidate_result(&response(status, Some(json!({"error":10000})))).unwrap_err();
        assert!(error.contains(&format!("HTTP {status}")));
        assert!(!error.contains("private-response"));
    }
    let error = geevalidate_result(&response(200, None)).unwrap_err();
    assert!(error.contains("响应解密失败"));
    assert!(!error.contains("private-response"));
}

#[test]
fn server_account_error_and_remaining_attempts_are_preserved() {
    let error = geevalidate_result(&response(
        200,
        Some(json!({"error":10175,"message":"账号或密码错误，还可尝试2次，失败后将锁定1小时"})),
    ))
    .unwrap_err();
    assert!(error.contains("10175"));
    assert!(error.contains("还可尝试2次"));
    assert!(error.contains("失败后将锁定1小时"));
}

#[test]
fn actual_validation_transport_only_contacts_the_loopback_endpoint_and_hides_credentials() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    struct NetworkGuard(Option<bool>);
    impl Drop for NetworkGuard {
        fn drop(&mut self) {
            crate::runtime::TEST_OFFLINE.with(|state| state.set(self.0));
        }
    }
    let _guard =
        NetworkGuard(crate::runtime::TEST_OFFLINE.with(|state| state.replace(Some(false))));
    for business in [
        json!({"error":10000}),
        json!({"error":10003}),
        json!({"error":10175,"message":"fixture account rejected"}),
        json!({"data":true}),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut client = ApiClient::new(crate::crypto::header::HeaderIdentity::default(), None);
        client.set_test_host(format!("http://{}", listener.local_addr().unwrap()));
        client.agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(2))
            .build();
        let expected = geevalidate_result(&response(200, Some(business.clone())));
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "mock validation request did not arrive"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut chunk = [0; 4096];
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&chunk[..size]);
                assert!(bytes.len() < 64 * 1024);
                if let Some(at) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    break at + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..end]);
            assert_eq!(
                headers.lines().next().unwrap(),
                format!("POST {GEEVALIDATE_PATH} HTTP/1.1")
            );
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < end + length {
                let mut chunk = [0; 4096];
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&chunk[..size]);
            }
            assert!(serde_json::from_slice::<Value>(&bytes[end..end + length])
                .unwrap()
                .is_object());
            let body = business.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let mut logs = Vec::new();
        let actual =
            verify_captcha_credentials(&mut client, &credentials(), "uuid", "user", &mut |line| {
                logs.push(line.to_owned());
            });
        server.join().unwrap();
        assert_eq!(actual, expected);
        assert_eq!(logs, vec!["[login] 已取得验证码凭证，等待平台二次校验"]);
        for secret in [
            "fixture-lot",
            "fixture-output",
            "fixture-pass",
            "1700000000",
        ] {
            assert!(!logs.join("\n").contains(secret));
        }
    }
}
