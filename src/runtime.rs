//! Runtime controls for isolated local verification. No credentials are read here.

pub const OFFLINE_MESSAGE: &str = "离线验证模式：联网操作已禁用";

pub fn offline() -> bool {
    #[cfg(test)]
    if let Some(value) = TEST_OFFLINE.with(|value| value.get()) {
        return value;
    }
    std::env::var_os("NSWT_OFFLINE").is_some_and(|value| value == "1")
}

pub fn ensure_network_allowed() -> Result<(), String> {
    if offline() {
        Err(OFFLINE_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_OFFLINE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_policy_refuses_before_transport_is_used() {
        TEST_OFFLINE.with(|value| value.set(Some(true)));
        assert_eq!(ensure_network_allowed().unwrap_err(), OFFLINE_MESSAGE);
        TEST_OFFLINE.with(|value| value.set(Some(false)));
        assert!(ensure_network_allowed().is_ok());
        TEST_OFFLINE.with(|value| value.set(None));
    }

    #[test]
    fn offline_http_entrypoints_return_the_policy_error_without_using_a_socket() {
        TEST_OFFLINE.with(|value| value.set(Some(true)));
        let mut client =
            crate::api::client::ApiClient::new(crate::api::model::HeaderIdentity::default(), None);
        let error = client
            .envelope_request("POST", "not-a-network-url", "{}", "fixture", &[])
            .err()
            .unwrap();
        assert_eq!(error, OFFLINE_MESSAGE);
        assert_eq!(
            crate::api::obs::put_object("not-a-network-url", b"{}", &mut |_| {}).unwrap_err(),
            OFFLINE_MESSAGE
        );
        assert_eq!(
            crate::api::obs::fetch_object(&mut client, "fixture", &mut |_| {}).unwrap_err(),
            OFFLINE_MESSAGE
        );
        assert_eq!(
            crate::api::gt4::solve_gt4("fixture", &mut client, &mut |_| {}).unwrap_err(),
            OFFLINE_MESSAGE
        );
        TEST_OFFLINE.with(|value| value.set(None));
    }
}
