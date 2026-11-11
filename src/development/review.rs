//! Turn-scoped file evidence and bounded, immutable full-file review snapshots.
use super::{Turn, journal::Data};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::Path};

const FILE_LIMIT: u64 = 1024 * 1024;
const TOTAL_LIMIT: usize = 4 * 1024 * 1024;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FileReview {
    pub path: String,
    pub kind: String,
    pub patches: Vec<String>,
    pub content: Option<String>,
    pub notice: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

pub struct Line {
    pub old: Option<usize>,
    pub new: Option<usize>,
    pub kind: LineKind,
    pub text: String,
}

struct Hunk<'a> {
    old: usize,
    new: usize,
    lines: Vec<(LineKind, &'a str)>,
}

fn hunks(patch: &str) -> Option<Vec<Hunk<'_>>> {
    let mut result: Vec<Hunk<'_>> = Vec::new();
    let mut remaining = (0_usize, 0_usize);
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("@@ -") {
            if remaining != (0, 0) {
                return None;
            }
            let (old, rest) = header.split_once(" +")?;
            let (new, _) = rest.split_once(" @@")?;
            let range = |text: &str| -> Option<(usize, usize)> {
                let (start, count) = text.split_once(',').unwrap_or((text, "1"));
                Some((start.parse().ok()?, count.parse().ok()?))
            };
            let (old, old_count) = range(old)?;
            let (new, new_count) = range(new)?;
            remaining = (old_count, new_count);
            result.push(Hunk {
                old: if old_count == 0 {
                    old
                } else {
                    old.checked_sub(1)?
                },
                new: if new_count == 0 {
                    new
                } else {
                    new.checked_sub(1)?
                },
                lines: Vec::new(),
            });
        } else if remaining != (0, 0) {
            let (kind, old, new) = match line.as_bytes().first()? {
                b'+' => (LineKind::Added, 0, 1),
                b'-' => (LineKind::Removed, 1, 0),
                b' ' => (LineKind::Context, 1, 1),
                b'\\' => continue,
                _ => return None,
            };
            remaining.0 = remaining.0.checked_sub(old)?;
            remaining.1 = remaining.1.checked_sub(new)?;
            result.last_mut()?.lines.push((kind, &line[1..]));
        }
    }
    (remaining == (0, 0) && !result.is_empty()).then_some(result)
}

// Git quotes paths as C strings, including octal UTF-8 bytes.
fn path_text(text: &str, prefixed: bool) -> String {
    let text = text.split('\t').next().unwrap_or(text);
    let decoded = if text.starts_with('"') && text.ends_with('"') {
        let mut bytes = text.as_bytes()[1..text.len() - 1]
            .iter()
            .copied()
            .peekable();
        let mut output = Vec::new();
        while let Some(byte) = bytes.next() {
            if byte != b'\\' {
                output.push(byte);
                continue;
            }
            let Some(next) = bytes.next() else { break };
            if (b'0'..=b'7').contains(&next) {
                let mut octal = u16::from(next - b'0');
                for _ in 0..2 {
                    if let Some(&digit) = bytes.peek()
                        && (b'0'..=b'7').contains(&digit)
                    {
                        octal = octal * 8 + u16::from(bytes.next().unwrap() - b'0');
                    }
                }
                output.push(octal as u8);
            } else {
                output.push(match next {
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    other => other,
                });
            }
        }
        String::from_utf8_lossy(&output).into_owned()
    } else {
        text.to_owned()
    };
    if prefixed {
        decoded
            .strip_prefix("a/")
            .or_else(|| decoded.strip_prefix("b/"))
            .unwrap_or(&decoded)
            .to_owned()
    } else {
        decoded
    }
}

/// Prefer the cumulative turn diff: counting both it and individual tool patches doubles totals.
pub fn recorded_files(turn: &Turn) -> Vec<FileReview> {
    let mut files: Vec<FileReview> = Vec::new();
    let mut section = String::new();
    let mut old = String::new();
    let mut path = String::new();
    let mut kind = "update";
    let mut in_hunk = false;
    let flush = |files: &mut Vec<FileReview>, section: &mut String, path: &str, kind: &str| {
        if !path.is_empty() && path != "/dev/null" {
            files.push(FileReview {
                path: path.into(),
                kind: kind.into(),
                patches: vec![std::mem::take(section)],
                notice: "旧任务未保存完整文件，仅展示已记录的差异。".into(),
                ..Default::default()
            });
        }
        section.clear();
    };
    for line in turn.diff.lines() {
        if line.starts_with("diff --git ") {
            flush(&mut files, &mut section, &path, kind);
            path.clear();
            old.clear();
            kind = "update";
            in_hunk = false;
            // Headers below give unambiguous paths even when names contain spaces.
            if let Some((_, name)) = line.rsplit_once(" b/") {
                path = name.into();
            } else if let Some((_, name)) = line.rsplit_once("\" \"") {
                path = path_text(&format!("\"{name}"), true);
            }
        } else if !in_hunk {
            if let Some(name) = line.strip_prefix("--- ") {
                if !path.is_empty() && !old.is_empty() {
                    flush(&mut files, &mut section, &path, kind);
                    kind = "update";
                }
                old = path_text(name, true);
                if old == "/dev/null" {
                    kind = "add";
                }
            } else if let Some(name) = line.strip_prefix("+++ ") {
                path = path_text(name, true);
                if path == "/dev/null" {
                    path.clone_from(&old);
                    kind = "delete";
                }
            } else if let Some(name) = line.strip_prefix("rename to ") {
                path = path_text(name, false);
                kind = "rename";
            } else if line.starts_with("new file mode ") {
                kind = "add";
            } else if line.starts_with("deleted file mode ") {
                kind = "delete";
            }
        }
        if line.starts_with("@@ ") {
            in_hunk = true;
        }
        section.push_str(line);
        section.push('\n');
    }
    flush(&mut files, &mut section, &path, kind);
    let cumulative_count = files.len();
    for change in turn.items.iter().flat_map(|item| &item.changes) {
        if change.path.is_empty() {
            continue;
        }
        // Absolute protocol paths and relative diff paths must identify the same file.
        let existing = files.iter().position(|file| {
            file.path == change.path
                || Path::new(&change.path).is_absolute()
                    && Path::new(&change.path).ends_with(&file.path)
        });
        if let Some(index) = existing {
            // Cumulative evidence wins. Fallback patches are ordered tool operations.
            if index >= cumulative_count {
                let file = &mut files[index];
                file.patches.push(change.diff.clone());
                file.kind.clone_from(&change.kind);
            }
        } else {
            files.push(FileReview {
                path: change.path.clone(),
                kind: change.kind.clone(),
                patches: vec![change.diff.clone()],
                notice: "未保存完整文件，仅展示工具记录的差异。".into(),
                ..Default::default()
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

impl FileReview {
    pub(super) fn sanitize(&mut self, filter: &dyn Fn(&str) -> String) {
        self.path = filter(&self.path);
        self.kind = filter(&self.kind);
        self.notice = filter(&self.notice);
        self.content = self.content.as_ref().map(|text| filter(text));
        self.patches
            .iter_mut()
            .for_each(|text| *text = filter(text));
    }

    /// Reverse validated patches while retaining final line identities. This combines
    /// repeated edits without marking intermediate text as a surviving change.
    pub fn lines(&self) -> Option<Vec<Line>> {
        let content = self.content.as_ref()?;
        let final_lines: Vec<_> = content.lines().collect();
        let mut original: Vec<_> = final_lines
            .iter()
            .enumerate()
            .map(|(index, text)| (Some(index + 1), *text))
            .collect();
        for patch in self.patches.iter().rev() {
            if patch.is_empty() {
                continue;
            }
            let Some(hunks) = hunks(patch) else {
                if !patch.lines().any(|line| {
                    line.starts_with("@@")
                        || line.starts_with('+') && !line.starts_with("+++")
                        || line.starts_with('-') && !line.starts_with("---")
                }) {
                    continue; // Rename, empty-file creation, or mode-only evidence.
                }
                return None;
            };
            for hunk in hunks.iter().rev() {
                let expected: Vec<_> = hunk
                    .lines
                    .iter()
                    .filter(|(kind, _)| *kind != LineKind::Removed)
                    .collect();
                let end = hunk.new.checked_add(expected.len())?;
                let actual = original.get(hunk.new..end)?;
                if !actual
                    .iter()
                    .zip(&expected)
                    .all(|((_, text), (_, expected))| text == expected)
                {
                    return None;
                }
                let mut current = actual.iter();
                let restored: Vec<_> = hunk
                    .lines
                    .iter()
                    .filter_map(|(kind, text)| match kind {
                        LineKind::Added => {
                            current.next();
                            None
                        }
                        LineKind::Removed => Some((None, *text)),
                        LineKind::Context => current.next().copied(),
                    })
                    .collect();
                original.splice(hunk.new..end, restored);
            }
            // Also validate old positions: malformed ranges must not color unrelated lines.
            for hunk in &hunks {
                let expected: Vec<_> = hunk
                    .lines
                    .iter()
                    .filter(|(kind, _)| *kind != LineKind::Added)
                    .collect();
                let actual = original.get(hunk.old..hunk.old.checked_add(expected.len())?)?;
                if !actual
                    .iter()
                    .zip(expected)
                    .all(|((_, text), (_, expected))| text == expected)
                {
                    return None;
                }
            }
        }
        let mut rows = Vec::new();
        let mut old_start = 0;
        let mut new_start = 0;
        // Unchanged line identities anchor gaps; trim equal edges inside each
        // gap so edits later reverted do not appear as surviving changes.
        for (old_end, new_end) in original
            .iter()
            .enumerate()
            .filter_map(|(old, (new, _))| new.map(|new| (old, new - 1)))
            .chain(std::iter::once((original.len(), final_lines.len())))
        {
            while old_start < old_end
                && new_start < new_end
                && original[old_start].1 == final_lines[new_start]
            {
                rows.push(Line {
                    old: Some(old_start + 1),
                    new: Some(new_start + 1),
                    kind: LineKind::Context,
                    text: final_lines[new_start].into(),
                });
                old_start += 1;
                new_start += 1;
            }
            let mut suffix = 0;
            while suffix < old_end - old_start
                && suffix < new_end - new_start
                && original[old_end - suffix - 1].1 == final_lines[new_end - suffix - 1]
            {
                suffix += 1;
            }
            for (offset, (_, text)) in original[old_start..old_end - suffix].iter().enumerate() {
                rows.push(Line {
                    old: Some(old_start + offset + 1),
                    new: None,
                    kind: LineKind::Removed,
                    text: (*text).into(),
                });
            }
            for (offset, text) in final_lines[new_start..new_end - suffix].iter().enumerate() {
                rows.push(Line {
                    old: None,
                    new: Some(new_start + offset + 1),
                    kind: LineKind::Added,
                    text: (*text).into(),
                });
            }
            for offset in (0..suffix).rev() {
                rows.push(Line {
                    old: Some(old_end - offset),
                    new: Some(new_end - offset),
                    kind: LineKind::Context,
                    text: final_lines[new_end - offset - 1].into(),
                });
            }
            if old_end < original.len() {
                rows.push(Line {
                    old: Some(old_end + 1),
                    new: Some(new_end + 1),
                    kind: LineKind::Context,
                    text: final_lines[new_end].into(),
                });
            }
            old_start = old_end + 1;
            new_start = new_end + 1;
        }
        Some(rows)
    }

    pub fn counts(&self) -> (usize, usize) {
        if let Some(lines) = self.lines() {
            return (
                lines
                    .iter()
                    .filter(|line| line.kind == LineKind::Added)
                    .count(),
                lines
                    .iter()
                    .filter(|line| line.kind == LineKind::Removed)
                    .count(),
            );
        }
        let mut counts = (0, 0);
        for patch in &self.patches {
            if let Some(hunks) = hunks(patch) {
                for (kind, _) in hunks.iter().flat_map(|hunk| &hunk.lines) {
                    match kind {
                        LineKind::Added => counts.0 += 1,
                        LineKind::Removed => counts.1 += 1,
                        _ => (),
                    }
                }
            } else {
                // Older protocol fixtures may omit unified headers.
                for line in patch.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        counts.0 += 1;
                    }
                    if line.starts_with('-') && !line.starts_with("---") {
                        counts.1 += 1;
                    }
                }
            }
        }
        counts
    }
}

impl Turn {
    pub fn capture_review(&mut self, project: &Path, filter: impl Fn(&str) -> String) {
        let mut files = recorded_files(self);
        let mut budget = TOTAL_LIMIT;
        for file in &mut files {
            let result = (|| -> Result<String, &'static str> {
                if file.patches.iter().any(|patch| {
                    patch
                        .lines()
                        .any(|line| line.starts_with("Binary files ") || line == "GIT binary patch")
                }) {
                    return Err("二进制文件，仅保留变更记录");
                }
                if file.kind == "delete" {
                    return Ok(String::new());
                }
                let root = project.canonicalize().map_err(|_| "项目目录不可用")?;
                let path = project
                    .join(&file.path)
                    .canonicalize()
                    .map_err(|_| "文件不存在或无法访问")?;
                if !path.starts_with(&root) {
                    return Err("文件位于项目外，未读取全文");
                }
                if !path.metadata().is_ok_and(|meta| meta.is_file()) {
                    return Err("不是普通文件");
                }
                let input = File::open(path).map_err(|_| "无法读取文件")?;
                if !input.metadata().is_ok_and(|meta| meta.is_file()) {
                    return Err("不是普通文件");
                }
                let mut bytes = Vec::new();
                input
                    .take(FILE_LIMIT.min(budget as u64) + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "无法读取文件")?;
                if bytes.len() as u64 > FILE_LIMIT || bytes.len() > budget {
                    return Err("文件超过快照容量，仅保留差异");
                }
                budget -= bytes.len();
                if bytes.contains(&0) {
                    return Err("二进制文件，仅保留变更记录");
                }
                String::from_utf8(bytes).map_err(|_| "非 UTF-8 文件，仅保留差异")
            })();
            match result {
                Ok(content) => {
                    file.content = Some(content);
                    file.notice.clear();
                    if file.lines().is_none() {
                        file.notice =
                            "完整文件已保存，但差异与结束时的文件不匹配；分别显示全文和记录差异。"
                                .into();
                    }
                }
                Err(error) => file.notice = error.into(),
            }
            file.sanitize(&filter);
        }
        self.review = Some(files.clone());
        self.record(Data::Review(files));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::development::{Activity, ExecutionItem, FileChange, Workspaces};

    #[test]
    fn empty_files_renames_modes_and_quoted_binary_paths_preserve_file_records() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("a")).unwrap();
        std::fs::write(root.path().join("a/renamed"), "unchanged\n").unwrap();
        std::fs::write(root.path().join("empty"), "").unwrap();
        std::fs::write(root.path().join("mode"), "unchanged\n").unwrap();
        let mut turn = Turn { diff: concat!(
            "diff --git a/old b/a/renamed\nsimilarity index 100%\nrename from old\nrename to a/renamed\n",
            "diff --git a/empty b/empty\nnew file mode 100644\n",
            "diff --git a/mode b/mode\nold mode 100644\nnew mode 100755\n",
            "diff --git \"a/\\344\\270\\255\" \"b/\\344\\270\\255\"\nBinary files a/old and b/new differ\n",
        ).into(), ..Default::default() };
        turn.capture_review(root.path(), str::to_owned);
        let files = turn.review.as_ref().unwrap();
        assert_eq!(files.len(), 4);
        let renamed = files.iter().find(|file| file.kind == "rename").unwrap();
        assert_eq!(renamed.path, "a/renamed");
        assert_eq!(renamed.lines().unwrap()[0].kind, LineKind::Context);
        let empty = files.iter().find(|file| file.path == "empty").unwrap();
        assert!(empty.lines().unwrap().is_empty());
        let mode = files.iter().find(|file| file.path == "mode").unwrap();
        assert_eq!(mode.counts(), (0, 0));
        let binary = files.iter().find(|file| file.path == "中").unwrap();
        assert!(binary.content.is_none());
        assert!(binary.notice.contains("二进制"));
    }

    #[test]
    fn cumulative_review_counts_headers_once_and_preserves_full_context_and_line_numbers() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.rs"), "before\nnew\nafter\nuntouched\n").unwrap();
        let patch = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,3 +1,3 @@\n before\n-old\n+new\n after\n";
        let mut turn = Turn {
            diff: patch.into(),
            items: vec![ExecutionItem {
                changes: vec![FileChange {
                    path: root.path().join("a.rs").to_string_lossy().into_owned(),
                    diff: "@@ -2 +2 @@\n-old\n+new\n".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        turn.capture_review(root.path(), str::to_owned);
        let files = turn.review.as_ref().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].counts(), (1, 1));
        let rows = files[0].lines().unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(
            (rows[1].old, rows[1].new, rows[1].kind),
            (Some(2), None, LineKind::Removed)
        );
        assert_eq!(
            (rows[2].old, rows[2].new, rows[2].kind),
            (None, Some(2), LineKind::Added)
        );
        assert_eq!(rows[4].text, "untouched");
        std::fs::write(root.path().join("a.rs"), "later edit").unwrap();
        assert_eq!(files[0].lines().unwrap()[4].text, "untouched");
    }

    #[test]
    fn repeated_tool_edits_and_reverts_exclude_intermediate_text() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a"), "first\nfinal\ntail\n").unwrap();
        let mut turn = Turn::default();
        for (index, patch) in [
            "@@ -2 +2 @@\n-old\n+middle\n",
            "@@ -2 +2 @@\n-middle\n+final\n",
        ]
        .iter()
        .enumerate()
        {
            turn.activity(Activity::Patch {
                id: index.to_string(),
                changes: serde_json::json!([{"path":"a","diff":patch}]),
            });
        }
        turn.capture_review(root.path(), str::to_owned);
        let file = &turn.review.as_ref().unwrap()[0];
        assert_eq!(file.counts(), (1, 1));
        assert!(
            !file
                .lines()
                .unwrap()
                .iter()
                .any(|line| line.text == "middle")
        );
        turn.activity(Activity::Patch {
            id: "revert".into(),
            changes: serde_json::json!([{"path":"a","diff":"@@ -2 +2 @@\n-final\n+old\n"}]),
        });
        std::fs::write(root.path().join("a"), "first\nold\ntail\n").unwrap();
        turn.capture_review(root.path(), str::to_owned);
        assert_eq!(turn.review.as_ref().unwrap()[0].counts(), (0, 0));
    }

    #[test]
    fn additions_deletions_multiple_hunks_and_quoted_paths_are_reviewable() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("中 文"), "中文\n").unwrap();
        std::fs::write(root.path().join("edit"), "a\ninsert\nb\nc\ne\n").unwrap();
        let mut turn = Turn { diff: concat!(
            "diff --git a/new b/new\nnew file mode 100644\n--- /dev/null\n+++ \"b/\\344\\270\\255 \\346\\226\\207\"\n@@ -0,0 +1 @@\n+中文\n",
            "diff --git a/deleted b/deleted\ndeleted file mode 100644\n--- a/deleted\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-gone\n-last\n\\ No newline at end of file\n",
            "diff --git a/edit b/edit\n--- a/edit\n+++ b/edit\n@@ -1,0 +2 @@\n+insert\n@@ -4 +4,0 @@\n-d\n",
        ).into(), ..Default::default() };
        turn.capture_review(root.path(), str::to_owned);
        let files = turn.review.as_ref().unwrap();
        assert_eq!(files.len(), 3);
        let added = files.iter().find(|f| f.kind == "add").unwrap();
        assert_eq!(added.path, "中 文");
        assert_eq!(added.counts(), (1, 0));
        let deleted = files.iter().find(|f| f.kind == "delete").unwrap();
        assert_eq!(deleted.counts(), (0, 2));
        assert!(
            deleted
                .lines()
                .unwrap()
                .iter()
                .all(|line| line.new.is_none())
        );
        let edited = files.iter().find(|f| f.path == "edit").unwrap();
        assert_eq!(edited.counts(), (1, 1));
        assert!(edited.notice.is_empty(), "{}", edited.notice);
    }

    #[test]
    fn unavailable_binary_large_external_and_mismatched_files_keep_honest_diff_fallbacks() {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::write(external.path().join("private"), "must not read").unwrap();
        std::fs::write(root.path().join("binary"), [0_u8, 1, 2]).unwrap();
        std::fs::write(
            root.path().join("large"),
            vec![b'x'; FILE_LIMIT as usize + 1],
        )
        .unwrap();
        std::fs::write(root.path().join("mismatch"), "newer\n").unwrap();
        let mut turn = Turn::default();
        for path in [
            "missing".to_owned(),
            "binary".into(),
            "large".into(),
            "mismatch".into(),
            external
                .path()
                .join("private")
                .to_string_lossy()
                .into_owned(),
        ] {
            turn.items.push(ExecutionItem {
                changes: vec![FileChange {
                    path,
                    diff: "@@ -1 +1 @@\n-old\n+new\n".into(),
                    ..Default::default()
                }],
                ..Default::default()
            });
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(external.path().join("private"), root.path().join("link"))
                .unwrap();
            turn.items.push(ExecutionItem {
                changes: vec![FileChange {
                    path: "link".into(),
                    ..Default::default()
                }],
                ..Default::default()
            });
        }
        turn.capture_review(root.path(), str::to_owned);
        for file in turn.review.as_ref().unwrap() {
            assert!(!file.notice.is_empty());
            assert!(file.lines().is_none());
            if file.path != "mismatch" {
                assert!(file.content.is_none());
            }
            assert!(
                !file
                    .content
                    .as_deref()
                    .unwrap_or_default()
                    .contains("must not read")
            );
        }
    }

    #[test]
    fn review_snapshots_are_redacted_journaled_and_survive_reload_and_legacy_history() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("history.json");
        std::fs::write(root.path().join("a"), "fixture-secret\n").unwrap();
        let mut book = Workspaces::default();
        book.active = Some(book.add(root.path()).unwrap());
        book.begin("change".into(), "Lexington", true);
        let turn = book.turn_mut().unwrap();
        turn.activity(Activity::Diff(
            "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+fixture-secret\n".into(),
        ));
        turn.sanitize(|text| text.replace("fixture-secret", "[hidden]"));
        turn.flush_events();
        turn.capture_review(root.path(), |text| {
            text.replace("fixture-secret", "[hidden]")
        });
        turn.finish("completed", "");
        turn.verify_replay().unwrap();
        assert!(
            !serde_json::to_string(turn)
                .unwrap()
                .contains("fixture-secret")
        );
        book.save(&store).unwrap();
        let loaded = Workspaces::read(&store).unwrap();
        let loaded = &loaded.task().unwrap().turns[0];
        assert_eq!(loaded.review.as_ref().unwrap()[0].counts(), (1, 1));
        loaded.verify_replay().unwrap();
        let old: Turn = serde_json::from_str(r#"{"status":"completed","diff":""}"#).unwrap();
        assert!(old.review.is_none());
    }

    #[test]
    fn malformed_hunks_do_not_color_unrelated_lines_or_panic() {
        for patch in [
            "@@ -0 +1 @@\n-old\n+new\n",
            "@@ -1,2 +1 @@\n-old\n+new\n",
            "@@ -999 +999 @@\n-old\n+new\n",
            "@@ -1 +1 @@\n-old\n+different\n",
        ] {
            let file = FileReview {
                content: Some("new\n".into()),
                patches: vec![patch.into()],
                ..Default::default()
            };
            assert!(file.lines().is_none(), "{patch}");
        }
    }
}
