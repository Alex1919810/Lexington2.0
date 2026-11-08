use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct AgentRegistry {
    default_character: Option<String>,
    characters: BTreeMap<String, PathBuf>,
}

pub(super) fn read_agent_file(path: &Path) -> Result<String, String> {
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
    if let Some(card) = super::RoleCard::load(root, character)? {
        return Ok(Some(card.instructions()));
    }
    let Some(path) = agent_path(root, character)? else {
        return Ok(None);
    };
    let text = read_agent_file(&path)?;
    if text.trim().is_empty() {
        return Err("当前角色的 Markdown 设定为空。".into());
    }
    Ok(Some(text))
}

pub(super) fn agent_path(root: &Path, character: &str) -> Result<Option<PathBuf>, String> {
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
    Ok(Some(path))
}

pub fn default_character(root: &Path) -> Result<Option<String>, String> {
    Ok(agent_registry(root)?.default_character)
}
