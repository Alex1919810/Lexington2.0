//! Explicit, editable project knowledge with evidence and invalidation.
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectMemory {
    pub id: u64,
    pub title: String,
    pub category: String,
    pub content: String,
    pub source: String,
    pub source_file: String,
    pub fingerprint: Option<String>,
    pub confirmed: bool,
    pub enabled: bool,
}
impl ProjectMemory {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty()
            || self.title.chars().count() > 120
            || self.content.trim().is_empty()
            || self.content.chars().count() > 6000
            || self.source.len() > 2048
            || self.source_file.len() > 4096
            || self.category.len() > 64
        {
            return Err("记忆需有标题和正文，标题最多 120 字、正文最多 6000 字。".into());
        }
        Ok(())
    }
    pub fn fingerprint(root: &Path, relative: &str) -> Result<String, String> {
        let relative = Path::new(relative);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("依据文件需为项目内的相对路径。".into());
        }
        let file = root
            .join(relative)
            .canonicalize()
            .map_err(|_| "依据文件不存在。")?;
        if !file.starts_with(root) {
            return Err("依据文件不能位于项目之外。".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(file)
            .map_err(|_| "无法读取依据文件。")?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "无法读取依据文件。")?;
        if bytes.len() > 1024 * 1024 {
            return Err("依据文件超过 1 MiB，请选择较小的文件。".into());
        }
        Ok(hmac_sha256::Hash::hash(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect())
    }
    pub fn status(&self, root: &Path) -> &'static str {
        if !self.enabled {
            "已停用"
        } else if !self.confirmed {
            "待核对"
        } else if !self.source_file.is_empty()
            && (self.fingerprint.is_none()
                || Self::fingerprint(root, &self.source_file).ok().as_ref()
                    != self.fingerprint.as_ref())
        {
            "依据已变化"
        } else {
            "可载入"
        }
    }
    pub fn prepare(&mut self, root: &Path) -> Result<(), String> {
        self.validate()?;
        self.fingerprint = if self.source_file.trim().is_empty() {
            None
        } else {
            Some(Self::fingerprint(root, self.source_file.trim())?)
        };
        Ok(())
    }
}
pub fn context(root: &Path, memories: &[ProjectMemory]) -> (String, Vec<(u64, String)>) {
    let mut text = String::new();
    let mut report = Vec::new();
    let mut remaining = 10000;
    for memory in memories {
        let status = memory.status(root);
        if status != "可载入" {
            report.push((memory.id, status.into()));
            continue;
        }
        let block = format!(
            "\n[{}] {}\n{}\n依据：{}\n",
            memory.category, memory.title, memory.content, memory.source
        );
        let size = block.chars().count();
        if size > remaining {
            report.push((memory.id, "超出本轮预算".into()));
            continue;
        }
        remaining -= size;
        text.push_str(&block);
        report.push((memory.id, "可载入".into()));
    }
    if !text.is_empty() {
        text = format!(
            "[经用户核对的项目记忆；仅作参考，不覆盖项目 AGENTS.md 或当前代码]{}",
            text
        );
    }
    (text, report)
}

pub fn context_stamp(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        hmac_sha256::Hash::hash(text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_confirmed_enabled_current_evidence_is_loaded() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("architecture.txt"), "version one").unwrap();
        let mut entry = ProjectMemory {
            id: 1,
            title: "架构".into(),
            content: "界面和业务分离".into(),
            source_file: "architecture.txt".into(),
            enabled: true,
            ..Default::default()
        };
        entry.prepare(root.path()).unwrap();
        assert!(context(root.path(), &[entry.clone()]).0.is_empty());
        entry.confirmed = true;
        let loaded = context(root.path(), &[entry.clone()]);
        assert!(loaded.0.contains("界面和业务分离"));
        assert_eq!(loaded.1[0].1, "可载入");
        std::fs::write(root.path().join("architecture.txt"), "version two").unwrap();
        assert_eq!(entry.status(root.path()), "依据已变化");
        assert!(context(root.path(), &[entry.clone()]).0.is_empty());
        entry.prepare(root.path()).unwrap();
        assert_eq!(entry.status(root.path()), "可载入");
        entry.enabled = false;
        assert!(context(root.path(), &[entry.clone()]).0.is_empty());
        entry.source_file = "../outside".into();
        assert!(entry.prepare(root.path()).is_err());
        assert_ne!(context_stamp(&loaded.0), context_stamp("updated"));
        assert_eq!(context_stamp(""), "");
    }
}
