//! Editor-provided diagnostic snapshots, scoped to a canonical project directory.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
    pub source: String,
    pub code: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_validate_scope_positions_and_invalidate_changed_files() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = root.path().join("中文.rs");
        std::fs::write(&path, "before").unwrap();
        let mut snapshot = DiagnosticSnapshot {
            captured_at: 10,
            documents: vec![DocumentDiagnostics {
                uri: url::Url::from_file_path(&path).unwrap().to_string(),
                path: Some(path.clone()),
                version: Some(4),
                diagnostics: vec![Diagnostic {
                    severity: "error".into(),
                    message: "类型不匹配".into(),
                    start_line: 2,
                    start_column: 3,
                    end_line: 2,
                    end_column: 5,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        snapshot.documents[0].diagnostics[0].source = "源".repeat(128);
        snapshot.capture(root.path()).unwrap();
        assert_eq!(snapshot.count(), 1);
        assert!(snapshot.context().contains("类型不匹配"));
        assert!(snapshot.validate(Some(outside.path())).is_err());
        std::fs::write(&path, "changed and longer").unwrap();
        assert!(snapshot.documents[0].stale());
        assert!(!snapshot.context().contains("类型不匹配"));
        snapshot.documents[0].dirty = true;
        assert!(snapshot.context().contains("未保存的编辑器版本"));
        snapshot.documents[0].diagnostics[0].end_column = 1;
        assert!(snapshot.validate(None).is_err());
        snapshot.documents.clear();
        assert_eq!(snapshot.count(), 0);
        assert!(snapshot.validate(None).is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn diagnostic_symlinks_cannot_claim_a_different_project() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        let link = root.path().join("escape.rs");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let snapshot = DiagnosticSnapshot {
            documents: vec![DocumentDiagnostics {
                uri: url::Url::from_file_path(&link).unwrap().to_string(),
                path: Some(link),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(snapshot.validate(Some(root.path())).is_err());
    }
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DocumentDiagnostics {
    pub uri: String,
    pub path: Option<PathBuf>,
    pub version: Option<i64>,
    pub dirty: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub disk_stamp: Option<String>,
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiagnosticSnapshot {
    pub captured_at: u64,
    pub truncated: bool,
    pub documents: Vec<DocumentDiagnostics>,
}
pub fn disk_stamp(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let changed = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(format!("{}:{}", metadata.len(), changed.as_nanos()))
}
impl DocumentDiagnostics {
    pub fn stale(&self) -> bool {
        !self.dirty
            && self.path.as_ref().is_some_and(|path| {
                self.disk_stamp.is_none() || disk_stamp(path) != self.disk_stamp
            })
    }
}
impl DiagnosticSnapshot {
    pub fn validate(&self, root: Option<&Path>) -> Result<(), String> {
        if self.documents.len() > 100
            || self
                .documents
                .iter()
                .map(|d| d.diagnostics.len())
                .sum::<usize>()
                > 200
        {
            return Err("诊断最多包含 100 个文档、200 个问题。".into());
        }
        for document in &self.documents {
            if document.uri.len() > 8192 {
                return Err("诊断文档来源过长。".into());
            }
            if let Some(path) = &document.path {
                if !path.is_absolute() || path.as_os_str().to_string_lossy().contains('\0') {
                    return Err("诊断文件必须是本地绝对路径。".into());
                }
                if let Some(root) = root {
                    let canonical = path
                        .canonicalize()
                        .or_else(|_| {
                            path.parent()
                                .ok_or_else(|| std::io::Error::other("missing parent"))?
                                .canonicalize()
                                .map(|p| p.join(path.file_name().unwrap_or_default()))
                        })
                        .map_err(|_| "诊断文件目录不可用。")?;
                    if !canonical.starts_with(root) {
                        return Err("诊断文件不属于来源项目。".into());
                    }
                }
                let uri = url::Url::parse(&document.uri).map_err(|_| "诊断文档 URI 无效。")?;
                if uri.scheme() != "file" || uri.to_file_path().ok().as_ref() != Some(path) {
                    return Err("诊断 URI 与本地路径不一致。".into());
                }
            }
            for diagnostic in &document.diagnostics {
                if !matches!(
                    diagnostic.severity.as_str(),
                    "error" | "warning" | "information" | "hint"
                ) || diagnostic.message.len() > 16000
                    || diagnostic.source.chars().count() > 256
                    || diagnostic.code.chars().count() > 256
                    || diagnostic.start_line == 0
                    || diagnostic.start_column == 0
                    || diagnostic.end_line < diagnostic.start_line
                    || diagnostic.end_column == 0
                    || (diagnostic.end_line == diagnostic.start_line
                        && diagnostic.end_column < diagnostic.start_column)
                {
                    return Err("诊断内容、级别或位置无效。".into());
                }
            }
        }
        Ok(())
    }
    pub fn capture(&mut self, root: &Path) -> Result<(), String> {
        self.validate(Some(root))?;
        for document in &mut self.documents {
            document.disk_stamp = document.path.as_deref().and_then(disk_stamp);
        }
        Ok(())
    }
    pub fn count(&self) -> usize {
        self.documents.iter().map(|d| d.diagnostics.len()).sum()
    }
    pub fn redact(&mut self, filter: impl Fn(&str) -> String) {
        for document in &mut self.documents {
            document.uri = filter(&document.uri);
            if let Some(path) = &mut document.path {
                *path = PathBuf::from(filter(&path.to_string_lossy()));
            }
            for diagnostic in &mut document.diagnostics {
                diagnostic.message = filter(&diagnostic.message);
                diagnostic.source = filter(&diagnostic.source);
                diagnostic.code = filter(&diagnostic.code);
            }
        }
    }
    pub fn context(&self) -> String {
        let mut lines = vec![format!(
            "[编辑器诊断快照，时间 {}；以当前代码为准，旧快照不代表当前状态]",
            self.captured_at
        )];
        let mut remaining = 12000;
        for document in &self.documents {
            if document.stale() {
                continue;
            }
            for diagnostic in &document.diagnostics {
                let line = format!(
                    "{}:{}:{} [{} / {}] {}{}",
                    document
                        .path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| document.uri.clone()),
                    diagnostic.start_line,
                    diagnostic.start_column,
                    diagnostic.severity,
                    diagnostic.source,
                    diagnostic.message,
                    if document.dirty {
                        "（未保存的编辑器版本）"
                    } else {
                        ""
                    }
                );
                if line.chars().count() > remaining {
                    lines.push("其余诊断未载入，可在诊断页查看。".into());
                    return lines.join("\n");
                }
                remaining -= line.chars().count();
                lines.push(line);
            }
        }
        lines.join("\n")
    }
}
