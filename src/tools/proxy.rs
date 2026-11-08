//! Bridge Windows' manual proxy settings to CLI children that only read proxy env vars.
use std::{ffi::OsString, process::Command};
use windows_sys::Win32::{
    Foundation::GlobalFree,
    Networking::WinHttp::{
        WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WinHttpGetIEProxyConfigForCurrentUser,
    },
};

const PROXY_ENV: [&str; 6] = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];

pub fn inherit_system_proxy(command: &mut Command) {
    // An explicit empty value also represents a deliberate override. Never change global env.
    if PROXY_ENV
        .iter()
        .any(|name| std::env::var_os(name).is_some())
    {
        return;
    }
    let mut config = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
    if unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut config) } == 0 {
        return;
    }
    // WinHTTP owns all three buffers, including the unused PAC URL.
    let server = take_string(config.lpszProxy);
    let bypass = take_string(config.lpszProxyBypass);
    let _ = take_string(config.lpszAutoConfigUrl);
    configure(command, &server, &bypass, |name| std::env::var_os(name));
}

fn take_string(pointer: *mut u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    unsafe {
        let mut length = 0;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length));
        GlobalFree(pointer.cast());
        text
    }
}

fn configure(
    command: &mut Command,
    server: &str,
    bypass: &str,
    env: impl Fn(&str) -> Option<OsString>,
) {
    if PROXY_ENV.iter().any(|name| env(name).is_some()) {
        return;
    }
    let mut configured = false;
    for protocol in ["http", "https"] {
        let endpoint = if server.contains('=') {
            server.split(';').find_map(|entry| {
                let (name, endpoint) = entry.trim().split_once('=')?;
                name.trim()
                    .eq_ignore_ascii_case(protocol)
                    .then_some(endpoint)
            })
        } else {
            Some(server)
        };
        if let Some(endpoint) = endpoint.and_then(proxy_url) {
            command.env(format!("{}_PROXY", protocol.to_uppercase()), endpoint);
            configured = true;
        }
    }
    if !configured {
        return;
    }
    // Keep explicit exclusions. Windows wildcard rules cannot in general be represented
    // by NO_PROXY; carry over literals, domain suffixes and IPv4 prefix subnets only.
    if env("NO_PROXY").is_none() && env("no_proxy").is_none() {
        let mut exclusions = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];
        for rule in bypass.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            if rule == "*" {
                exclusions.push(rule.into());
            } else if let Some(domain) = rule.strip_prefix("*.") {
                if !domain.contains(['*', '?', '<', '>']) {
                    exclusions.push(format!(".{domain}"));
                }
            } else if let Some(prefix) = rule.strip_suffix(".*") {
                let octets: Vec<_> = prefix.split('.').collect();
                if (1..=3).contains(&octets.len()) && octets.iter().all(|s| s.parse::<u8>().is_ok())
                {
                    exclusions.push(format!(
                        "{}{}/{}",
                        prefix,
                        ".0".repeat(4 - octets.len()),
                        octets.len() * 8
                    ));
                }
            } else if !rule.contains(['*', '?', '<', '>']) {
                exclusions.push(rule.into());
            }
        }
        command.env("NO_PROXY", exclusions.join(","));
    }
}

fn proxy_url(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.chars().any(char::is_whitespace) {
        return None;
    }
    let endpoint = if endpoint.contains("://") {
        endpoint.to_owned()
    } else {
        // The 'https=' selector means HTTPS destinations, not TLS to the proxy itself.
        format!("http://{endpoint}")
    };
    let parsed = url::Url::parse(&endpoint).ok()?;
    (matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str().is_some()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && parsed.path() == "/")
        .then_some(endpoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(command: &Command) -> std::collections::BTreeMap<String, String> {
        command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.unwrap().to_string_lossy().into_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn manual_proxy_maps_protocols_and_preserves_explicit_environment() {
        let mut command = Command::new("fixture-only.exe");
        configure(&mut command, "127.0.0.1:7890", "", |_| None);
        let vars = values(&command);
        assert_eq!(vars["HTTP_PROXY"], "http://127.0.0.1:7890");
        assert_eq!(vars["HTTPS_PROXY"], "http://127.0.0.1:7890");
        assert!(command.get_args().next().is_none());
        for name in PROXY_ENV {
            let mut command = Command::new("fixture-only.exe");
            configure(&mut command, "127.0.0.1:7890", "", |key| {
                (key == name).then(OsString::new)
            });
            assert_eq!(command.get_envs().count(), 0);
        }
        let mut command = Command::new("fixture-only.exe");
        configure(
            &mut command,
            "http=127.0.0.1:8000; https=127.0.0.1:8001; socks=127.0.0.1:8002",
            "",
            |key| (key == "NO_PROXY").then(|| "explicit.invalid".into()),
        );
        let vars = values(&command);
        assert_eq!(vars["HTTP_PROXY"], "http://127.0.0.1:8000");
        assert_eq!(vars["HTTPS_PROXY"], "http://127.0.0.1:8001");
        assert!(!vars.contains_key("NO_PROXY"));
    }

    #[test]
    fn proxy_validation_and_supported_bypass_rules() {
        for value in [
            "",
            "bad proxy",
            "socks5://localhost:1080",
            "http://user:secret@localhost",
            "http://localhost/path",
            "http://localhost?key=secret",
        ] {
            assert!(proxy_url(value).is_none());
        }
        for server in [
            "",
            "socks=127.0.0.1:1080",
            "https=http://user:secret@localhost",
        ] {
            let mut command = Command::new("fixture-only.exe");
            configure(&mut command, server, "", |_| None);
            assert_eq!(command.get_envs().count(), 0);
        }
        let mut command = Command::new("fixture-only.exe");
        configure(
            &mut command,
            "https=127.0.0.1:7890",
            "*.example.invalid;10.*;192.168.*;172.16.1.*;host.invalid;<local>;a?b.invalid",
            |_| None,
        );
        let vars = values(&command);
        assert!(!vars.contains_key("HTTP_PROXY"));
        assert_eq!(
            vars["NO_PROXY"],
            "localhost,127.0.0.1,::1,.example.invalid,10.0.0.0/8,192.168.0.0/16,172.16.1.0/24,host.invalid"
        );
    }
}
