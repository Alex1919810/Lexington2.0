use super::RoleCard;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UserProfile {
    pub persona: String,
    pub memories: Vec<Memory>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Memory {
    pub id: String,
    pub text: String,
    pub source: String,
    pub confirmed: bool,
    pub enabled: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Memory {
    fn default() -> Self {
        Self {
            id: format!("{:016x}", rand::random::<u64>()),
            text: String::new(),
            source: "手动录入".into(),
            confirmed: false,
            enabled: true,
            extra: BTreeMap::new(),
        }
    }
}
impl std::fmt::Debug for UserProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UserProfile { content: [hidden] }")
    }
}
impl UserProfile {
    pub fn load(root: &Path, character: &str, legacy: Option<&RoleCard>) -> Result<Self, String> {
        let document = super::store::read_document(root, "roleplay-memory.json")?;
        if let Some(value) = document
            .get("profiles")
            .and_then(|profiles| profiles.get(character))
        {
            let profile: Self =
                serde_json::from_value(value.clone()).map_err(|_| "用户人设与记忆格式错误。")?;
            profile.validate()?;
            return Ok(profile);
        }
        if document
            .get("profiles")
            .is_some_and(|profiles| !profiles.is_object())
        {
            return Err("记忆分组必须为对象。".into());
        }
        let mut profile = Self::default();
        if let Some(card) = legacy {
            profile.persona = card.user_persona.clone();
            if !card.memory.trim().is_empty() {
                profile.memories.push(Memory {
                    id: "legacy".into(),
                    text: card.memory.clone(),
                    source: "旧版手动记忆".into(),
                    confirmed: true,
                    ..Default::default()
                });
            }
        }
        Ok(profile)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.memories.len() > 64
            || serde_json::to_vec(self)
                .map_err(|_| "记忆无法保存。")?
                .len()
                > 65536
        {
            return Err("每个角色的用户人设与记忆最多 64 KiB、64 条记忆。".into());
        }
        Ok(())
    }
    pub fn memory_text(&self) -> String {
        self.memories
            .iter()
            .filter(|memory| memory.enabled && memory.confirmed && !memory.text.trim().is_empty())
            .map(|memory| format!("- {}", memory.text))
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn save(&self, root: &Path, character: &str) -> Result<(), String> {
        self.validate()?;
        super::store::save_section(
            root,
            "roleplay-memory.json",
            "profiles",
            character,
            serde_json::to_value(self).map_err(|_| "记忆无法保存。")?,
        )
    }
}

pub fn session_card(root: &Path, character: &str) -> Result<Option<RoleCard>, String> {
    let mut card = RoleCard::load(root, character)?;
    let profile = UserProfile::load(root, character, card.as_ref())?;
    let memory = profile.memory_text();
    if card.is_none() && (!profile.persona.is_empty() || !memory.is_empty()) {
        card = Some(RoleCard {
            name: character.into(),
            description: super::agent_instructions(root, character)?.unwrap_or_default(),
            ..Default::default()
        });
    }
    if let Some(card) = &mut card {
        card.user_persona = profile.persona;
        card.memory = memory;
    }
    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn profile_confirmation_revocation_isolation_and_legacy_fallback_survive_restart() {
        let root =
            std::env::temp_dir().join(format!("lexington-profile-{}", rand::random::<u64>()));
        let dir = root.join("resource/agents");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("characters.json"),
            r#"{"characters":{"a":"a.md"}}"#,
        )
        .unwrap();
        fs::write(dir.join("a.md"), "legacy identity").unwrap();
        let mut profile = UserProfile {
            persona: "user preference".into(),
            memories: vec![Memory {
                text: "candidate fact".into(),
                source: "private source".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        profile.save(&root, "a").unwrap();
        let card = session_card(&root, "a").unwrap().unwrap();
        assert!(card.description.contains("legacy identity"));
        assert!(!card.instructions().contains("candidate fact"));
        assert!(session_card(&root, "b").unwrap().is_none());
        profile.memories[0].confirmed = true;
        profile.save(&root, "a").unwrap();
        let card = session_card(&root, "a").unwrap().unwrap();
        assert!(card.instructions().contains("candidate fact"));
        assert!(!card.instructions().contains("private source"));
        profile.memories[0].confirmed = false;
        profile.save(&root, "a").unwrap();
        assert!(
            !session_card(&root, "a")
                .unwrap()
                .unwrap()
                .instructions()
                .contains("candidate fact")
        );
        let saved = fs::read(root.join("config/roleplay-memory.json")).unwrap();
        profile.memories[0].text = "x".repeat(65537);
        assert!(profile.save(&root, "a").is_err());
        assert_eq!(
            fs::read(root.join("config/roleplay-memory.json")).unwrap(),
            saved
        );
        fs::write(root.join("config/roleplay-memory.json"), "broken").unwrap();
        assert!(UserProfile::default().save(&root, "a").is_err());
        assert_eq!(
            fs::read_to_string(root.join("config/roleplay-memory.json")).unwrap(),
            "broken"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
