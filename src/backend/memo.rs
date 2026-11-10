use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io::Write, path::PathBuf};

pub const MAX_TEXT_CHARS: usize = 2000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Memo {
    pub id: u64,
    pub text: String,
    pub completed: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    items: Vec<Memo>,
}

pub struct MemoState {
    path: PathBuf,
    record: Record,
}

impl MemoState {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let record: Record = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "备忘录记录损坏或格式不兼容，未覆盖原文件。")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Record {
                version: 1,
                items: Vec::new(),
            },
            Err(error) => return Err(format!("读取备忘录失败：{error}")),
        };
        let mut ids = HashSet::new();
        if record.version != 1
            || record.items.iter().any(|item| {
                !ids.insert(item.id)
                    || item.text.trim().is_empty()
                    || item.text.chars().count() > MAX_TEXT_CHARS
            })
        {
            return Err("备忘录记录无效或版本不兼容，未覆盖原文件。".into());
        }
        Ok(Self { path, record })
    }

    pub fn items(&self) -> &[Memo] {
        &self.record.items
    }

    pub fn add(&mut self, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > MAX_TEXT_CHARS {
            return Err(format!("事项需为 1～{MAX_TEXT_CHARS} 个字符。"));
        }
        let mut record = self.record.clone();
        let id = loop {
            let id = rand::random();
            if record.items.iter().all(|item| item.id != id) {
                break id;
            }
        };
        record.items.insert(
            0,
            Memo {
                id,
                text: text.into(),
                completed: false,
            },
        );
        self.store(record)
    }

    pub fn set_completed(&mut self, id: u64, completed: bool) -> Result<(), String> {
        let mut record = self.record.clone();
        let item = record
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or("事项已不存在。")?;
        item.completed = completed;
        self.store(record)
    }

    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        let mut record = self.record.clone();
        record.items.retain(|item| item.id != id);
        self.store(record)
    }

    fn store(&mut self, record: Record) -> Result<(), String> {
        let parent = self.path.parent().ok_or("备忘录路径无效。")?;
        fs::create_dir_all(parent).map_err(|error| format!("保存备忘录失败：{error}"))?;
        // NamedTempFile uses private permissions and cleans up unsuccessful writes.
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| format!("保存备忘录失败：{error}"))?;
        let bytes = serde_json::to_vec(&record).map_err(|_| "无法编码备忘录。")?;
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|error| format!("保存备忘录失败：{error}"))?;
        temporary
            .persist(&self.path)
            .map_err(|error| format!("保存备忘录失败：{}", error.error))?;
        self.record = record;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_lifecycle_persists_completion_restore_and_permanent_deletion() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("memos.json");
        let mut state = MemoState::load(path.clone()).unwrap();
        assert!(!path.exists());
        assert!(state.add(" \n ").is_err());
        assert!(state.add(&"字".repeat(MAX_TEXT_CHARS + 1)).is_err());
        state.add("  第一件事\n第二行  ").unwrap();
        let id = state.items()[0].id;
        state.add("另一件事").unwrap();
        state.set_completed(id, true).unwrap();
        let mut state = MemoState::load(path.clone()).unwrap();
        assert_eq!(state.items().len(), 2);
        assert_eq!(state.items()[1].text, "第一件事\n第二行");
        assert!(state.items()[1].completed);
        state.set_completed(id, false).unwrap();
        assert!(!MemoState::load(path.clone()).unwrap().items()[1].completed);
        state.delete(id).unwrap();
        let state = MemoState::load(path.clone()).unwrap();
        assert_eq!(state.items().len(), 1);
        assert!(!fs::read_to_string(&path).unwrap().contains("第一件事"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn memo_invalid_records_are_not_overwritten_and_failed_writes_keep_memory() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("memos.json");
        for invalid in [
            "broken",
            r#"{"version":2,"items":[]}"#,
            r#"{"version":1,"items":[{"id":1,"text":" ","completed":false}]}"#,
            r#"{"version":1,"items":[{"id":1,"text":"a","completed":false},{"id":1,"text":"b","completed":true}]}"#,
        ] {
            fs::write(&path, invalid).unwrap();
            assert!(MemoState::load(path.clone()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
        let path = directory.path().join("records/memos.json");
        let mut state = MemoState::load(path.clone()).unwrap();
        state.add("保留事项").unwrap();
        let id = state.items()[0].id;
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(state.set_completed(id, true).is_err());
        assert!(!state.items()[0].completed);
        assert!(state.delete(id).is_err());
        assert_eq!(state.items().len(), 1);
        assert!(state.add("不应进入内存").is_err());
        assert_eq!(state.items().len(), 1);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }
}
