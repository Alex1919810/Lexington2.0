use std::{
    fs,
    path::{Path, PathBuf},
};

fn entries(path: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    files.sort();
    files
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Illustration {
    pub path: PathBuf,
    pub character: String,
    pub format: String,
    pub outfit: String,
    pub damaged: bool,
}
pub fn illustrations(root: &Path) -> Vec<Illustration> {
    let mut result = Vec::new();
    for dir in entries(&root.join("resource/illustration"))
        .into_iter()
        .chain(entries(&root.join("config/characters")))
    {
        if !dir.is_dir()
            || dir
                .file_name()
                .is_none_or(|name| name.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        let character = dir.file_name().unwrap().to_string_lossy().into_owned();
        for format in ["gif", "png", "live2d"] {
            for path in entries(&dir.join(format)) {
                if format == "live2d" {
                    if path.is_dir() && path.join("actor.json").is_file() {
                        let variant = path.file_name().unwrap().to_string_lossy();
                        let (outfit, state) = variant
                            .rsplit_once('_')
                            .filter(|(_, s)| {
                                s.eq_ignore_ascii_case("normal") || s.eq_ignore_ascii_case("damage")
                            })
                            .unwrap_or((&variant, "normal"));
                        result.push(Illustration {
                            path: path.join("actor.json"),
                            character: character.clone(),
                            format: format.into(),
                            outfit: outfit.into(),
                            damaged: state.eq_ignore_ascii_case("damage"),
                        });
                    }
                    continue;
                }
                if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case(format))
                {
                    let stem = path.file_stem().unwrap().to_string_lossy();
                    // Only the final state suffix belongs to the pairing key.
                    // In particular, Normal_Normal is the normal state of outfit Normal.
                    let variant = stem.rsplit_once('_').filter(|(_, state)| {
                        state.eq_ignore_ascii_case("normal") || state.eq_ignore_ascii_case("damage")
                    });
                    let outfit = variant
                        .map_or(stem.as_ref(), |(outfit, _)| outfit)
                        .to_owned();
                    let damaged =
                        variant.is_some_and(|(_, state)| state.eq_ignore_ascii_case("damage"));
                    result.push(Illustration {
                        path,
                        character: character.clone(),
                        format: format.into(),
                        outfit,
                        damaged,
                    });
                }
            }
        }
    }
    result
}
pub fn background(root: &Path, kind: &str) -> Option<PathBuf> {
    entries(&root.join("resource/background").join(kind))
        .into_iter()
        .find(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    ["png", "jpg", "jpeg", "gif", "webp"]
                        .iter()
                        .any(|x| e.eq_ignore_ascii_case(x))
                })
        })
}
pub fn texts(root: &Path, character: &str) -> Vec<String> {
    // Character names originate in directory scanning; reject traversal for other callers too.
    if character.is_empty() || Path::new(character).components().count() != 1 || character == ".." {
        return vec![];
    }
    fs::read(
        root.join("resource/texts")
            .join(character)
            .join("text.json"),
    )
    .ok()
    .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
    .unwrap_or_default()
    .into_iter()
    .filter(|s| !s.trim().is_empty())
    .collect()
}
