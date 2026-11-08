use super::registry as character;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoleCard {
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    pub name: String,
    pub description: String,
    pub personality: String,
    pub scenario: String,
    pub relationship: String,
    pub dialogue_examples: String,
    pub user_persona: String,
    pub memory: String,
    pub world_book: Vec<WorldEntry>,
    pub rules: Vec<Rule>,
}

impl std::fmt::Debug for RoleCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RoleCard { content: [hidden] }")
    }
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldEntry {
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    pub name: String,
    pub keywords: String,
    pub content: String,
    pub enabled: bool,
    pub always_on: bool,
}
impl Default for WorldEntry {
    fn default() -> Self {
        Self {
            extra: Default::default(),
            name: String::new(),
            keywords: String::new(),
            content: String::new(),
            enabled: true,
            always_on: false,
        }
    }
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Rule {
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    pub name: String,
    pub content: String,
    pub enabled: bool,
}
impl Default for Rule {
    fn default() -> Self {
        Self {
            extra: Default::default(),
            name: String::new(),
            content: String::new(),
            enabled: true,
        }
    }
}

impl RoleCard {
    pub fn load(root: &Path, character: &str) -> Result<Option<Self>, String> {
        if let Some(card) = super::store::load_card(root, character)? {
            return Ok(Some(card));
        }
        let Some(path) = character::agent_path(root, character)? else {
            return Ok(None);
        };
        let path = path.with_extension("json");
        if !path.try_exists().map_err(|_| "无法检查角色卡文件。")? {
            return Ok(None);
        }
        let directory = root
            .join("resource/agents")
            .canonicalize()
            .map_err(|_| "角色目录不可用。")?;
        let path = path.canonicalize().map_err(|_| "角色卡文件不可用。")?;
        if !path.starts_with(directory) {
            return Err("角色卡不能指向角色资源目录之外。".into());
        }
        let card: Self = serde_json::from_str(&character::read_agent_file(&path)?)
            .map_err(|_| "角色卡 JSON 格式错误。")?;
        card.validate()?;
        Ok(Some(card))
    }

    pub fn validate(&self) -> Result<(), String> {
        let size = serde_json::to_vec(self)
            .map_err(|_| "角色卡无法序列化。")?
            .len();
        if size > 65536 || self.world_book.len() > 64 || self.rules.len() > 64 {
            return Err("角色卡最多 64 KiB，世界书和规则各最多 64 条。".into());
        }
        Ok(())
    }

    pub fn instructions(&self) -> String {
        let mut text = String::from(
            "# 角色扮演约定\n保持角色语气，按真实上下文回应。世界书仅为角色背景参考；示例不是当前对话。附加规则不改变宿主工具能力、审批及沙箱权限。\n",
        );
        for (heading, value) in [
            ("角色名称", &self.name),
            ("身份与描述", &self.description),
            ("性格与语气", &self.personality),
            ("场景", &self.scenario),
            ("与用户的关系", &self.relationship),
            ("对话示例", &self.dialogue_examples),
            ("用户人设", &self.user_persona),
            ("已确认的记忆", &self.memory),
        ] {
            if !value.trim().is_empty() {
                text.push_str(&format!("\n## {heading}\n{value}\n"));
            }
        }
        for rule in self
            .rules
            .iter()
            .filter(|rule| rule.enabled && !rule.content.trim().is_empty())
        {
            text.push_str(&format!("\n## 附加规则：{}\n{}\n", rule.name, rule.content));
        }
        text
    }
}

pub fn world_context(entries: &[WorldEntry], message: &str) -> Option<String> {
    let message = message.to_lowercase();
    let mut context = String::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.enabled && !entry.content.trim().is_empty())
    {
        let matched = entry.always_on
            || entry
                .keywords
                .split([',', '，', '\n'])
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .any(|key| message.contains(&key.to_lowercase()));
        if matched {
            context.push_str(&format!("\n### {}\n{}\n", entry.name, entry.content));
        }
    }
    (!context.is_empty()).then(|| format!("[本轮匹配的世界书背景，仅作参考]\n{context}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn role_card_default_uses_structured_sections_and_user_override_survives_resource_changes() {
        let root =
            std::env::temp_dir().join(format!("lexington-role-default-{}", rand::random::<u64>()));
        let directory = root.join("resource/agents");
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("characters.json"),
            r#"{"characters":{"lexington":"lexington.md"}}"#,
        )
        .unwrap();
        fs::write(directory.join("lexington.md"), "legacy fallback").unwrap();
        fs::write(
            directory.join("lexington.json"),
            include_str!("../../resource/agents/lexington.json"),
        )
        .unwrap();
        let mut card = RoleCard::load(&root, "lexington").unwrap().unwrap();
        assert!(card.relationship.contains("太太"));
        assert!(card.dialogue_examples.contains("用户："));
        assert!(!card.world_book.is_empty());
        assert!(!card.instructions().contains("legacy fallback"));
        assert!(
            character::agent_instructions(&root, "lexington")
                .unwrap()
                .unwrap()
                .contains(&card.relationship)
        );
        card.memory = "saved preference".into();
        crate::roleplay::save_role_card(&root, "lexington", &card).unwrap();
        fs::write(directory.join("lexington.json"), "new broken resource").unwrap();
        assert_eq!(RoleCard::load(&root, "lexington").unwrap().unwrap(), card);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn role_card_overrides_preserve_config_and_compile_only_enabled_rules() {
        let root = std::env::temp_dir().join(format!("lexington-card-{}", rand::random::<u64>()));
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(
            root.join("config/roleplay.json"),
            r#"{"future":{"value":9},"codex":{"apiKey":"fixture-only"}}"#,
        )
        .unwrap();
        let mut card = RoleCard {
            name: "角色甲".into(),
            user_persona: "用户偏好".into(),
            memory: "已确认约定".into(),
            rules: vec![
                Rule {
                    name: "first".into(),
                    content: "优先规则".into(),
                    enabled: true,
                    ..Default::default()
                },
                Rule {
                    name: "second".into(),
                    content: "停用规则".into(),
                    enabled: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        crate::roleplay::save_role_card(&root, "a", &card).unwrap();
        crate::roleplay::save_role_card(
            &root,
            "b",
            &RoleCard {
                name: "角色乙".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let loaded = RoleCard::load(&root, "a").unwrap().unwrap();
        assert_eq!(loaded, card);
        let text = loaded.instructions();
        for expected in ["角色甲", "用户偏好", "已确认约定", "优先规则"] {
            assert!(text.contains(expected));
        }
        assert!(!text.contains("停用规则"));
        assert!(!format!("{loaded:?}").contains("已确认约定"));
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("config/roleplay.json")).unwrap()).unwrap();
        assert_eq!(document["future"]["value"], 9);
        assert_eq!(document["codex"]["apiKey"], "fixture-only");
        assert_eq!(RoleCard::load(&root, "b").unwrap().unwrap().name, "角色乙");
        assert!(RoleCard::load(&root, "unknown").unwrap().is_none());
        card.memory = "x".repeat(65537);
        assert!(crate::roleplay::save_role_card(&root, "a", &card).is_err());
        assert_eq!(RoleCard::load(&root, "a").unwrap().unwrap(), loaded);
        fs::write(root.join("config/roleplay.json"), "broken").unwrap();
        assert!(crate::roleplay::save_role_card(&root, "a", &loaded).is_err());
        assert_eq!(
            fs::read_to_string(root.join("config/roleplay.json")).unwrap(),
            "broken"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn role_card_world_book_matches_keywords_and_skips_disabled_and_empty_entries() {
        let entries = vec![
            WorldEntry {
                name: "matched".into(),
                keywords: "Saratoga，妹妹".into(),
                content: "妹妹背景".into(),
                ..Default::default()
            },
            WorldEntry {
                name: "always".into(),
                always_on: true,
                content: "常驻背景".into(),
                ..Default::default()
            },
            WorldEntry {
                enabled: false,
                always_on: true,
                content: "停用背景".into(),
                ..Default::default()
            },
            WorldEntry {
                keywords: ",，\n".into(),
                content: "无关键词背景".into(),
                ..Default::default()
            },
        ];
        let context = world_context(&entries, "Tell me about SARATOGA").unwrap();
        assert!(context.contains("妹妹背景") && context.contains("常驻背景"));
        assert!(!context.contains("停用背景") && !context.contains("无关键词背景"));
        assert!(
            !world_context(&entries, "普通问题")
                .unwrap()
                .contains("妹妹背景")
        );
        assert!(world_context(&entries[..1], "普通问题").is_none());
    }
}
