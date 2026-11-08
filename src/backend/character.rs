use super::resources::{self, Illustration};
use std::path::Path;
use std::{collections::BTreeMap, fs, io::Read, path::PathBuf};

#[derive(Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct AgentRegistry {
    default_character: Option<String>,
    characters: BTreeMap<String, PathBuf>,
}

fn read_agent_file(path: &Path) -> Result<String, String> {
    let mut text = String::new();
    fs::File::open(path)
        .and_then(|file| file.take(65537).read_to_string(&mut text))
        .map_err(|_| "无法读取角色设定文件，请检查文件是否存在及 UTF-8 编码。".to_owned())?;
    if text.len() > 65536 {
        return Err("角色设定文件不能超过 64 KiB。".into());
    }
    Ok(text)
}

fn agent_registry(root: &Path) -> Result<AgentRegistry, String> {
    let path = root.join("resource/agents/characters.json");
    if !path.try_exists().map_err(|_| "无法检查角色配置文件。")? {
        return Ok(AgentRegistry::default());
    }
    serde_json::from_str(&read_agent_file(&path)?)
        .map_err(|_| "角色配置 characters.json 格式错误。".into())
}

/// Unconfigured characters use a fresh neutral session, never another character's persona.
pub fn agent_instructions(root: &Path, character: &str) -> Result<Option<String>, String> {
    let registry = agent_registry(root)?;
    let Some(relative) = registry.characters.get(character) else {
        return Ok(None);
    };
    if relative.extension().is_none_or(|ext| ext != "md")
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("角色设定路径必须为 agents 目录内的相对 .md 路径。".into());
    }
    let directory = root
        .join("resource/agents")
        .canonicalize()
        .map_err(|_| "角色设定目录不可用。")?;
    let path = directory
        .join(relative)
        .canonicalize()
        .map_err(|_| "找不到当前角色配置的 Markdown 设定。")?;
    if !path.starts_with(&directory) {
        return Err("角色设定不能指向 agents 目录之外。".into());
    }
    let text = read_agent_file(&path)?;
    if text.trim().is_empty() {
        return Err("当前角色的 Markdown 设定为空。".into());
    }
    Ok(Some(text))
}

/// Resource selection and format fallback, with no GPU or UI state.
pub struct CharacterState {
    pub illustrations: Vec<Illustration>,
    pub selected: Option<usize>,
    pub character: String,
    pub format: String,
    pub damaged: bool,
    pub texts: Vec<String>,
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            illustrations: vec![],
            selected: None,
            character: String::new(),
            format: "gif".into(),
            damaged: false,
            texts: vec![],
        }
    }
}
impl CharacterState {
    pub fn characters(&self) -> Vec<String> {
        let mut names: Vec<_> = self
            .illustrations
            .iter()
            .map(|i| i.character.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }
    pub fn filtered(&self) -> Vec<usize> {
        self.illustrations
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                i.character == self.character
                    && i.format == self.format
                    && i.damaged == self.damaged
            })
            .map(|(n, _)| n)
            .collect()
    }
    pub fn selected_path(&self) -> Option<&Path> {
        self.selected.map(|i| self.illustrations[i].path.as_path())
    }
    pub fn select_first(&mut self) {
        let other = if self.format == "gif" { "png" } else { "gif" };
        self.selected = [
            (self.format.as_str(), self.damaged),
            (other, self.damaged),
            (self.format.as_str(), !self.damaged),
            (other, !self.damaged),
        ]
        .into_iter()
        .find_map(|(format, damaged)| {
            self.illustrations.iter().position(|ill| {
                ill.character == self.character && ill.format == format && ill.damaged == damaged
            })
        });
        if let Some(i) = self.selected {
            self.format.clone_from(&self.illustrations[i].format);
            self.damaged = self.illustrations[i].damaged;
        }
    }
    pub fn damage_counterpart(&self) -> Option<usize> {
        let current = self.illustrations.get(self.selected?)?;
        self.illustrations.iter().position(|ill| {
            ill.character == current.character
                && ill.format == current.format
                && ill.outfit.eq_ignore_ascii_case(&current.outfit)
                && ill.damaged != current.damaged
        })
    }
    pub fn toggle_damage(&mut self) {
        if let Some(i) = self.damage_counterpart() {
            self.selected = Some(i);
            self.damaged = self.illustrations[i].damaged;
        }
    }
    pub fn refresh(&mut self, root: &Path) {
        let old = self.selected.map(|i| self.illustrations[i].path.clone());
        self.illustrations = resources::illustrations(root);
        let names = self.characters();
        if !names.contains(&self.character) {
            self.character = agent_registry(root)
                .ok()
                .and_then(|registry| registry.default_character)
                .filter(|name| names.contains(name))
                .or_else(|| names.first().cloned())
                .unwrap_or_default();
        }
        if let Some(i) =
            old.and_then(|path| self.illustrations.iter().position(|ill| ill.path == path))
        {
            self.selected = Some(i);
            self.damaged = self.illustrations[i].damaged;
        } else {
            self.select_first();
        }
        self.texts = resources::texts(root, &self.character);
    }
    pub fn next_illustration(&mut self) {
        let indices = self.filtered();
        let next = indices
            .iter()
            .position(|i| Some(*i) == self.selected)
            .map_or(0, |current| (current + 1) % indices.len());
        self.selected = indices.get(next).copied();
    }
    pub fn toggle_format(&mut self) {
        let outfit = self.selected.map(|i| self.illustrations[i].outfit.clone());
        self.format = if self.format == "gif" { "png" } else { "gif" }.into();
        self.select_first();
        if let Some(i) = self.filtered().into_iter().find(|&i| {
            outfit
                .as_ref()
                .is_some_and(|outfit| self.illustrations[i].outfit.eq_ignore_ascii_case(outfit))
        }) {
            self.selected = Some(i);
        }
    }
    pub fn next_character(&mut self, root: &Path) {
        let names = self.characters();
        let index = names.iter().position(|s| s == &self.character).unwrap_or(0);
        if !names.is_empty() {
            self.character = names[(index + 1) % names.len()].clone();
            self.texts = resources::texts(root, &self.character);
            self.select_first();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_agents_resolve_independently_and_reject_invalid_markdown() {
        let root = std::env::temp_dir().join(format!("lexington-agents-{}", rand::random::<u64>()));
        let directory = root.join("resource/agents");
        fs::create_dir_all(&directory).unwrap();
        assert!(agent_instructions(&root, "a").unwrap().is_none());
        fs::write(
            directory.join("characters.json"),
            r#"{"defaultCharacter":"b","characters":{"a":"a.md","b":"b.md"}}"#,
        )
        .unwrap();
        fs::write(directory.join("a.md"), "角色甲\n只属于甲").unwrap();
        fs::write(directory.join("b.md"), "角色乙").unwrap();
        assert_eq!(
            agent_instructions(&root, "a").unwrap().as_deref(),
            Some("角色甲\n只属于甲")
        );
        assert_eq!(
            agent_instructions(&root, "b").unwrap().as_deref(),
            Some("角色乙")
        );
        assert!(agent_instructions(&root, "unconfigured").unwrap().is_none());
        for name in ["a", "b"] {
            let artwork = root.join("resource/illustration").join(name).join("png");
            fs::create_dir_all(&artwork).unwrap();
            fs::write(artwork.join("normal.png"), []).unwrap();
        }
        let mut library = CharacterState::default();
        library.refresh(&root);
        assert_eq!(library.character, "b");
        library.next_character(&root);
        assert_eq!(library.character, "a");
        library.refresh(&root);
        assert_eq!(library.character, "a");
        for contents in [Vec::new(), vec![b'x'; 65537], vec![0xff]] {
            fs::write(directory.join("a.md"), contents).unwrap();
            assert!(agent_instructions(&root, "a").is_err());
        }
        fs::remove_file(directory.join("a.md")).unwrap();
        assert!(agent_instructions(&root, "a").is_err());
        fs::write(
            directory.join("characters.json"),
            r#"{"characters":{"a":"../outside.md"}}"#,
        )
        .unwrap();
        assert!(agent_instructions(&root, "a").is_err());
        fs::write(directory.join("characters.json"), "invalid registry").unwrap();
        assert!(agent_instructions(&root, "a").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bundled_outfits_pair_both_ways_and_wrap_in_separate_queues() {
        let mut library = CharacterState::default();
        library.refresh(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(!library.damaged, "start with normal artwork");
        for (format, count) in [("gif", 7), ("png", 8)] {
            library.format = format.into();
            for damaged in [false, true] {
                library.damaged = damaged;
                library.select_first();
                let first = library.selected;
                let mut outfits = std::collections::BTreeSet::new();
                for _ in 0..count {
                    let index = library.selected.unwrap();
                    let current = &library.illustrations[index];
                    assert_eq!(current.format, format);
                    assert_eq!(current.damaged, damaged);
                    assert!(
                        outfits.insert(current.outfit.clone()),
                        "duplicate outfit in queue"
                    );
                    let counterpart = &library.illustrations[library.damage_counterpart().unwrap()];
                    assert_eq!(current.outfit, counterpart.outfit);
                    assert_eq!(current.character, counterpart.character);
                    assert_eq!(counterpart.format, format);
                    assert_ne!(current.damaged, counterpart.damaged);
                    library.toggle_damage();
                    assert_eq!(library.damaged, !damaged);
                    library.toggle_damage();
                    assert_eq!(library.selected, Some(index));
                    library.next_illustration();
                }
                assert_eq!(outfits.len(), count);
                assert_eq!(
                    library.selected, first,
                    "queue must wrap after every outfit"
                );
            }
        }
    }

    #[test]
    fn missing_pairs_stay_put_and_reload_recovers_with_matching_state_first() {
        let root = std::env::temp_dir().join(format!("lexington-pairs-{}", rand::random::<u64>()));
        for path in [
            "a/gif/A_Normal.gif",
            "a/png/A_Damage.png",
            "b/gif/A_Damage.gif",
            "a/gif/Summer_Event_Normal.gif",
            "a/gif/summer_event_dAmAgE.GIF",
            "a/gif/Legacy.gif",
        ] {
            let path = root.join("resource/illustration").join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, []).unwrap();
        }
        let mut library = CharacterState::default();
        library.refresh(&root);
        assert_eq!(
            library.selected_path().unwrap().file_name().unwrap(),
            "A_Normal.gif"
        );
        assert!(
            library.damage_counterpart().is_none(),
            "never pair across characters or formats"
        );
        let old = library.selected;
        library.toggle_damage();
        assert_eq!(library.selected, old);
        assert!(!library.damaged);
        library.next_illustration();
        assert_eq!(
            library.illustrations[library.selected.unwrap()].outfit,
            "Legacy"
        );
        library.next_illustration();
        assert_eq!(
            library.illustrations[library.selected.unwrap()].outfit,
            "Summer_Event"
        );
        library.toggle_damage();
        assert_eq!(
            library.selected_path().unwrap().file_name().unwrap(),
            "summer_event_dAmAgE.GIF"
        );
        library.next_illustration();
        assert_eq!(library.filtered().len(), 1);
        assert!(library.damaged);
        library.toggle_format();
        assert_eq!(library.format, "png");
        assert!(library.damaged);
        assert_eq!(
            library.selected_path().unwrap().file_name().unwrap(),
            "A_Damage.png"
        );
        library.next_character(&root);
        assert_eq!(library.character, "b");
        assert_eq!(library.format, "gif");
        assert!(library.damaged);
        assert!(library.damage_counterpart().is_none());
        // Removing the selected character and the GIF damage queue should prefer
        // PNG damage over jumping into the normal GIF queue.
        std::fs::remove_dir_all(root.join("resource/illustration/b")).unwrap();
        std::fs::remove_file(root.join("resource/illustration/a/gif/summer_event_dAmAgE.GIF"))
            .unwrap();
        library.refresh(&root);
        assert_eq!(library.character, "a");
        assert_eq!(library.format, "png");
        assert!(library.damaged);
        std::fs::remove_file(root.join("resource/illustration/a/png/A_Damage.png")).unwrap();
        library.refresh(&root);
        assert_eq!(library.format, "gif");
        assert!(
            !library.damaged,
            "fall back to normal only if damage is unavailable"
        );
        std::fs::remove_dir_all(&root).unwrap();
        library.refresh(&root);
        library.next_illustration();
        library.toggle_damage();
        assert!(library.selected.is_none());
    }
}
