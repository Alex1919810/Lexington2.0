use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DialogConfig {
    pub width: f32,
    pub max_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub auto_close_delay: f64,
}
impl Default for DialogConfig {
    fn default() -> Self {
        Self {
            width: 200.,
            max_height: 120.,
            offset_x: 100.,
            offset_y: 50.,
            auto_close_delay: 10.,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LlmConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
}
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.openai.com/v1/chat/completions".into(),
            api_key: String::new(),
            model: "gpt-3.5-turbo".into(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClockConfig {
    pub work_reminder_hours: f64,
    pub clock_in_message: String,
    pub clock_out_message: String,
    pub clock_in_cancel_message: String,
}
impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            work_reminder_hours: 12.,
            clock_in_message: "打卡成功！".into(),
            clock_out_message: "下班时间到了！".into(),
            clock_in_cancel_message: "打卡已终止".into(),
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub dialog: DialogConfig,
    pub llm: LlmConfig,
    pub clock_in: ClockConfig,
}
impl Config {
    pub fn load(root: &Path) -> Self {
        let mut config: Self = fs::read(root.join("config/config.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        // Keep malformed sizes/timers from crashing the native renderer.
        config.dialog.width = config.dialog.width.clamp(40., 2000.);
        config.dialog.max_height = config.dialog.max_height.clamp(24., 2000.);
        config.dialog.auto_close_delay = config.dialog.auto_close_delay.clamp(0., 86400.);
        config.clock_in.work_reminder_hours = config.clock_in.work_reminder_hours.clamp(0., 87600.);
        config
    }
}

pub fn resource_root() -> PathBuf {
    if let Some(root) = std::env::var_os("LEXINGTON_HOME") {
        return PathBuf::from(root);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("resource").is_dir()
    {
        return dir.into();
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
fn entries(path: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    files.sort();
    files
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Illustration {
    pub path: PathBuf,
    pub character: String,
    pub format: String,
}
pub fn illustrations(root: &Path) -> Vec<Illustration> {
    let mut result = Vec::new();
    for dir in entries(&root.join("resource/illustration")) {
        if !dir.is_dir() {
            continue;
        }
        let character = dir.file_name().unwrap().to_string_lossy().into_owned();
        for format in ["gif", "png"] {
            for path in entries(&dir.join(format)) {
                if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case(format))
                {
                    result.push(Illustration {
                        path,
                        character: character.clone(),
                        format: format.into(),
                    });
                }
            }
        }
    }
    result
}
pub fn background(root: &Path, kind: &str) -> Option<PathBuf> {
    entries(&root.join("resource/background").join(kind))
        .into_iter()
        .find(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    ["png", "jpg", "jpeg", "gif", "webp"]
                        .iter()
                        .any(|x| e.eq_ignore_ascii_case(x))
                })
        })
}
pub fn texts(root: &Path, character: &str) -> Vec<String> {
    // Character names originate in directory scanning; reject traversal for other callers too.
    if character.is_empty() || Path::new(character).components().count() != 1 || character == ".." {
        return vec![];
    }
    fs::read(
        root.join("resource/texts")
            .join(character)
            .join("text.json"),
    )
    .ok()
    .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
    .unwrap_or_default()
    .into_iter()
    .filter(|s| !s.trim().is_empty())
    .collect()
}
pub fn call_llm(config: &LlmConfig, input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("用户输入不能为空".into());
    }
    if config.api_url.is_empty() || config.api_key.is_empty() || config.model.is_empty() {
        return Err("API配置不完整，请检查config.json中的llm配置".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("网络初始化失败: {e}"))?;
    let response = client.post(&config.api_url).bearer_auth(&config.api_key)
        .json(&serde_json::json!({"model": config.model, "messages": [{"role": "user", "content": input.trim()}]}))
        .send().map_err(|e| if e.is_timeout() { "请求超时，请检查网络连接或API地址".into() } else { format!("网络请求失败: {e}") })?;
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .map_err(|e| format!("解析API响应失败: {e}"))?;
    if status.as_u16() != 200 {
        return Err(body["error"]["message"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("API请求失败，状态码: {status}")));
    }
    body["choices"][0]["message"]["content"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "API响应格式不正确".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

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
