use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io::Write, path::PathBuf};

pub const MAX_PROMPT_CHARS: usize = 20_000;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub root: PathBuf,
    pub workspace: Option<PathBuf>,
    pub character: String,
    pub development_mode: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Schedule {
    Once { time: String },
    Daily { time: String },
    Interval { minutes: u32 },
}
impl Schedule {
    fn next_due(&self, now: i64) -> Result<i64, String> {
        match self {
            Self::Once { time } | Self::Daily { time } => super::alarm::next_due(time, now),
            Self::Interval { minutes } => {
                if !(1..=525_600).contains(minutes) {
                    return Err("间隔需为 1～525600 分钟。".into());
                }
                now.checked_add(i64::from(*minutes) * 60)
                    .ok_or_else(|| "下次执行时间超出范围。".into())
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskSpec {
    pub name: String,
    pub prompt: String,
    pub schedule: Schedule,
    pub target: Target,
}
impl TaskSpec {
    fn validate(&self, now: i64) -> Result<i64, String> {
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            return Err("任务名称需为 1～80 个字符。".into());
        }
        if self.prompt.trim().is_empty() || self.prompt.chars().count() > MAX_PROMPT_CHARS {
            return Err(format!("提示词需为 1～{MAX_PROMPT_CHARS} 个字符。"));
        }
        if !self.target.root.is_absolute()
            || self
                .target
                .workspace
                .as_ref()
                .is_some_and(|p| !p.is_absolute())
        {
            return Err("任务目标必须使用本机绝对路径。".into());
        }
        self.schedule.next_due(now)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ready,
    Pending,
    Dispatching,
    Running,
    Completed,
    Failed,
    Interrupted,
    Unknown,
}
impl Status {
    pub fn active(self) -> bool {
        matches!(self, Self::Dispatching | Self::Running)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "尚未执行",
            Self::Pending => "已到期，等待后台执行",
            Self::Dispatching => "提交中，等待发送确认",
            Self::Running => "运行中",
            Self::Completed => "已完成",
            Self::Failed => "执行失败，可重新排队",
            Self::Interrupted => "已中断，可重新排队",
            Self::Unknown => "上次退出时结果未确认，可重新排队",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Task {
    pub id: u64,
    pub spec: TaskSpec,
    pub enabled: bool,
    pub next_due: i64,
    pub status: Status,
    pub last_run: Option<i64>,
}
pub const MAX_HISTORY: usize = 200;
pub const MAX_OUTPUT_CHARS: usize = 64_000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Run {
    pub id: u64,
    pub task_id: u64,
    pub spec: TaskSpec,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: Status,
    pub output: String,
    pub error: String,
    pub unread: bool,
}

pub struct Outcome {
    pub status: Status,
    pub output: String,
    pub error: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    tasks: Vec<Task>,
    history: Vec<Run>,
}
pub struct ScheduleState {
    path: PathBuf,
    record: Record,
}
impl ScheduleState {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let mut record: Record = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "定时任务记录损坏或格式不兼容，未覆盖原文件。")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Record {
                version: 1,
                tasks: Vec::new(),
                history: Vec::new(),
            },
            Err(e) => return Err(format!("读取定时任务失败：{e}")),
        };
        let mut ids = HashSet::new();
        let mut run_ids = HashSet::new();
        if record.version != 1
            || record.tasks.len() > 100
            || record.history.len() > MAX_HISTORY
            || record.history.iter().any(|run| {
                !run_ids.insert(run.id)
                    || run.spec.validate(run.started_at).is_err()
                    || run.output.chars().count() > MAX_OUTPUT_CHARS
                    || run.error.chars().count() > MAX_OUTPUT_CHARS
                    || (run.status.active() != run.finished_at.is_none())
            })
            || record.tasks.iter().any(|t| {
                !ids.insert(t.id)
                    || t.spec.validate(t.next_due).is_err()
                    || super::clock::local_time(t.next_due).is_none()
                    || t.last_run
                        .is_some_and(|time| super::clock::local_time(time).is_none())
            })
        {
            return Err("定时任务记录无效，未覆盖原文件。".into());
        }
        // A process may have died after the server accepted a prompt. Never resend
        // an unconfirmed run automatically; a user can explicitly queue it again.
        let recovered = record.tasks.iter().any(|t| t.status.active());
        for task in &mut record.tasks {
            if task.status.active() {
                task.status = Status::Unknown;
            }
        }
        for run in &mut record.history {
            if run.status.active() {
                run.status = Status::Unknown;
                run.finished_at = Some(super::clock::now());
                run.error = "应用退出时执行结果未确认，未自动重试。".into();
                run.unread = true;
            }
        }
        let mut state = Self { path, record };
        if recovered {
            state.store(state.record.clone())?;
        }
        Ok(state)
    }
    pub fn tasks(&self) -> &[Task] {
        &self.record.tasks
    }
    pub fn history(&self) -> &[Run] {
        &self.record.history
    }
    pub fn mark_read(&mut self) -> Result<(), String> {
        let mut record = self.record.clone();
        for run in &mut record.history {
            run.unread = false;
        }
        self.store(record)
    }
    pub fn save(&mut self, id: Option<u64>, mut spec: TaskSpec, now: i64) -> Result<(), String> {
        spec.name = spec.name.trim().into();
        spec.prompt = spec.prompt.trim().into();
        let due = spec.validate(now)?;
        let mut record = self.record.clone();
        if let Some(id) = id {
            let task = record
                .tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or("任务已不存在。")?;
            if task.status.active() {
                return Err("运行中的任务不能编辑。".into());
            }
            task.spec = spec;
            task.next_due = due;
            task.status = Status::Ready;
        } else {
            if record.tasks.len() >= 100 {
                return Err("最多保存 100 个定时任务。".into());
            }
            let id = loop {
                let id = rand::random();
                if record.tasks.iter().all(|t| t.id != id) {
                    break id;
                }
            };
            record.tasks.push(Task {
                id,
                spec,
                enabled: true,
                next_due: due,
                status: Status::Ready,
                last_run: None,
            });
        }
        self.store(record)
    }
    pub fn toggle(&mut self, id: u64, now: i64) -> Result<(), String> {
        let mut record = self.record.clone();
        let task = record
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("任务已不存在。")?;
        task.enabled = !task.enabled;
        if task.enabled {
            task.next_due = task.spec.schedule.next_due(now)?;
        }
        if task.status == Status::Pending {
            task.status = Status::Ready;
        }
        self.store(record)
    }
    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        let mut record = self.record.clone();
        if record.tasks.iter().any(|t| t.id == id && t.status.active()) {
            return Err("请先中断或等待任务结束再删除。".into());
        }
        record.tasks.retain(|t| t.id != id);
        self.store(record)
    }
    pub fn poll(&mut self, now: i64) -> Result<(), String> {
        let mut record = self.record.clone();
        let mut changed = false;
        for task in &mut record.tasks {
            if !task.enabled || task.next_due > now {
                continue;
            }
            changed = true;
            // Coalesce missed intervals and avoid adding a backlog while busy.
            if !task.status.active() && task.status != Status::Unknown {
                task.status = Status::Pending;
            }
            task.next_due = task.spec.schedule.next_due(now)?;
            if matches!(task.spec.schedule, Schedule::Once { .. }) {
                task.enabled = false;
            }
        }
        if changed { self.store(record) } else { Ok(()) }
    }
    pub fn queue(&mut self, id: u64) -> Result<(), String> {
        let mut record = self.record.clone();
        let task = record
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("任务已不存在。")?;
        if task.status.active() {
            return Err("任务仍在运行。".into());
        }
        task.status = Status::Pending;
        self.store(record)
    }
    pub fn begin(&mut self, id: u64, now: i64) -> Result<Run, String> {
        let mut record = self.record.clone();
        if record.history.iter().any(|r| r.status.active()) {
            return Err("已有后台任务正在执行。".into());
        }
        let task = record
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("任务已不存在。")?;
        if task.status != Status::Pending {
            return Err("任务未在等待队列中。".into());
        }
        task.status = Status::Dispatching;
        task.last_run = Some(now);
        let run_id = loop {
            let candidate = rand::random();
            if record.history.iter().all(|r| r.id != candidate) {
                break candidate;
            }
        };
        let run = Run {
            id: run_id,
            task_id: id,
            spec: task.spec.clone(),
            started_at: now,
            finished_at: None,
            status: Status::Dispatching,
            output: String::new(),
            error: String::new(),
            unread: false,
        };
        record.history.insert(0, run.clone());
        record.history.truncate(MAX_HISTORY);
        self.store(record)?;
        Ok(run)
    }
    pub fn sent(&mut self, id: u64) -> Result<(), String> {
        let mut record = self.record.clone();
        let run = record
            .history
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("执行记录已不存在。")?;
        run.status = Status::Running;
        if let Some(task) = record.tasks.iter_mut().find(|t| t.id == run.task_id) {
            task.status = Status::Running;
        }
        self.store(record)
    }
    pub fn finish(&mut self, id: u64, outcome: &Outcome, now: i64) -> Result<(), String> {
        if outcome.status.active() || matches!(outcome.status, Status::Ready | Status::Pending) {
            return Err("执行结果无效。".into());
        }
        let mut record = self.record.clone();
        let run = record
            .history
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("执行记录已不存在。")?;
        run.status = outcome.status;
        run.output = outcome.output.chars().take(MAX_OUTPUT_CHARS).collect();
        run.error = outcome.error.chars().take(MAX_OUTPUT_CHARS).collect();
        run.finished_at = Some(now);
        run.unread = true;
        if let Some(task) = record.tasks.iter_mut().find(|t| t.id == run.task_id) {
            task.status = outcome.status;
        }
        self.store(record)
    }
    fn store(&mut self, record: Record) -> Result<(), String> {
        let parent = self.path.parent().ok_or("定时任务路径无效。")?;
        fs::create_dir_all(parent).map_err(|e| format!("保存定时任务失败：{e}"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| format!("保存定时任务失败：{e}"))?;
        let bytes = serde_json::to_vec(&record).map_err(|_| "无法编码定时任务。")?;
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|e| format!("保存定时任务失败：{e}"))?;
        temporary
            .persist(&self.path)
            .map_err(|e| format!("保存定时任务失败：{}", e.error))?;
        self.record = record;
        Ok(())
    }
}

/// An independent Codex session with explicitly authorized approvalPolicy=never.
/// Sandbox permissions inherit the host; events stay out of interactive chat.
pub struct Executor {
    client: super::codex::Client,
    messages: Vec<(String, String)>,
}
pub enum ExecutionEvent {
    Prompt(super::codex::Prompt),
    Sent,
    Finished(Outcome),
}
impl Executor {
    pub fn start(
        config: &super::config::CodexConfig,
        run: &Run,
        context: crate::development::context::Manifest,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let mut config = config.clone();
        config.no_approval = true;
        config.working_directory = run.spec.target.workspace.clone();
        config.development_mode = run.spec.target.development_mode;
        let client = super::codex::Client::start_in_workspace(
            &config,
            &run.spec.target.root,
            &run.spec.target.character,
            None,
            wake,
        )?;
        client.submit_with_context(run.spec.prompt.clone(), config.no_approval, context)?;
        Ok(Self {
            client,
            messages: Vec::new(),
        })
    }
    pub fn interrupt(&self) {
        self.client.interrupt();
    }
    pub fn reply(&self, id: serde_json::Value, answer: serde_json::Value) {
        self.client.reply(id, answer);
    }
    pub fn poll(&mut self) -> Vec<ExecutionEvent> {
        use super::codex::Event;
        let started = std::time::Instant::now();
        let mut events = Vec::new();
        for _ in 0..64 {
            if started.elapsed() >= std::time::Duration::from_millis(4) {
                break;
            }
            let Some(event) = self.client.poll() else {
                break;
            };
            let mut terminal = None;
            match event {
                Event::Text { id, text, replace } => {
                    let index = self
                        .messages
                        .iter()
                        .position(|(key, _)| *key == id)
                        .unwrap_or_else(|| {
                            self.messages.push((id, String::new()));
                            self.messages.len() - 1
                        });
                    if replace {
                        self.messages[index].1 = text;
                    } else {
                        self.messages[index].1.push_str(&text);
                    }
                    if self
                        .messages
                        .iter()
                        .map(|(_, text)| text.len())
                        .sum::<usize>()
                        > 1_048_576
                    {
                        terminal = Some((Status::Failed, "后台回复超过 1 MiB，已停止。".into()));
                    }
                }
                Event::Manifest(manifest) if !manifest.preview => events.push(ExecutionEvent::Sent),
                Event::Prompt(prompt) => {
                    if prompt.questions.is_empty() {
                        self.client
                            .reply(prompt.id, serde_json::json!({"decision":"accept"}));
                    } else {
                        events.push(ExecutionEvent::Prompt(prompt));
                    }
                }
                Event::Finished { interrupted } => {
                    terminal = Some((
                        if interrupted {
                            Status::Interrupted
                        } else {
                            Status::Completed
                        },
                        String::new(),
                    ))
                }
                Event::Failed(error) | Event::Stopped(error) => {
                    terminal = Some((Status::Failed, error))
                }
                _ => {}
            }
            if let Some((status, error)) = terminal {
                let output = self
                    .messages
                    .iter()
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                events.push(ExecutionEvent::Finished(Outcome {
                    status,
                    output: self.client.sanitize(&output),
                    error: self.client.sanitize(&error),
                }));
                break;
            }
        }
        events
    }
    pub fn interrupted(&self) -> Outcome {
        Outcome {
            status: Status::Interrupted,
            output: self.client.sanitize(
                &self
                    .messages
                    .iter()
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            ),
            error: "应用已退出，后台任务已停止。".into(),
        }
    }
    #[cfg(test)]
    pub(crate) fn mock(mode: &str, prompt: &str) -> Self {
        let client = super::codex::Client::mock(mode);
        client
            .submit(
                prompt.into(),
                mode == "schedule" || prompt.starts_with("no-approval"),
            )
            .unwrap();
        Self {
            client,
            messages: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 1_800_000_000;
    fn spec(schedule: Schedule) -> TaskSpec {
        TaskSpec {
            name: "晚间检查".into(),
            prompt: "检查项目".into(),
            schedule,
            target: Target {
                root: std::env::temp_dir(),
                workspace: None,
                character: "fixture".into(),
                development_mode: false,
            },
        }
    }
    #[test]
    fn schedule_coalesces_due_times_and_persists_queue_history_and_deletion() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("schedule.json");
        let mut state = ScheduleState::load(path.clone()).unwrap();
        let time = super::super::clock::local_time(NOW + 120).unwrap().label();
        for schedule in [
            Schedule::Once { time: time.clone() },
            Schedule::Daily { time },
            Schedule::Interval { minutes: 10 },
        ] {
            state.save(None, spec(schedule), NOW).unwrap();
        }
        state.poll(NOW).unwrap();
        assert!(state.tasks().iter().all(|t| t.status == Status::Ready));
        let later = NOW + 5 * 86400;
        state.poll(later).unwrap();
        assert!(state.tasks().iter().all(|t| t.status == Status::Pending));
        assert!(!state.tasks()[0].enabled);
        assert!(state.tasks()[1..].iter().all(|t| t.next_due > later));
        assert_eq!(
            state.tasks()[1].spec.schedule.next_due(later).unwrap(),
            state.tasks()[1].next_due
        );
        let mut state = ScheduleState::load(path.clone()).unwrap();
        state.poll(later + 1).unwrap();
        assert!(state.history().is_empty());
        let id = state.tasks()[0].id;
        let run = state.begin(id, later).unwrap();
        assert!(state.begin(state.tasks()[1].id, later).is_err());
        assert!(state.delete(id).is_err());
        assert!(
            state
                .save(Some(id), spec(Schedule::Interval { minutes: 1 }), later)
                .is_err()
        );
        state.sent(run.id).unwrap();
        state
            .finish(
                run.id,
                &Outcome {
                    status: Status::Completed,
                    output: "检查完成".into(),
                    error: String::new(),
                },
                later + 5,
            )
            .unwrap();
        state.delete(id).unwrap();
        let mut state = ScheduleState::load(path.clone()).unwrap();
        assert_eq!(state.history()[0].output, "检查完成");
        assert_eq!(state.history()[0].spec.name, "晚间检查");
        assert!(state.history()[0].unread);
        state.mark_read().unwrap();
        assert!(!ScheduleState::load(path.clone()).unwrap().history()[0].unread);
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
    fn schedule_restart_marks_unconfirmed_run_and_requires_explicit_retry() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("schedule.json");
        let mut state = ScheduleState::load(path.clone()).unwrap();
        state
            .save(None, spec(Schedule::Interval { minutes: 1 }), NOW)
            .unwrap();
        let id = state.tasks()[0].id;
        state.poll(NOW + 60).unwrap();
        state.begin(id, NOW + 60).unwrap();
        let mut state = ScheduleState::load(path.clone()).unwrap();
        assert!(state.tasks()[0].status == Status::Unknown);
        assert!(state.history()[0].status == Status::Unknown && state.history()[0].unread);
        state.poll(NOW + 86400).unwrap();
        assert!(state.tasks()[0].status == Status::Unknown);
        state.queue(id).unwrap();
        let run = state.begin(id, NOW + 86400).unwrap();
        state
            .finish(
                run.id,
                &Outcome {
                    status: Status::Failed,
                    output: String::new(),
                    error: "模拟失败".into(),
                },
                NOW + 86401,
            )
            .unwrap();
        assert_eq!(state.history().len(), 2);
        assert_eq!(state.history()[0].error, "模拟失败");
    }
    #[test]
    fn schedule_validation_and_failed_writes_preserve_records_and_memory() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("schedule.json");
        for invalid in ["invalid", r#"{"version":2,"tasks":[],"history":[]}"#] {
            fs::write(&path, invalid).unwrap();
            assert!(ScheduleState::load(path.clone()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
        fs::remove_file(&path).unwrap();
        let mut state = ScheduleState::load(path.clone()).unwrap();
        for schedule in [
            Schedule::Daily {
                time: "24:00".into(),
            },
            Schedule::Once {
                time: "1:00".into(),
            },
            Schedule::Interval { minutes: 0 },
        ] {
            assert!(state.save(None, spec(schedule), NOW).is_err());
        }
        let mut invalid = spec(Schedule::Interval { minutes: 1 });
        invalid.prompt = "字".repeat(MAX_PROMPT_CHARS + 1);
        assert!(state.save(None, invalid, NOW).is_err());
        state
            .save(None, spec(Schedule::Interval { minutes: 1 }), NOW)
            .unwrap();
        let id = state.tasks()[0].id;
        state.toggle(id, NOW).unwrap();
        state.poll(NOW + 3600).unwrap();
        assert!(state.tasks()[0].status == Status::Ready);
        state.toggle(id, NOW + 3600).unwrap();
        assert_eq!(state.tasks()[0].next_due, NOW + 3660);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(state.poll(NOW + 3660).is_err());
        assert!(state.tasks()[0].status == Status::Ready);
        assert!(state.delete(id).is_err());
        assert_eq!(state.tasks().len(), 1);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[test]
    fn schedule_executor_streams_finishes_interrupts_and_routes_questions() {
        use std::time::{Duration, Instant};
        for (prompt, mode, expected) in [
            ("no-approval 后台执行", "schedule", Status::Completed),
            ("fail", "schedule", Status::Failed),
            ("interrupt", "schedule", Status::Interrupted),
            ("approve", "schedule", Status::Completed),
            ("question", "schedule", Status::Completed),
        ] {
            let mut executor = Executor::mock(mode, prompt);
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut outcome = None;
            let mut sent = false;
            let mut stopped = false;
            while outcome.is_none() && Instant::now() < deadline {
                for event in executor.poll() {
                    match event {
                        ExecutionEvent::Sent => sent = true,
                        ExecutionEvent::Prompt(prompt) => executor.reply(
                            prompt.id,
                            serde_json::json!({"answers":{"choice":{"answers":["A"]}}}),
                        ),
                        ExecutionEvent::Finished(result) => outcome = Some(result),
                    }
                }
                if prompt == "interrupt" && !executor.messages.is_empty() && !stopped {
                    executor.interrupt();
                    stopped = true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let outcome = outcome.expect("background executor timed out");
            assert!(outcome.status == expected, "{}", outcome.error);
            assert!(sent);
        }
    }
    #[test]
    fn schedule_executor_redacts_assembled_chunks_before_history_truncation() {
        use super::super::codex::{Client, Event};
        let mut executor = Executor {
            client: Client::mock_events(
                vec![
                    Event::Text {
                        id: "r".into(),
                        text: "prefix private-".into(),
                        replace: false,
                    },
                    Event::Text {
                        id: "r".into(),
                        text: "fixture-key suffix".into(),
                        replace: false,
                    },
                    Event::Finished { interrupted: false },
                ],
                vec!["private-fixture-key".into()],
            ),
            messages: Vec::new(),
        };
        let outcome = executor
            .poll()
            .into_iter()
            .find_map(|event| match event {
                ExecutionEvent::Finished(result) => Some(result),
                _ => None,
            })
            .unwrap();
        assert_eq!(outcome.output, "prefix [hidden] suffix");
    }
    #[test]
    #[cfg(unix)]
    fn schedule_start_forces_no_approval_without_changing_interactive_config() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("codex-fixture");
        let fixture = root.path().join("server.py");
        fs::write(
            &fixture,
            include_str!("../../tests/fixtures/codex-server.py"),
        )
        .unwrap();
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nexec python3 -X utf8 -u '{}' schedule\n",
                fixture.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let config = super::super::config::CodexConfig {
            executable: script.to_string_lossy().into(),
            no_approval: false,
            ..Default::default()
        };
        let mut state = ScheduleState::load(root.path().join("state.json")).unwrap();
        let mut spec = spec(Schedule::Interval { minutes: 1 });
        spec.target.root = root.path().to_path_buf();
        spec.prompt = "no-approval scheduled".into();
        state.save(None, spec, NOW).unwrap();
        let id = state.tasks()[0].id;
        state.queue(id).unwrap();
        let run = state.begin(id, NOW).unwrap();
        let mut executor = Executor::start(&config, &run, Default::default(), || {}).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut result = None;
        while result.is_none() && std::time::Instant::now() < deadline {
            result = executor.poll().into_iter().find_map(|event| match event {
                ExecutionEvent::Finished(result) => Some(result),
                _ => None,
            });
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let result = result.expect("background execution timed out");
        assert!(result.status == Status::Completed, "{}", result.error);
        assert!(result.output.contains("no-approval scheduled"));
        assert!(!config.no_approval);
    }
    #[test]
    fn schedule_history_keeps_latest_runs_and_caps_unicode_output() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("schedules.json");
        let mut state = ScheduleState::load(path.clone()).unwrap();
        state
            .save(None, spec(Schedule::Interval { minutes: 1 }), NOW)
            .unwrap();
        let id = state.tasks()[0].id;
        state.queue(id).unwrap();
        let run = state.begin(id, NOW).unwrap();
        let outcome = Outcome {
            status: Status::Completed,
            output: "字".repeat(MAX_OUTPUT_CHARS + 1),
            error: String::new(),
        };
        state.finish(run.id, &outcome, NOW + 1).unwrap();
        assert_eq!(state.history()[0].output.chars().count(), MAX_OUTPUT_CHARS);
        let mut old = state.history()[0].clone();
        old.output.clear();
        state.record.history = (0..MAX_HISTORY)
            .map(|i| {
                let mut item = old.clone();
                item.id = i as u64;
                item
            })
            .collect();
        state.queue(id).unwrap();
        let latest = state.begin(id, NOW + 60).unwrap();
        assert_eq!(state.history().len(), MAX_HISTORY);
        assert_eq!(state.history()[0].id, latest.id);
        assert_eq!(
            state.history()[MAX_HISTORY - 1].id,
            (MAX_HISTORY - 2) as u64
        );
        let mut invalid = state.record.clone();
        invalid.history[1].id = latest.id;
        fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(ScheduleState::load(path).is_err());
    }
}
