use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    #[default]
    Openai,
    Custom,
}

#[derive(Clone, Default)]
pub struct LocalCodexConfig {
    pub kind: ProviderKind,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub use_api_key: bool,
    pub provider_id: String,
    source: String,
}

impl std::fmt::Debug for LocalCodexConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalCodexConfig")
            .field("kind", &self.kind)
            .field("api_key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl LocalCodexConfig {
    pub fn home() -> Result<PathBuf, String> {
        if let Some(home) = std::env::var_os("CODEX_HOME").filter(|home| !home.is_empty()) {
            return Ok(home.into());
        }
        #[cfg(windows)]
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        #[cfg(not(windows))]
        let home = std::env::var_os("HOME");
        home.filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join(".codex"))
            .ok_or_else(|| "无法定位本机 Codex 配置目录。".into())
    }

    pub fn read(home: &Path) -> Result<Self, String> {
        Self::read_with_env(home, |name| {
            if cfg!(test) {
                None
            } else {
                std::env::var(name).ok()
            }
        })
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
        let model = profile
            .and_then(|profile| profile.get("model"))
            .or_else(|| config.get("model"))
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let base_url = field("base_url")
            .map(str::to_owned)
            .or_else(|| {
                (provider_id == "openai" && provider.is_none())
                    .then(|| {
                        config
                            .get("openai_base_url")
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned)
                            .or_else(|| env("OPENAI_BASE_URL"))
                    })
                    .flatten()
            })
            .unwrap_or_default();
        let official_url = base_url.is_empty()
            || url::Url::parse(&base_url).is_ok_and(|url| {
                url.scheme() == "https"
                    && matches!(
                        url.host_str(),
                        Some("api.openai.com" | "chatgpt.com" | "chat.openai.com")
                    )
                    && url.port_or_known_default() == Some(443)
                    && url.username().is_empty()
                    && url.password().is_none()
            });
        let kind = if official_url && (openai_auth || !base_url.is_empty()) {
            ProviderKind::Openai
        } else {
            ProviderKind::Custom
        };
        let use_api_key = !api_key.is_empty() || !openai_auth;
        Ok(Self {
            kind,
            model,
            base_url,
            api_key,
            use_api_key,
            provider_id: provider_id.into(),
            source: text,
        })
    }

    pub fn summary(&self) -> &'static str {
        match (self.kind, self.use_api_key) {
            (ProviderKind::Openai, true) => "OpenAI 官方 API",
            (ProviderKind::Openai, false) => "OpenAI 官方（本机登录）",
            (ProviderKind::Custom, _) => "第三方 API",
        }
    }

    pub fn save(&self, home: &Path) -> Result<(), String> {
        if self.model.len() > 256 || self.model.chars().any(char::is_control) {
            return Err("模型名称无效。".into());
        }
        if self.api_key.len() > 16384 || self.api_key.chars().any(char::is_control) {
            return Err("API Key 无效，请勿包含换行或控制字符。".into());
        }
        if self.kind == ProviderKind::Openai && self.use_api_key && self.api_key.trim().is_empty() {
            return Err("请填写 OpenAI API Key，或选择本机登录。".into());
        }
        if self.kind == ProviderKind::Custom {
            if self.model.trim().is_empty() {
                return Err("请填写第三方 API 的模型名称。".into());
            }
            let address =
                url::Url::parse(self.base_url.trim()).map_err(|_| "API 基础地址无效。")?;
            if !matches!(address.scheme(), "http" | "https")
                || address.host_str().is_none()
                || !address.username().is_empty()
                || address.password().is_some()
                || address.query().is_some()
                || address.fragment().is_some()
            {
                return Err(
                    "API 基础地址必须为 HTTP(S) 地址，不得包含账号、密码、查询参数或片段。".into(),
                );
            }
        }
        let path = home.join("config.toml");
        let path = if path.is_symlink() {
            path.canonicalize()
                .map_err(|_| "无法读取 Codex 配置链接。")?
        } else {
            path
        };
        let current = Self::source(&path)?;
        if current != self.source {
            return Err("本机 Codex 配置已被其他程序修改，请重新读取后再保存。".into());
        }
        let mut document: toml_edit::DocumentMut = current
            .parse()
            .map_err(|_| "本机 Codex 配置格式错误，未保存。")?;
        if document.contains_key("profile") {
            return Err(
                "当前 Codex 配置使用旧版 profile，请先迁移为当前 Codex 支持的配置。".into(),
            );
        }
        let provider_id = match (self.kind, self.use_api_key) {
            (ProviderKind::Openai, false) => "lexington_official",
            (ProviderKind::Openai, true) => "lexington_official_api",
            (ProviderKind::Custom, _) => "lexington_custom",
        };
        document["model_provider"] = toml_edit::value(provider_id);
        if self.model.trim().is_empty() {
            document.remove("model");
        } else {
            document["model"] = toml_edit::value(self.model.trim());
        }
        let providers = document
            .entry("model_providers")
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_like_mut()
            .ok_or("Codex 提供方配置必须为表，未保存。")?;
        if providers.get(provider_id).is_none() {
            providers.insert(provider_id, toml_edit::Item::Table(toml_edit::Table::new()));
        }
        let provider = providers
            .get_mut(provider_id)
            .and_then(toml_edit::Item::as_table_like_mut)
            .ok_or("Codex 提供方配置必须为表，未保存。")?;
        provider.insert(
            "name",
            toml_edit::value(match self.kind {
                ProviderKind::Openai => "OpenAI",
                ProviderKind::Custom => "Lexington Custom API",
            }),
        );
        provider.insert("wire_api", toml_edit::value("responses"));
        let login = self.kind == ProviderKind::Openai && !self.use_api_key;
        provider.insert("requires_openai_auth", toml_edit::value(login));
        provider.remove("env_key");
        if login {
            provider.remove("base_url");
            provider.remove("experimental_bearer_token");
        } else {
            provider.insert(
                "base_url",
                toml_edit::value(if self.kind == ProviderKind::Openai {
                    "https://api.openai.com/v1"
                } else {
                    self.base_url.trim()
                }),
            );
            if self.api_key.trim().is_empty() {
                provider.remove("experimental_bearer_token");
            } else {
                provider.insert(
                    "experimental_bearer_token",
                    toml_edit::value(self.api_key.trim()),
                );
            }
        }
        let parent = path.parent().ok_or("无法定位 Codex 配置目录。")?;
        fs::create_dir_all(parent).map_err(|_| "无法创建 Codex 配置目录。")?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| "无法创建 Codex 临时配置文件。")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temporary
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| "无法保护 Codex 配置文件权限。")?;
        }
        temporary
            .write_all(document.to_string().as_bytes())
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|_| "无法写入 Codex 配置，原配置未修改。")?;
        if Self::source(&path)? != current {
            return Err("本机 Codex 配置已被其他程序修改，请重新读取后再保存。".into());
        }
        temporary
            .persist(&path)
            .map_err(|_| "无法替换 Codex 配置，原配置未修改。")?;
        Ok(())
    }

    fn source(path: &Path) -> Result<String, String> {
        match fs::read_to_string(path) {
            Ok(source) => Ok(source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(_) => Err("无法读取本机 Codex 配置。".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_connection_identifies_official_login_api_and_third_party() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("config.toml");
        let login = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        assert_eq!(login.summary(), "OpenAI 官方（本机登录）");
        assert!(!path.exists());
        let official = LocalCodexConfig::read_with_env(home.path(), |name| {
            (name == "OPENAI_API_KEY").then(|| "fixture-official-key".into())
        })
        .unwrap();
        assert_eq!(official.summary(), "OpenAI 官方 API");
        let proxy = LocalCodexConfig::read_with_env(home.path(), |name| {
            (name == "OPENAI_BASE_URL").then(|| "https://proxy.invalid/v1".into())
        })
        .unwrap();
        assert_eq!(proxy.summary(), "第三方 API");
        fs::write(
            &path,
            "openai_base_url='https://proxy.invalid/v1'\nmodel='fixture-model'\n",
        )
        .unwrap();
        let proxy = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        assert_eq!(proxy.kind, ProviderKind::Custom);
        assert_eq!(proxy.model, "fixture-model");
        for (base_url, expected) in [
            ("https://api.openai.com/v1", ProviderKind::Openai),
            (
                "https://api.openai.com.evil.invalid/v1",
                ProviderKind::Custom,
            ),
            ("http://api.openai.com/v1", ProviderKind::Custom),
            ("https://fixture@api.openai.com/v1", ProviderKind::Custom),
        ] {
            fs::write(&path, format!("model_provider='fixture'\n[model_providers.fixture]\nbase_url='{base_url}'\nexperimental_bearer_token='fixture-key'\n")).unwrap();
            let config = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
            assert_eq!(config.kind, expected);
            assert!(!format!("{config:?}").contains("fixture-key"));
        }
    }

    #[test]
    fn native_connection_save_preserves_comments_unknown_fields_and_login() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("config.toml");
        let source = "# keep this comment\nmodel='old-model'\nmodel_provider='old'\napproval_policy='on-request'\n[model_providers.old]\nname='Old provider'\nexperimental_bearer_token='old-fixture-key'\n[features]\nfixture_unknown=true\n";
        fs::write(&path, source).unwrap();
        let login =
            br#"{"tokens":{"access_token":"fixture-access","refresh_token":"fixture-refresh"}}"#;
        fs::write(home.path().join("auth.json"), login).unwrap();
        let mut draft = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        draft.kind = ProviderKind::Custom;
        draft.model = " fixture-custom-model ".into();
        draft.base_url = " https://fixture.invalid/v1 ".into();
        draft.api_key = " fixture-custom-key ".into();
        draft.save(home.path()).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# keep this comment"));
        let document: toml::Table = saved.parse().unwrap();
        assert_eq!(document["approval_policy"].as_str(), Some("on-request"));
        assert_eq!(
            document["features"]["fixture_unknown"].as_bool(),
            Some(true)
        );
        assert_eq!(
            document["model_providers"]["old"]["experimental_bearer_token"].as_str(),
            Some("old-fixture-key")
        );
        let current = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        assert_eq!(current.summary(), "第三方 API");
        assert_eq!(current.model, "fixture-custom-model");
        assert_eq!(current.base_url, "https://fixture.invalid/v1");
        assert_eq!(current.api_key, "fixture-custom-key");
        assert_eq!(fs::read(home.path().join("auth.json")).unwrap(), login);
        assert!(!home.path().join("config/config.json").exists());
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn native_connection_switches_between_official_key_login_and_custom_without_erasing_keys() {
        let home = tempfile::tempdir().unwrap();
        let mut draft = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        draft.use_api_key = true;
        draft.api_key = "fixture-official-key".into();
        draft.model = "fixture-official-model".into();
        draft.save(home.path()).unwrap();
        let mut draft = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        assert_eq!(draft.summary(), "OpenAI 官方 API");
        assert_eq!(draft.base_url, "https://api.openai.com/v1");
        draft.use_api_key = false;
        draft.api_key.clear();
        draft.model.clear();
        draft.save(home.path()).unwrap();
        let mut draft = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        assert_eq!(draft.summary(), "OpenAI 官方（本机登录）");
        assert!(draft.api_key.is_empty() && draft.base_url.is_empty() && draft.model.is_empty());
        draft.kind = ProviderKind::Custom;
        draft.base_url = "http://127.0.0.1:12345/v1".into();
        draft.model = "fixture-local-model".into();
        draft.save(home.path()).unwrap();
        let current = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        assert_eq!(current.summary(), "第三方 API");
        assert!(current.api_key.is_empty());
        let document: toml::Table = fs::read_to_string(home.path().join("config.toml"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            document["model_providers"]["lexington_official_api"]["experimental_bearer_token"]
                .as_str(),
            Some("fixture-official-key")
        );
        assert!(!home.path().join("auth.json").exists());
    }

    #[test]
    fn native_connection_refuses_invalid_and_concurrently_changed_config() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("config.toml");
        let draft = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
        fs::write(&path, "# external change\n").unwrap();
        assert!(draft.save(home.path()).unwrap_err().contains("重新读取"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "# external change\n");
        fs::write(&path, "malformed='fixture-secret").unwrap();
        let error = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap_err();
        assert!(!error.contains("fixture-secret"));
        for source in ["", "model_providers=[]", "profile='legacy'"] {
            fs::write(&path, source).unwrap();
            let mut draft = LocalCodexConfig::read_with_env(home.path(), |_| None).unwrap();
            draft.kind = ProviderKind::Custom;
            draft.model = "fixture-model".into();
            for address in [
                "not-a-url",
                "file:///fixture",
                "https://fixture:secret@example.invalid/v1",
                "https://example.invalid/v1?key=fixture-secret",
            ] {
                draft.base_url = address.into();
                let error = draft.save(home.path()).unwrap_err();
                assert!(!error.contains("fixture-secret"));
                assert_eq!(fs::read_to_string(&path).unwrap(), source);
            }
            draft.base_url = "https://fixture.invalid/v1".into();
            if source.is_empty() {
                draft.kind = ProviderKind::Openai;
                draft.use_api_key = true;
            }
            assert!(draft.save(home.path()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 1);
    }

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
