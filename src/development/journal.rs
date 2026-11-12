//! Bounded, versioned observations of development turns. Replay never executes tools.
use super::{ExecutionItem, Turn, context::Manifest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const LIMIT: usize = 2 * 1024 * 1024;
const EVENT_LIMIT: usize = 4096;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Data {
    Started {
        user: String,
        started_at: u64,
    },
    Message {
        id: String,
        text: String,
    },
    Execution(Box<ExecutionItem>),
    Diff(String),
    Review(Vec<super::review::FileReview>),
    Plan(Value),
    Context(Box<Manifest>),
    Supplement(String),
    SupplementAt {
        text: String,
        boundary: super::OutputBoundary,
    },
    Finished {
        status: String,
        error: String,
    },
    Checkpoint(Box<Turn>),
}
impl Data {
    pub fn label(&self) -> &str {
        match self {
            Self::Started { .. } => "轮次开始",
            Self::Message { .. } => "回复快照",
            Self::Execution(_) => "执行记录快照",
            Self::Diff(_) => "文件差异",
            Self::Review(_) => "文件全文快照",
            Self::Plan(_) => "计划更新",
            Self::Context(_) => "发送清单",
            Self::Supplement(_) | Self::SupplementAt { .. } => "已确认的运行中补充",
            Self::Finished { .. } => "轮次结束",
            Self::Checkpoint(_) => "合并检查点",
        }
    }
    pub(super) fn sanitize(&mut self, filter: &dyn Fn(&str) -> String) {
        match self {
            Self::Started { user, .. } => *user = filter(user),
            Self::Message { id, text } => {
                *id = filter(id);
                *text = filter(text);
            }
            Self::Execution(item) => item.sanitize(filter),
            Self::Diff(text) | Self::Supplement(text) => *text = filter(text),
            Self::SupplementAt { text, boundary } => {
                *text = filter(text);
                boundary.sanitize(filter);
            }
            Self::Review(files) => files.iter_mut().for_each(|file| file.sanitize(filter)),
            Self::Plan(plan) => sanitize_value(plan, filter),
            Self::Context(manifest) => manifest.sanitize(filter),
            Self::Finished { status, error } => {
                *status = filter(status);
                *error = filter(error);
            }
            Self::Checkpoint(turn) => turn.sanitize_with(filter),
        }
    }
}

pub(super) fn sanitize_value(value: &mut Value, filter: &dyn Fn(&str) -> String) {
    match value {
        Value::String(text) => *text = filter(text),
        Value::Array(values) => values.iter_mut().for_each(|v| sanitize_value(v, filter)),
        Value::Object(values) => values.values_mut().for_each(|v| sanitize_value(v, filter)),
        _ => (),
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub seq: u64,
    pub time_ms: u64,
    pub data: Data,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Journal {
    pub version: u32,
    pub compacted: bool,
    pub truncated: bool,
    pub events: Vec<Record>,
    #[serde(skip)]
    bytes: usize,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            version: 1,
            compacted: false,
            truncated: false,
            events: Vec::new(),
            bytes: 0,
        }
    }
}
impl Journal {
    pub fn replay(&self) -> Result<Turn, String> {
        if self.version != 1 || self.truncated || self.events.is_empty() {
            return Err("事件记录版本不支持或记录不完整，不能离线回放。".into());
        }
        let mut turn = Turn::default();
        let mut previous = None;
        for (index, record) in self.events.iter().enumerate() {
            if record.seq == u64::MAX
                || previous.is_some_and(|seq: u64| seq.checked_add(1) != Some(record.seq))
                || (index == 0
                    && !matches!(record.data, Data::Started { .. } | Data::Checkpoint(_)))
                || (index == 0 && matches!(record.data, Data::Started { .. }) && record.seq != 0)
            {
                return Err("事件序号不连续或缺少起点，不能离线回放。".into());
            }
            previous = Some(record.seq);
            match &record.data {
                Data::Started { user, started_at } => {
                    if index != 0 {
                        return Err("事件记录含重复轮次起点。".into());
                    }
                    turn.user.clone_from(user);
                    turn.started_at = *started_at;
                    turn.status = "running".into();
                }
                Data::Checkpoint(snapshot) => {
                    if index != 0 || snapshot.journal.is_some() {
                        return Err("事件检查点无效。".into());
                    }
                    turn = *snapshot.clone();
                }
                Data::Message { id, text } => turn.text(id.clone(), text.clone(), true),
                Data::Execution(item) => {
                    if let Some(current) = turn.items.iter_mut().find(|i| i.id == item.id) {
                        *current = *item.clone();
                    } else {
                        turn.items.push(*item.clone());
                    }
                }
                Data::Diff(diff) => turn.diff.clone_from(diff),
                Data::Review(files) => turn.review = Some(files.clone()),
                Data::Plan(plan) => turn.plan.clone_from(plan),
                Data::Context(manifest) => {
                    if manifest.version != 1 || manifest.preview {
                        return Err("事件中的发送清单版本或状态无效。".into());
                    }
                    turn.manifest = Some(*manifest.clone());
                }
                Data::Supplement(text) => turn.supplements.push(text.clone()),
                Data::SupplementAt { text, boundary } => {
                    turn.output_boundaries.push(boundary.clone());
                    turn.supplements.push(text.clone());
                }
                Data::Finished { status, error } => turn.finish(status, error),
            }
        }
        turn.pending.clear();
        Ok(turn)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Pending {
    Message(String),
    Execution(String),
    Diff,
    Plan,
}

impl Turn {
    pub(super) fn record(&mut self, mut data: Data) {
        let Some(mut journal) = self.journal.take() else {
            return;
        };
        if journal.truncated {
            self.journal = Some(journal);
            return;
        }
        let Some(seq) = journal
            .events
            .last()
            .map_or(Some(0), |record| record.seq.checked_add(1))
        else {
            journal.truncated = true;
            self.journal = Some(journal);
            return;
        };
        let mut size = serde_json::to_vec(&data).map_or(LIMIT + 1, |v| v.len() + 64);
        if journal.bytes + size > LIMIT || journal.events.len() >= EVENT_LIMIT {
            // Preserve a complete current projection instead of dropping a log prefix.
            data = Data::Checkpoint(Box::new(self.clone()));
            size = serde_json::to_vec(&data).map_or(LIMIT + 1, |v| v.len() + 64);
            if size > LIMIT {
                journal.truncated = true;
                self.journal = Some(journal);
                return;
            }
            journal.events.clear();
            journal.bytes = 0;
            journal.compacted = true;
        }
        journal.bytes += size;
        journal.events.push(Record {
            seq,
            time_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            data,
        });
        self.journal = Some(journal);
    }

    pub(super) fn changed(&mut self, pending: Pending) {
        if self.journal.is_some() && !self.pending.contains(&pending) {
            self.pending.push(pending);
        }
    }

    /// Call after sanitizing the assembled stream, never on individual raw deltas.
    pub fn flush_events(&mut self) {
        for pending in std::mem::take(&mut self.pending) {
            let data = match pending {
                Pending::Message(id) => {
                    let Some((_, text)) = self.messages.iter().find(|(key, _)| key == &id) else {
                        continue;
                    };
                    Data::Message {
                        id,
                        text: text.clone(),
                    }
                }
                Pending::Execution(id) => {
                    let Some(item) = self.items.iter().find(|item| item.id == id) else {
                        continue;
                    };
                    Data::Execution(Box::new(item.clone()))
                }
                Pending::Diff => Data::Diff(self.diff.clone()),
                Pending::Plan => Data::Plan(self.plan.clone()),
            };
            self.record(data);
        }
    }

    pub fn set_manifest(&mut self, manifest: Manifest) {
        self.manifest = Some(manifest.clone());
        self.record(Data::Context(Box::new(manifest)));
    }

    pub fn supplement(&mut self, text: String, boundary: super::OutputBoundary) {
        self.flush_events();
        self.output_boundaries.push(boundary.clone());
        self.supplements.push(text.clone());
        self.record(Data::SupplementAt { text, boundary });
    }

    pub fn finish(&mut self, status: &str, error: &str) {
        self.flush_events();
        self.status = status.into();
        self.error = error.into();
        for item in &mut self.items {
            if item.status == "inProgress" {
                item.status = "interrupted".into();
            }
        }
        self.record(Data::Finished {
            status: status.into(),
            error: error.into(),
        });
    }

    pub fn verify_replay(&self) -> Result<(), String> {
        let replay = self
            .journal
            .as_ref()
            .ok_or("旧记录没有事件日志。")?
            .replay()?;
        let mut current = self.clone();
        current.journal = None;
        if serde_json::to_value(current).ok() != serde_json::to_value(replay).ok() {
            return Err("回放结果与保存的任务快照不一致。".into());
        }
        Ok(())
    }

    pub(super) fn restore_journal(&mut self) -> Result<(), String> {
        if let Some(journal) = &mut self.journal {
            if journal.version != 1 {
                return Err("事件记录版本不支持，未覆盖原文件。".into());
            }
            journal.bytes = serde_json::to_vec(&journal.events)
                .map_err(|_| "事件记录无效。")?
                .len();
            if !journal.truncated {
                self.verify_replay()?;
            }
        }
        if self
            .manifest
            .as_ref()
            .is_some_and(|m| m.version != 1 || m.preview)
        {
            return Err("发送清单版本不支持，未覆盖原文件。".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::development::{
        Activity, Workspaces,
        context::{Block, Channel},
    };
    use serde_json::json;

    #[test]
    fn replay_preserves_reconciled_execution_final_messages_context_and_late_supplements() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("history.json");
        let mut book = Workspaces::default();
        book.active = Some(book.add(root.path()).unwrap());
        book.begin("original input".into(), "a", true);
        let turn = book.turn_mut().unwrap();
        turn.set_manifest(Manifest {
            blocks: vec![Block {
                id: "input".into(),
                content: "original input".into(),
                channel: Channel::Input,
                included: true,
                ..Default::default()
            }],
            ..Default::default()
        });
        // Output can precede the started notification. Store only sanitized,
        // assembled snapshots, including a key split across processing batches.
        turn.activity(Activity::Output {
            id: "cmd".into(),
            delta: "fixture-".into(),
        });
        turn.text("reply".into(), "fixture-".into(), false);
        turn.flush_events();
        turn.activity(Activity::Output {
            id: "cmd".into(),
            delta: "secret".into(),
        });
        turn.text("reply".into(), "secret".into(), false);
        turn.sanitize(|s| s.replace("fixture-secret", "[hidden]"));
        turn.flush_events();
        turn.activity(Activity::Item(json!({"id":"cmd","type":"commandExecution","command":"cargo test","status":"completed","aggregatedOutput":"failed fixture-secret","exitCode":1})));
        turn.activity(Activity::Patch {
            id: "patch".into(),
            changes: json!([{"path":"a.rs","diff":"+new"}]),
        });
        turn.activity(Activity::Plan(
            json!({"plan":[{"step":"fixture-secret","status":"completed"}]}),
        ));
        turn.activity(Activity::Diff("+new".into()));
        turn.text("reply".into(), "final reply".into(), true);
        turn.sanitize(|s| s.replace("fixture-secret", "[hidden]"));
        turn.finish("completed", "");
        turn.supplement("late confirmed input".into(), turn.output_boundary());
        turn.verify_replay().unwrap();
        assert_eq!(turn.items[0].exit_code, Some(1));
        assert_eq!(turn.status, "completed");
        assert!(
            !serde_json::to_string(&turn)
                .unwrap()
                .contains("fixture-secret")
        );
        let journal = turn.journal.as_ref().unwrap();
        assert!(
            journal
                .events
                .windows(2)
                .all(|pair| pair[0].seq + 1 == pair[1].seq)
        );
        assert!(
            journal
                .events
                .iter()
                .any(|record| matches!(record.data, Data::Context(_)))
        );
        book.save(&file).unwrap();
        let loaded = Workspaces::read(&file).unwrap();
        loaded.task().unwrap().turns[0].verify_replay().unwrap();

        // A missing event or future schema must never be silently replayed or overwritten.
        let mut document = serde_json::to_value(&book).unwrap();
        document["projects"][0]["tasks"][0]["turns"][0]["journal"]["version"] = json!(99);
        let damaged = serde_json::to_vec(&document).unwrap();
        std::fs::write(&file, &damaged).unwrap();
        assert!(Workspaces::read(&file).is_err());
        assert!(book.save(&file).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), damaged);
        let turn = book.turn_mut().unwrap();
        let original_seq = turn.journal.as_ref().unwrap().events[0].seq;
        turn.journal.as_mut().unwrap().events[0].seq = u64::MAX;
        assert!(turn.verify_replay().is_err());
        turn.journal.as_mut().unwrap().events[0].seq = original_seq;
        turn.journal.as_mut().unwrap().events.remove(1);
        assert!(turn.verify_replay().is_err());
    }

    #[test]
    fn bounded_journals_compact_to_replayable_checkpoints_and_report_oversized_snapshots() {
        let mut turn = Turn {
            user: "start".into(),
            status: "running".into(),
            journal: Some(Journal::default()),
            ..Default::default()
        };
        turn.record(Data::Started {
            user: "start".into(),
            started_at: 0,
        });
        turn.text("reply".into(), "before supplement".into(), true);
        turn.supplement(
            "keep position after compaction".into(),
            turn.output_boundary(),
        );
        for index in 0..150 {
            turn.text(
                "reply".into(),
                format!("{index}{}", "中".repeat(8000)),
                true,
            );
            turn.flush_events();
        }
        turn.finish("interrupted", "cancelled");
        turn.verify_replay().unwrap();
        let journal = turn.journal.as_ref().unwrap();
        assert!(journal.compacted);
        assert!(matches!(journal.events[0].data, Data::Checkpoint(_)));
        assert_eq!(turn.output_boundaries.len(), 1);
        assert_eq!(
            turn.output_messages(1).collect::<String>(),
            turn.messages[0].1
        );
        assert!(journal.events[0].seq > 0);
        assert!(serde_json::to_vec(journal).unwrap().len() < LIMIT);
        turn.text("large".into(), "x".repeat(LIMIT + 1), true);
        turn.flush_events();
        assert!(turn.journal.as_ref().unwrap().truncated);
        assert!(turn.verify_replay().is_err());
    }
}
