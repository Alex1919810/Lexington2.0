use super::RoleCard;
use serde::Deserialize;
use std::{collections::BTreeMap, fs, io::Write, path::Path};

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Overrides {
    role_cards: BTreeMap<String, RoleCard>,
}

pub(super) fn load_card(root: &Path, character: &str) -> Result<Option<RoleCard>, String> {
    let path = root.join("config/roleplay.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("无法读取角色配置。".into()),
    };
    let mut document: Overrides =
        serde_json::from_slice(&bytes).map_err(|_| "角色配置格式错误。")?;
    let card = document.role_cards.remove(character);
    if let Some(card) = &card {
        card.validate()?;
    }
    Ok(card)
}

pub fn save_role_card(root: &Path, character: &str, card: &RoleCard) -> Result<(), String> {
    if character.trim().is_empty() || character.len() > 200 {
        return Err("角色标识无效。".into());
    }
    card.validate()?;
    let path = root.join("config/roleplay.json");
    let mut document: serde_json::Value = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "角色配置格式错误，未保存。")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(_) => return Err("无法读取角色配置，未保存。".into()),
    };
    document
        .as_object_mut()
        .ok_or("角色配置必须为对象。")?
        .entry("roleCards")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or("角色卡分组必须为对象。")?
        .insert(
            character.into(),
            serde_json::to_value(card).map_err(|_| "角色卡无法保存。")?,
        );
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|_| "角色配置无法保存。")?;
    bytes.push(b'\n');
    fs::create_dir_all(root.join("config")).map_err(|_| "无法创建角色配置目录。")?;
    let temporary = path.with_extension(format!("{}.tmp", rand::random::<u64>()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|_| "无法写入角色配置。")?;
    let written = file.write_all(&bytes).and_then(|_| file.sync_all());
    drop(file);
    let result = written.and_then(|_| fs::rename(&temporary, &path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|_| "角色配置保存失败，原文件保留。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_card_storage_preserves_unknown_fields_and_leaves_codex_configuration_untouched() {
        let root =
            std::env::temp_dir().join(format!("lexington-role-store-{}", rand::random::<u64>()));
        fs::create_dir_all(root.join("config")).unwrap();
        let basic = b"{\"codex\":{\"apiKey\":\"fixture-only\"}}";
        fs::write(root.join("config/config.json"), basic).unwrap();
        fs::write(root.join("config/roleplay.json"), r#"{"future":9,"roleCards":{"a":{"name":"a","futureCard":true,"rules":[{"name":"rule","content":"hello","enabled":true,"futureRule":7}]}}}"#).unwrap();
        let mut card = load_card(&root, "a").unwrap().unwrap();
        card.memory = "saved note".into();
        save_role_card(&root, "a", &card).unwrap();
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("config/roleplay.json")).unwrap()).unwrap();
        assert_eq!(document["future"], 9);
        assert_eq!(document["roleCards"]["a"]["futureCard"], true);
        assert_eq!(document["roleCards"]["a"]["rules"][0]["futureRule"], 7);
        assert_eq!(fs::read(root.join("config/config.json")).unwrap(), basic);
        assert_eq!(fs::read_dir(root.join("config")).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
