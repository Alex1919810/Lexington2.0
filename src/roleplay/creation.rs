use super::RoleCard;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn valid_name(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.ends_with(['.', ' '])
        && !name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str())
        && !(base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

pub(super) fn load_created_card(root: &Path, character: &str) -> Result<Option<RoleCard>, String> {
    if !valid_name(character) {
        return Ok(None);
    }
    let path = root
        .join("config/characters")
        .join(character)
        .join("card.json");
    if !path.try_exists().map_err(|_| "无法检查用户角色卡。")? {
        return Ok(None);
    }
    let text = super::registry::read_agent_file(&path)?;
    let card: RoleCard = serde_json::from_str(&text).map_err(|_| "用户角色卡格式错误。")?;
    card.validate()?;
    Ok(Some(card))
}

/// Publish a complete personal character directory only after every artwork validates.
pub fn create_character(
    root: &Path,
    card: &RoleCard,
    artworks: &[PathBuf],
) -> Result<String, String> {
    let name = card.name.trim();
    if !valid_name(name) {
        return Err("请输入有效角色名称，不能使用路径、系统保留名或特殊文件名字符。".into());
    }
    if artworks.is_empty() || artworks.len() > 32 {
        return Err("请导入至少一张立绘，每次最多 32 张（PNG / GIF）。".into());
    }
    let mut card = card.clone();
    card.name = name.into();
    card.validate()?;
    for parent in [
        root.join("resource/illustration"),
        root.join("config/characters"),
    ] {
        match fs::read_dir(parent) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.map_err(|_| "无法检查已有角色。")?;
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(name)
                    {
                        return Err("已存在同名角色，请使用其他名称。".into());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("无法检查已有角色。".into()),
        }
    }
    if RoleCard::load(root, name)?.is_some() || super::registry::agent_path(root, name)?.is_some() {
        return Err("已存在同名角色卡，请使用其他名称。".into());
    }
    let parent = root.join("config/characters");
    fs::create_dir_all(&parent).map_err(|_| "无法创建用户角色目录。")?;
    let stage = tempfile::Builder::new()
        .prefix(".creating-")
        .tempdir_in(&parent)
        .map_err(|_| "无法创建角色导入目录。")?;
    let mut total = 0;
    let mut names = Vec::new();
    for artwork in artworks {
        let format = artwork
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .filter(|s| matches!(s.as_str(), "png" | "gif"))
            .ok_or("立绘仅支持 PNG / GIF。")?;
        let filename = artwork.file_name().ok_or("立绘文件名无效。")?;
        let key = format!("{format}/{}", filename.to_string_lossy().to_lowercase());
        if names.contains(&key) {
            return Err("所选立绘中存在同名文件，请先重命名。".into());
        }
        names.push(key);
        let input = fs::File::open(artwork).map_err(|_| "无法读取所选立绘。")?;
        let metadata = input.metadata().map_err(|_| "无法检查立绘大小。")?;
        const MAX_FILE: u64 = 64 * 1024 * 1024;
        if !metadata.is_file() || metadata.len() > MAX_FILE {
            return Err("单张立绘不能超过 64 MiB，且必须为文件。".into());
        }
        let directory = stage.path().join(&format);
        fs::create_dir_all(&directory).map_err(|_| "无法创建立绘目录。")?;
        let target = directory.join(filename);
        let mut output = fs::File::create(&target).map_err(|_| "无法导入立绘。")?;
        let copied = std::io::copy(&mut input.take(MAX_FILE + 1), &mut output)
            .and_then(|n| output.sync_all().map(|_| n))
            .map_err(|_| "立绘复制失败。")?;
        total += copied;
        if copied > MAX_FILE || total > 256 * 1024 * 1024 {
            return Err("立绘总大小不能超过 256 MiB，单张不能超过 64 MiB。".into());
        }
        drop(output);
        let (width, height) = image::image_dimensions(&target).map_err(|_| "立绘文件无法解码。")?;
        if u64::from(width) * u64::from(height) > 16 * 1024 * 1024 {
            return Err("立绘像素总数不能超过 1600 万。".into());
        }
        crate::tools::gif::Decoder::load(&target).map_err(|_| "立绘文件无法解码。")?;
    }
    let mut file =
        fs::File::create(stage.path().join("card.json")).map_err(|_| "无法写入新角色卡。")?;
    let data = serde_json::to_vec(&card).map_err(|_| "角色卡无法保存。")?;
    file.write_all(&data)
        .and_then(|_| file.sync_all())
        .map_err(|_| "新角色卡保存失败。")?;
    drop(file);
    let target = parent.join(name);
    if target.try_exists().map_err(|_| "无法检查角色目录。")? {
        return Err("已存在同名角色，请使用其他名称。".into());
    }
    fs::rename(stage.path(), target).map_err(|_| "新角色创建失败，已有角色未修改。")?;
    Ok(name.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_requires_decodable_artwork_and_preserves_existing_characters() {
        let root = tempfile::tempdir().unwrap();
        let artwork = root.path().join("外部立绘.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(&artwork)
            .unwrap();
        let card = RoleCard {
            name: "新角色".into(),
            description: "独立人格".into(),
            ..Default::default()
        };
        assert!(create_character(root.path(), &card, &[]).is_err());
        assert!(!root.path().join("config").exists());
        let bad = root.path().join("损坏.png");
        fs::write(&bad, "not an image").unwrap();
        assert!(create_character(root.path(), &card, &[artwork.clone(), bad]).is_err());
        assert_eq!(
            fs::read_dir(root.path().join("config/characters"))
                .unwrap()
                .count(),
            0
        );
        let config = root.path().join("config/roleplay.json");
        fs::write(
            &config,
            r#"{"future":9,"roleCards":{"existing":{"name":"existing"}}}"#,
        )
        .unwrap();
        let before = fs::read(&config).unwrap();
        assert_eq!(
            create_character(root.path(), &card, std::slice::from_ref(&artwork)).unwrap(),
            "新角色"
        );
        assert_eq!(
            RoleCard::load(root.path(), "新角色")
                .unwrap()
                .unwrap()
                .description,
            "独立人格"
        );
        assert_eq!(fs::read(&config).unwrap(), before);
        assert!(create_character(root.path(), &card, std::slice::from_ref(&artwork)).is_err());
        let mut changed = card.clone();
        changed.description = "编辑后的设定".into();
        super::super::save_role_card(root.path(), "新角色", &changed).unwrap();
        assert_eq!(
            RoleCard::load(root.path(), "新角色")
                .unwrap()
                .unwrap()
                .description,
            "编辑后的设定"
        );
        for name in ["../escape", "CON", "a/b", "a\\b", "trailing."] {
            let mut invalid = card.clone();
            invalid.name = name.into();
            assert!(
                create_character(root.path(), &invalid, std::slice::from_ref(&artwork)).is_err()
            );
        }
        assert_eq!(
            fs::read(&artwork).unwrap(),
            fs::read(
                root.path()
                    .join("config/characters/新角色/png/外部立绘.png")
            )
            .unwrap()
        );
        let animated = root.path().join("动画_Normal.gif");
        image::codecs::gif::GifEncoder::new(fs::File::create(&animated).unwrap())
            .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                4,
                4,
                image::Rgba([1, 2, 3, 255]),
            )))
            .unwrap();
        let mut animated_card = card.clone();
        animated_card.name = "动画角色".into();
        create_character(root.path(), &animated_card, &[animated]).unwrap();
        fs::write(&config, "broken json").unwrap();
        animated_card.name = "不能创建".into();
        assert!(create_character(root.path(), &animated_card, &[artwork]).is_err());
        assert_eq!(fs::read(&config).unwrap(), b"broken json");
        assert!(!root.path().join("config/characters/不能创建").exists());
    }
}
