use super::config::LlmConfig;
use std::{sync::mpsc, time::Duration};

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

/// Run HTTP work off the UI thread and notify its event loop after completion.
pub fn request(
    config: LlmConfig,
    input: String,
    wake: impl FnOnce() + Send + 'static,
) -> mpsc::Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(call_llm(&config, &input));
        wake();
    });
    receiver
}
