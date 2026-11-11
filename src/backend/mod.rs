pub mod alarm;
pub mod character;
pub mod clock;
pub mod codex;
pub mod codex_config;
pub mod commands;
pub mod config;
mod context;
pub mod handoff;
pub mod jobs;
pub mod memo;
pub mod resources;
pub mod schedule;
pub mod weather;

#[cfg(test)]
mod tests {
    use super::{config::*, resources::*};
    use std::{fs, path::PathBuf};

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
        assert_eq!(config.codex.executable, "codex");
        assert!(config.codex.working_directory.is_none());
        assert!(!config.codex.no_approval);
        assert_eq!(config.clock_in.clock_in_message, "打卡成功！");
        assert!(config.clock_in.auto_start);
        assert_eq!(config.clock_in.confirmation_seconds, 180);
        root.write("config/config.json", "bad json");
        assert_eq!(Config::load(&root.0).dialog.width, 200.);
    }
    #[test]
    fn approval_setting_preserves_unknown_config_and_refuses_to_overwrite_invalid_files() {
        let root = Fixture::new();
        root.write("config/config.json", r#"{"dialog":{"offsetX":37},"llm":{"apiKey":"fixture-value"},"custom":[1,2],"codex":{"executable":"custom-codex","workingDirectory":"project","future":true}}"#);
        let path = root.0.join("config/config.json");
        let before: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for enabled in [true, false] {
            Config::save_no_approval(&root.0, enabled).unwrap();
            assert_eq!(Config::load(&root.0).codex.no_approval, enabled);
            let mut saved: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(
                saved["codex"].as_object_mut().unwrap().remove("noApproval"),
                Some(enabled.into())
            );
            assert_eq!(saved, before);
        }
        for invalid in ["not json", "[]", r#"{"codex":null}"#] {
            fs::write(&path, invalid).unwrap();
            assert!(Config::save_no_approval(&root.0, true).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
        assert_eq!(fs::read_dir(root.0.join("config")).unwrap().count(), 1);
        let missing = Fixture::new();
        Config::save_no_approval(&missing.0, true).unwrap();
        assert!(Config::load(&missing.0).codex.no_approval);
    }
    #[test]
    fn automatic_clock_setting_preserves_config_and_clamps_confirmation_time() {
        let root = Fixture::new();
        root.write("config/config.json", r#"{"clockIn":{"workReminderHours":9,"confirmationSeconds":1,"future":true},"codex":{"noApproval":true},"unknown":[1,2]}"#);
        let path = root.0.join("config/config.json");
        let before: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for enabled in [false, true] {
            Config::save_auto_clock(&root.0, enabled).unwrap();
            let config = Config::read(&root.0).unwrap();
            assert_eq!(config.clock_in.auto_start, enabled);
            assert_eq!(config.clock_in.confirmation_seconds, 30);
            let mut saved: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(
                saved["clockIn"]
                    .as_object_mut()
                    .unwrap()
                    .remove("autoStart"),
                Some(enabled.into())
            );
            assert_eq!(saved, before);
        }
        root.write(
            "config/config.json",
            r#"{"clockIn":{"confirmationSeconds":99999}}"#,
        );
        assert_eq!(
            Config::read(&root.0).unwrap().clock_in.confirmation_seconds,
            600
        );
        for invalid in ["bad json", "[]", r#"{"clockIn":null}"#] {
            fs::write(&path, invalid).unwrap();
            assert!(Config::save_auto_clock(&root.0, false).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
    }
    #[test]
    fn development_setting_preserves_legacy_config_but_never_loads_connection_fields() {
        let root = Fixture::new();
        root.write("config/config.json",r#"{"codex":{"executable":"ignored","workingDirectory":"missing","model":"ignored","apiKey":"fixture-secret","useCustomApi":true,"future":7},"other":42}"#);
        Config::save_development_mode(&root.0, true).unwrap();
        let config = Config::read(&root.0).unwrap();
        assert!(config.codex.development_mode);
        assert_eq!(config.codex.executable, "codex");
        assert!(config.codex.working_directory.is_none());
        assert!(!format!("{:?}", config.codex).contains("fixture-secret"));
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(root.0.join("config/config.json")).unwrap()).unwrap();
        assert_eq!(value["codex"]["future"], 7);
        assert_eq!(value["codex"]["apiKey"], "fixture-secret");
        assert_eq!(value["other"], 42);
        root.write("config/config.json", "invalid");
        assert!(Config::save_development_mode(&root.0, false).is_err());
        assert_eq!(
            fs::read_to_string(root.0.join("config/config.json")).unwrap(),
            "invalid"
        );
    }
}
