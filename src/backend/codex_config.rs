//! Read the effective provider key solely for redaction. Never persist or display it.
use std::{fs, path::Path};

#[derive(Default)]
pub struct LocalCodexConfig {
    pub api_key: String,
}

impl LocalCodexConfig {
    pub fn read(home: &Path) -> Result<Self, String> {
        Self::read_with_env(home, |name| std::env::var(name).ok())
    }

    fn read_with_env(home: &Path, env: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let text = match fs::read_to_string(home.join("config.toml")) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(_) => return Err("无法读取本机 Codex 配置。".into()),
        };
        // Parser errors may contain source lines, including inline credentials.
        let config: toml::Table = text.parse().map_err(|_| "本机 Codex 配置格式错误。")?;
        let profile = config
            .get("profile")
            .and_then(toml::Value::as_str)
            .and_then(|name| config.get("profiles")?.get(name));
        let provider_id = profile
            .and_then(|p| p.get("model_provider"))
            .or_else(|| config.get("model_provider"))
            .and_then(toml::Value::as_str)
            .unwrap_or("openai");
        let provider = config
            .get("model_providers")
            .and_then(|p| p.get(provider_id));
        let field = |name| {
            provider
                .and_then(|p| p.get(name))
                .and_then(toml::Value::as_str)
        };
        let mut api_key = field("env_key")
            .and_then(&env)
            .filter(|key| !key.trim().is_empty())
            .or_else(|| {
                field("experimental_bearer_token")
                    .map(str::to_owned)
                    .filter(|key| !key.trim().is_empty())
            })
            .unwrap_or_default();
        let openai_auth = provider
            .and_then(|p| p.get("requires_openai_auth"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(provider_id == "openai");
        if api_key.is_empty() && openai_auth {
            api_key = env("OPENAI_API_KEY").unwrap_or_default();
            if api_key.trim().is_empty() {
                match fs::read(home.join("auth.json")) {
                    Ok(bytes) => {
                        let auth: serde_json::Value = serde_json::from_slice(&bytes)
                            .map_err(|_| "本机 Codex 登录配置格式错误。")?;
                        // OAuth access/refresh/id tokens are not portable API keys.
                        api_key = auth["OPENAI_API_KEY"].as_str().unwrap_or_default().into();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err("无法读取本机 Codex 登录配置。".into()),
                }
            }
        }
        Ok(Self { api_key })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_profiles_inline_and_environment_keys_and_preserves_inheritance() {
        let home =
            std::env::temp_dir().join(format!("lexington-local-codex-{}", rand::random::<u64>()));
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join("config.toml"),
            r#"
model = "global-model"
model_provider = "openai"
profile = "work"
[profiles.work]
model = "profile-model"
model_provider = "work"
[model_providers.work]
base_url = "https://example.invalid/v1"
env_key = "FIXTURE_API_KEY"
experimental_bearer_token = "fixture-inline-secret"
requires_openai_auth = false
"#,
        )
        .unwrap();
        let inline = LocalCodexConfig::read_with_env(&home, |_| None).unwrap();
        assert_eq!(inline.api_key, "fixture-inline-secret");
        let environment = LocalCodexConfig::read_with_env(&home, |name| {
            (name == "FIXTURE_API_KEY").then(|| "fixture-env-secret".into())
        })
        .unwrap();
        assert_eq!(environment.api_key, "fixture-env-secret");
        fs::write(
            home.join("config.toml"),
            "model_provider='work'\n[model_providers.work]\nenv_key='FIXTURE_API_KEY'\n",
        )
        .unwrap();
        let key = LocalCodexConfig::read_with_env(&home, |name| {
            (name == "FIXTURE_API_KEY").then(|| "fixture-env-secret".into())
        })
        .unwrap();
        assert_eq!(key.api_key, "fixture-env-secret");
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn auth_uses_only_api_keys_and_errors_never_echo_source() {
        let home =
            std::env::temp_dir().join(format!("lexington-local-auth-{}", rand::random::<u64>()));
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join("auth.json"), r#"{"OPENAI_API_KEY":"fixture-auth-secret","tokens":{"access_token":"never-use-oauth"}}"#).unwrap();
        let local = LocalCodexConfig::read_with_env(&home, |_| None).unwrap();
        assert_eq!(local.api_key, "fixture-auth-secret");
        fs::write(
            home.join("auth.json"),
            r#"{"tokens":{"access_token":"never-use-oauth"}}"#,
        )
        .unwrap();
        assert!(
            LocalCodexConfig::read_with_env(&home, |_| None)
                .unwrap()
                .api_key
                .is_empty()
        );
        fs::write(
            home.join("config.toml"),
            "experimental_bearer_token='fixture-secret\n",
        )
        .unwrap();
        let error = LocalCodexConfig::read_with_env(&home, |_| None)
            .err()
            .unwrap();
        assert!(!error.contains("fixture-secret"));
        fs::remove_dir_all(home).unwrap();
    }
}
