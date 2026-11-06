pub mod character;
pub mod clock;
pub mod codex;
pub mod config;
pub mod resources;

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
        assert_eq!(config.clock_in.clock_in_message, "打卡成功！");
        root.write("config/config.json", "bad json");
        assert_eq!(Config::load(&root.0).dialog.width, 200.);
    }
}
