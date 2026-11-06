pub mod character;
pub mod clock;
pub mod config;
pub mod llm;
pub mod resources;

#[cfg(test)]
mod tests {
    use super::{config::*, llm::call_llm, resources::*};
    use std::io::{Read, Write};
    use std::{fs, path::PathBuf, time::Duration};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lexington-test-{}-{}",
                std::process::id(),
                rand::random::<u64>()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn write(&self, path: &str, contents: &str) {
            let target = self.0.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, contents).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn scans_only_supported_files_and_validates_character_text() {
        let root = Fixture::new();
        root.write("resource/illustration/乙/png/02.PNG", "");
        root.write("resource/illustration/甲/gif/01.gif", "");
        root.write("resource/illustration/甲/gif/not.png", "");
        root.write("resource/illustration/甲/png/fake.png/not-a-file", "");
        root.write("resource/texts/甲/text.json", r#"["你好", " ", "再见"]"#);
        root.write("resource/texts/乙/text.json", r#"["你好", 42]"#);
        let images = illustrations(&root.0);
        assert_eq!(images.len(), 2);
        assert_eq!(
            images.iter().map(|i| i.format.as_str()).collect::<Vec<_>>(),
            ["png", "gif"]
        );
        assert_eq!(texts(&root.0, "甲"), ["你好", "再见"]);
        assert!(texts(&root.0, "乙").is_empty());
        assert!(texts(&root.0, "../甲").is_empty());
        assert!(illustrations(&root.0.join("missing")).is_empty());
    }
    #[test]
    fn partial_config_keeps_defaults_and_clamps_invalid_geometry() {
        let root = Fixture::new();
        root.write(
            "config/config.json",
            r#"{"dialog":{"width":-2,"offsetX":0},"llm":{"model":"local"}}"#,
        );
        let config = Config::load(&root.0);
        assert_eq!(config.dialog.width, 40.);
        assert_eq!(config.dialog.offset_x, 0.);
        assert_eq!(config.dialog.max_height, 120.);
        assert_eq!(config.llm.model, "local");
        assert_eq!(config.clock_in.clock_in_message, "打卡成功！");
        root.write("config/config.json", "bad json");
        assert_eq!(Config::load(&root.0).dialog.width, 200.);
    }
    fn mock_request(status: &str, body: &str) -> (Result<String, String>, serde_json::Value) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let n = socket.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(header_end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..header_end]);
                    let len: usize = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= header_end + 4 + len {
                        assert!(header.starts_with("POST /chat?test=1 HTTP/1.1"));
                        assert!(
                            header
                                .to_ascii_lowercase()
                                .contains("authorization: bearer test-key")
                        );
                        let request: serde_json::Value =
                            serde_json::from_slice(&bytes[header_end + 4..]).unwrap();
                        socket.write_all(response.as_bytes()).unwrap();
                        return request;
                    }
                }
            }
        });
        let config = LlmConfig {
            api_url: format!("http://{address}/chat?test=1"),
            api_key: "test-key".into(),
            model: "local-model".into(),
        };
        let result = call_llm(&config, "  你好  ");
        (result, server.join().unwrap())
    }
    #[test]
    fn llm_posts_compatible_request_and_parses_unicode_reply() {
        let (result, request) = mock_request(
            "200 OK",
            r#"{"choices":[{"message":{"content":"  你好，提督！  "}}]}"#,
        );
        assert_eq!(result.unwrap(), "你好，提督！");
        assert_eq!(
            request,
            serde_json::json!({"model":"local-model","messages":[{"role":"user","content":"你好"}]})
        );
    }
    #[test]
    fn llm_reports_api_and_response_errors() {
        assert_eq!(
            mock_request("401 Unauthorized", r#"{"error":{"message":"invalid key"}}"#)
                .0
                .unwrap_err(),
            "invalid key"
        );
        assert_eq!(
            mock_request("200 OK", r#"{"choices":[]}"#).0.unwrap_err(),
            "API响应格式不正确"
        );
        assert!(
            call_llm(&LlmConfig::default(), "hello")
                .unwrap_err()
                .contains("配置不完整")
        );
        assert_eq!(
            call_llm(&LlmConfig::default(), "  ").unwrap_err(),
            "用户输入不能为空"
        );
    }
}
