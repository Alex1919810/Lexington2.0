//! Project workspaces, task history and execution evidence. No GUI or process ownership.
pub mod context;
pub mod diagnostics;
pub mod journal;
pub mod memory;
pub mod review;
pub mod testing;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const INSTRUCTIONS: &str = "[Lexington development]\n你正在辅助用户进行代码开发。先核对当前工作区、项目规则（含适用的 AGENTS.md）和现有修改，再实施任务。保留用户已有修改，按项目架构做必要改动。以真实工具结果汇报执行进度、文件变更和验证结果，明确哪些检查失败或未执行；不能把任务结束当作测试通过。默认选择受影响的增量测试。未经用户明确授权，不提交或推送 Git、不执行破坏性回退。所有操作继续遵守本机 Codex 的审批与沙箱策略。";
const LOG_LIMIT: usize = 256 * 1024;
const STORE_LIMIT: u64 = 32 * 1024 * 1024;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExecutionItem {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub command: String,
    pub cwd: String,
    pub output: String,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<i64>,
    pub changes: Vec<FileChange>,
    pub truncated: bool,
}

impl ExecutionItem {
    pub(super) fn sanitize(&mut self, filter: &dyn Fn(&str) -> String) {
        self.id = filter(&self.id);
        self.kind = filter(&self.kind);
        self.status = filter(&self.status);
        self.command = filter(&self.command);
        self.cwd = filter(&self.cwd);
        self.output = filter(&self.output);
        for change in &mut self.changes {
            change.path = filter(&change.path);
            change.kind = filter(&change.kind);
            change.diff = filter(&change.diff);
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub kind: String,
    pub diff: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Activity {
    Item(Value),
    Output { id: String, delta: String },
    Diff(String),
    Plan(Value),
    Patch { id: String, changes: Value },
}
impl Activity {
    pub fn redact(&mut self, secret: &str) {
        fn visit(value: &mut Value, secret: &str) {
            match value {
                Value::String(text) => *text = text.replace(secret, "[hidden]"),
                Value::Array(values) => values.iter_mut().for_each(|v| visit(v, secret)),
                Value::Object(values) => values.values_mut().for_each(|v| visit(v, secret)),
                _ => (),
            }
        }
        match self {
            Self::Item(v) | Self::Plan(v) => visit(v, secret),
            Self::Patch { changes, .. } => visit(changes, secret),
            Self::Output { delta, .. } | Self::Diff(delta) => {
                *delta = delta.replace(secret, "[hidden]")
            }
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OutputBoundary {
    pub supplement: usize,
    pub messages: Vec<(String, String)>,
    pub items: Vec<String>,
}
impl OutputBoundary {
    pub(super) fn sanitize(&mut self, filter: &dyn Fn(&str) -> String) {
        for (id, text) in &mut self.messages {
            *id = filter(id);
            *text = filter(text);
        }
        for id in &mut self.items {
            *id = filter(id);
        }
    }
    fn message_end(&self, id: &str, text: &str) -> usize {
        self.messages
            .iter()
            .find(|(key, _)| key == id)
            .map_or(0, |(_, prefix)| {
                prefix
                    .chars()
                    .zip(text.chars())
                    .take_while(|(a, b)| a == b)
                    .map(|(ch, _)| ch.len_utf8())
                    .sum()
            })
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Turn {
    pub review: Option<Vec<review::FileReview>>,
    pub manifest: Option<context::Manifest>,
    pub journal: Option<journal::Journal>,
    #[serde(skip)]
    pub(crate) pending: Vec<journal::Pending>,
    pub user: String,
    pub supplements: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub output_boundaries: Vec<OutputBoundary>,
    pub messages: Vec<(String, String)>,
    pub items: Vec<ExecutionItem>,
    pub diff: String,
    pub plan: Value,
    pub status: String,
    pub error: String,
    pub started_at: u64,
}
impl Turn {
    pub fn sanitize(&mut self, filter: impl Fn(&str) -> String) {
        self.sanitize_with(&filter);
    }
    pub(super) fn sanitize_with(&mut self, filter: &dyn Fn(&str) -> String) {
        self.user = filter(&self.user);
        for text in &mut self.supplements {
            *text = filter(text);
        }
        for boundary in &mut self.output_boundaries {
            boundary.sanitize(filter);
        }
        self.status = filter(&self.status);
        self.error = filter(&self.error);
        self.diff = filter(&self.diff);
        if let Some(files) = &mut self.review {
            for file in files {
                file.sanitize(filter);
            }
        }
        journal::sanitize_value(&mut self.plan, filter);
        for (id, text) in &mut self.messages {
            *id = filter(id);
            *text = filter(text);
        }
        for item in &mut self.items {
            item.sanitize(filter);
        }
        if let Some(manifest) = &mut self.manifest {
            manifest.sanitize(filter);
        }
        if let Some(journal) = &mut self.journal {
            for record in &mut journal.events {
                record.data.sanitize(filter);
            }
        }
        for pending in &mut self.pending {
            if let journal::Pending::Message(id) | journal::Pending::Execution(id) = pending {
                *id = filter(id);
            }
        }
    }
    pub fn activity(&mut self, event: Activity) {
        match &event {
            Activity::Diff(_) => self.changed(journal::Pending::Diff),
            Activity::Plan(_) => self.changed(journal::Pending::Plan),
            Activity::Output { id, .. } | Activity::Patch { id, .. } => {
                self.changed(journal::Pending::Execution(id.clone()))
            }
            Activity::Item(value) => {
                if let Some(id) = value["id"].as_str() {
                    self.changed(journal::Pending::Execution(id.into()));
                }
            }
        }
        match event {
            Activity::Diff(diff) => self.diff = diff,
            Activity::Plan(plan) => self.plan = plan,
            Activity::Patch { id, changes } => {
                let value = serde_json::json!({"id":id,"type":"fileChange","changes":changes});
                self.activity(Activity::Item(value));
            }
            Activity::Output { id, delta } => {
                let item = self.item(&id);
                item.output.push_str(&delta);
                item.truncated |= limit_text(&mut item.output, LOG_LIMIT);
            }
            Activity::Item(value) => {
                let Some(id) = value["id"].as_str() else {
                    return;
                };
                let item = self.item(id);
                for (field, target) in [
                    ("type", &mut item.kind),
                    ("status", &mut item.status),
                    ("command", &mut item.command),
                    ("cwd", &mut item.cwd),
                ] {
                    if let Some(text) = value[field].as_str() {
                        *target = text.into();
                    }
                }
                item.exit_code = value["exitCode"].as_i64().or(item.exit_code);
                item.duration_ms = value["durationMs"].as_i64().or(item.duration_ms);
                if let Some(output) = value["aggregatedOutput"].as_str() {
                    item.output = output.into();
                    item.truncated = limit_text(&mut item.output, LOG_LIMIT);
                }
                if let Some(changes) = value["changes"].as_array() {
                    item.changes = changes
                        .iter()
                        .map(|c| FileChange {
                            path: c["path"].as_str().unwrap_or_default().into(),
                            kind: c["kind"]["type"].as_str().unwrap_or("update").into(),
                            diff: c["diff"].as_str().unwrap_or_default().into(),
                        })
                        .collect();
                }
            }
        }
    }
    fn item(&mut self, id: &str) -> &mut ExecutionItem {
        let index = self
            .items
            .iter()
            .position(|i| i.id == id)
            .unwrap_or_else(|| {
                self.items.push(ExecutionItem {
                    id: id.into(),
                    ..Default::default()
                });
                self.items.len() - 1
            });
        &mut self.items[index]
    }
    pub fn text(&mut self, id: String, text: String, replace: bool) {
        self.changed(journal::Pending::Message(id.clone()));
        if let Some((_, current)) = self.messages.iter_mut().find(|(item, _)| item == &id) {
            if replace {
                *current = text;
            } else {
                current.push_str(&text);
            }
        } else {
            self.messages.push((id, text));
        }
    }
    /// Capture the send position; only persist it when the supplement succeeds.
    pub fn output_boundary(&self) -> OutputBoundary {
        OutputBoundary {
            supplement: self.supplements.len(),
            messages: self.messages.clone(),
            items: self.items.iter().map(|item| item.id.clone()).collect(),
        }
    }
    /// Split cumulative replies, including updates to the same message ID.
    /// Prefixes remain valid after credential redaction.
    pub fn output_messages(&self, section: usize) -> impl Iterator<Item = &str> {
        let boundary = |index| {
            self.output_boundaries
                .iter()
                .find(|boundary| boundary.supplement >= index)
        };
        let before = section.checked_sub(1).and_then(boundary);
        let after = boundary(section);
        self.messages.iter().filter_map(move |(id, text)| {
            let start = if section == 0 {
                0
            } else {
                before.map_or(text.len(), |boundary| boundary.message_end(id, text))
            };
            let end = after.map_or(text.len(), |boundary| boundary.message_end(id, text));
            (end > start).then(|| &text[start..end])
        })
    }
    pub fn output_items(&self, section: usize) -> impl Iterator<Item = &ExecutionItem> {
        self.items.iter().filter(move |item| {
            let position = self
                .output_boundaries
                .iter()
                .find(|boundary| boundary.items.contains(&item.id))
                .map_or(self.supplements.len(), |boundary| boundary.supplement);
            // Histories without boundaries retain their original grouped layout.
            section
                == if self.output_boundaries.is_empty() {
                    0
                } else {
                    position
                }
        })
    }
}
fn limit_text(text: &mut String, limit: usize) -> bool {
    if text.len() <= limit {
        return false;
    }
    let mut boundary = text.len() - limit;
    while !text.is_char_boundary(boundary) {
        boundary += 1;
    }
    text.drain(..boundary);
    true
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub character: String,
    pub development_mode: bool,
    pub memory_revision: u64,
    pub memory_context_stamp: String,
    pub thread_id: Option<String>,
    pub turns: Vec<Turn>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Workspace {
    pub path: PathBuf,
    pub tasks: Vec<Task>,
    pub current_task: Option<u64>,
    pub diagnostics: Option<diagnostics::DiagnosticSnapshot>,
    pub tests: Vec<testing::TestDefinition>,
    pub test_runs: Vec<testing::TestRun>,
    pub memories: Vec<memory::ProjectMemory>,
    pub memory_revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Workspaces {
    pub version: u32,
    pub projects: Vec<Workspace>,
    pub active: Option<PathBuf>,
}
impl Default for Workspaces {
    fn default() -> Self {
        Self {
            version: 3,
            projects: Vec::new(),
            active: None,
        }
    }
}
impl Workspaces {
    pub fn read(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self {
                version: 3,
                ..Default::default()
            });
        }
        if fs::metadata(path)
            .map_err(|_| "无法读取 development 记录。")?
            .len()
            > STORE_LIMIT
        {
            return Err("development 记录超过大小限制，未加载或覆盖。".into());
        }
        let mut book: Self =
            serde_json::from_slice(&fs::read(path).map_err(|_| "无法读取 development 记录。")?)
                .map_err(|_| "development 记录损坏，未覆盖原文件。")?;
        if !matches!(book.version, 1..=3) {
            return Err("development 记录版本不支持。".into());
        }
        book.version = 3;
        for project in &mut book.projects {
            for run in &mut project.test_runs {
                if run.status == "running" {
                    run.status = "interrupted".into();
                }
            }
            for task in &mut project.tasks {
                for turn in &mut task.turns {
                    turn.restore_journal()?;
                    if turn.status == "running" {
                        turn.finish("interrupted", "应用退出或中断后恢复，未确认任务完成。");
                    }
                }
            }
        }
        Ok(book)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        // Refuse to overwrite a damaged or newer store, including external edits.
        Self::read(path)?;
        let contents = serde_json::to_vec(self).map_err(|_| "无法编码 development 记录。")?;
        if contents.len() as u64 > STORE_LIMIT {
            return Err("development 记录已满，请先删除旧任务。".into());
        }
        let parent = path.parent().ok_or("development 记录路径无效。")?;
        fs::create_dir_all(parent).map_err(|_| "无法创建 development 记录目录。")?;
        let mut file =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| "无法保存 development 记录。")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| "无法保护 development 记录。")?;
        }
        file.write_all(&contents)
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "无法保存 development 记录。")?;
        file.persist(path)
            .map_err(|_| "无法替换 development 记录。")?;
        Ok(())
    }
    pub fn add(&mut self, path: &Path) -> Result<PathBuf, String> {
        let path = path
            .canonicalize()
            .map_err(|_| "项目目录不存在或无法访问。")?;
        if !path.is_dir() {
            return Err("请选择项目目录。".into());
        }
        if !self.projects.iter().any(|p| p.path == path) {
            self.projects.push(Workspace {
                path: path.clone(),
                ..Default::default()
            });
        }
        Ok(path)
    }
    pub fn route(
        &self,
        workspace: Option<&str>,
        files: &[String],
    ) -> Result<Option<PathBuf>, String> {
        if let Some(workspace) = workspace {
            let path = Path::new(workspace)
                .canonicalize()
                .map_err(|_| "来源项目目录不可用。")?;
            if !path.is_dir() {
                return Err("来源项目不是目录。".into());
            }
            for file in files {
                if !Path::new(file)
                    .canonicalize()
                    .map_err(|_| "来源文件不可用。")?
                    .starts_with(&path)
                {
                    return Err("来源文件不属于所选项目，请分项目发送。".into());
                }
            }
            return Ok(Some(path));
        }
        let mut result = None;
        for file in files {
            let file = Path::new(file)
                .canonicalize()
                .map_err(|_| "来源文件不可用。")?;
            let project = self
                .projects
                .iter()
                .filter(|p| file.starts_with(&p.path))
                .max_by_key(|p| p.path.components().count());
            let candidate = project.map(|p| p.path.clone()).or_else(|| {
                let start = if file.is_dir() {
                    file.as_path()
                } else {
                    file.parent()?
                };
                start
                    .ancestors()
                    .find(|p| p.join(".git").exists())
                    .map(Path::to_path_buf)
            });
            match (&result, candidate) {
                (_, None) => {
                    return Err("无法确定来源项目，请在 development 中添加目录后重发。".into());
                }
                (Some(previous), Some(path)) if previous != &path => {
                    return Err("文件属于不同项目，请分项目发送。".into());
                }
                (_, Some(path)) => result = Some(path),
            }
        }
        Ok(result)
    }
    pub fn current(&self) -> Option<&Workspace> {
        self.projects
            .iter()
            .find(|p| Some(&p.path) == self.active.as_ref())
    }
    pub fn current_mut(&mut self) -> Option<&mut Workspace> {
        self.projects
            .iter_mut()
            .find(|p| Some(&p.path) == self.active.as_ref())
    }
    pub fn task(&self) -> Option<&Task> {
        let p = self.current()?;
        p.tasks.iter().find(|t| Some(t.id) == p.current_task)
    }
    pub fn task_mut(&mut self) -> Option<&mut Task> {
        let p = self.current_mut()?;
        p.tasks.iter_mut().find(|t| Some(t.id) == p.current_task)
    }
    pub fn begin(&mut self, user: String, character: &str, development_mode: bool) {
        let Some(project) = self.current_mut() else {
            return;
        };
        if !project.tasks.iter().any(|t| {
            Some(t.id) == project.current_task
                && t.character == character
                && t.development_mode == development_mode
        }) {
            let id = rand::random();
            project.tasks.push(Task {
                id,
                title: user
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("新任务")
                    .chars()
                    .take(60)
                    .collect(),
                character: character.into(),
                development_mode,
                memory_revision: project.memory_revision,
                ..Default::default()
            });
            project.current_task = Some(id);
        }
        let turn = Turn {
            user,
            status: "running".into(),
            started_at: testing::now(),
            journal: Some(Default::default()),
            ..Default::default()
        };
        self.task_mut().unwrap().turns.push(turn);
        let turn = self.turn_mut().unwrap();
        turn.record(journal::Data::Started {
            user: turn.user.clone(),
            started_at: turn.started_at,
        });
    }

    pub fn turn_mut(&mut self) -> Option<&mut Turn> {
        self.task_mut()?.turns.last_mut()
    }
}

pub fn branch(root: &Path) -> Option<String> {
    let marker = root.join(".git");
    let git = if marker.is_dir() {
        marker
    } else {
        let text = fs::read_to_string(marker).ok()?;
        root.join(text.trim().strip_prefix("gitdir: ")?)
    };
    let head = fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    Some(
        head.strip_prefix("ref: refs/heads/")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("detached {}", head.chars().take(12).collect::<String>())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn version_one_history_migrates_and_incomplete_test_runs_recover_without_success() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("history.json");
        let document = json!({"version":1,"active":root.path(),"projects":[{"path":root.path(),"currentTask":1,"tasks":[{"id":1,"title":"existing","threadId":"old-thread","turns":[{"user":"keep","status":"completed"}]}],"testRuns":[{"id":2,"status":"running"}]}]});
        fs::write(&path, document.to_string()).unwrap();
        let book = Workspaces::read(&path).unwrap();
        assert_eq!(book.version, 3);
        assert!(book.task().unwrap().turns[0].supplements.is_empty());
        assert_eq!(
            book.task().unwrap().thread_id.as_deref(),
            Some("old-thread")
        );
        assert_eq!(book.current().unwrap().test_runs[0].status, "interrupted");
        assert!(book.task().unwrap().turns[0].journal.is_none());
        let mut version_two = document.clone();
        version_two["version"] = json!(2);
        fs::write(&path, version_two.to_string()).unwrap();
        let migrated = Workspaces::read(&path).unwrap();
        assert_eq!(migrated.version, 3);
        assert!(migrated.task().unwrap().turns[0].journal.is_none());
        book.save(&path).unwrap();
        assert_eq!(
            Workspaces::read(&path).unwrap().task().unwrap().turns[0].user,
            "keep"
        );
        fs::write(&path, r#"{"version":999}"#).unwrap();
        assert!(book.save(&path).is_err());
    }

    #[test]
    fn workspace_routing_is_canonical_nested_and_rejects_cross_project_requests() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("项目 A");
        let nested = a.join("nested");
        let b = root.path().join("B");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(nested.join("a.rs"), "").unwrap();
        fs::write(b.join("b.rs"), "").unwrap();
        let mut book = Workspaces::read(&root.path().join("absent.json")).unwrap();
        book.add(&a).unwrap();
        book.add(&nested).unwrap();
        book.add(&b).unwrap();
        book.add(&a.join(".")).unwrap();
        assert_eq!(book.projects.len(), 3);
        let files = vec![nested.join("a.rs").display().to_string()];
        assert_eq!(
            book.route(None, &files).unwrap(),
            Some(nested.canonicalize().unwrap())
        );
        assert_eq!(
            book.route(Some(a.to_str().unwrap()), &files).unwrap(),
            Some(a.canonicalize().unwrap())
        );
        assert!(book.route(Some(b.to_str().unwrap()), &files).is_err());
        assert!(
            book.route(
                None,
                &[files[0].clone(), b.join("b.rs").display().to_string()]
            )
            .is_err()
        );
        fs::create_dir_all(a.join(".git")).unwrap();
        fs::write(a.join(".git/HEAD"), "ref: refs/heads/feature/dev\n").unwrap();
        assert_eq!(branch(&a).as_deref(), Some("feature/dev"));
    }

    #[test]
    fn history_roundtrips_privately_recovers_interrupted_and_protects_corrupt_stores() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("development.json");
        let mut book = Workspaces::read(&path).unwrap();
        book.active = Some(book.add(root.path()).unwrap());
        book.begin("修复问题".into(), "a", true);
        let turn = book.turn_mut().unwrap();
        turn.supplement("运行中补充".into(), turn.output_boundary());
        book.task_mut().unwrap().thread_id = Some("thread-1".into());
        book.turn_mut()
            .unwrap()
            .text("reply".into(), "回复".into(), true);
        book.turn_mut().unwrap().flush_events();
        book.save(&path).unwrap();
        let loaded = Workspaces::read(&path).unwrap();
        assert_eq!(
            loaded.task().unwrap().thread_id.as_deref(),
            Some("thread-1")
        );
        assert_eq!(loaded.task().unwrap().turns[0].status, "interrupted");
        assert_eq!(loaded.task().unwrap().turns[0].messages[0].1, "回复");
        assert_eq!(loaded.task().unwrap().turns[0].supplements, ["运行中补充"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::write(&path, "broken").unwrap();
        assert!(book.save(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    }

    #[test]
    fn execution_reconciles_out_of_order_deltas_final_items_patches_and_utf8_limits() {
        let mut turn = Turn::default();
        turn.activity(Activity::Output {
            id: "cmd".into(),
            delta: "第一段".into(),
        });
        turn.activity(Activity::Item(json!({"id":"cmd","type":"commandExecution","status":"inProgress","command":"cargo test"})));
        turn.activity(Activity::Output {
            id: "cmd".into(),
            delta: "第二段".into(),
        });
        turn.activity(Activity::Item(json!({"id":"cmd","type":"commandExecution","status":"completed","aggregatedOutput":"第一段第二段","exitCode":1})));
        assert_eq!(turn.items.len(), 1);
        assert_eq!(turn.items[0].output, "第一段第二段");
        assert_eq!(turn.items[0].exit_code, Some(1));
        turn.activity(Activity::Patch {
            id: "patch".into(),
            changes: json!([{"path":"a.rs","kind":{"type":"update"},"diff":"-old\n+new"}]),
        });
        assert_eq!(turn.items[1].changes[0].diff, "-old\n+new");
        turn.activity(Activity::Output {
            id: "cmd".into(),
            delta: "字".repeat(LOG_LIMIT),
        });
        assert!(turn.items[0].truncated);
        assert!(turn.items[0].output.len() <= LOG_LIMIT);
        turn.text("reply".into(), "part".into(), false);
        turn.text("reply".into(), "complete".into(), true);
        assert_eq!(turn.messages.len(), 1);
        assert_eq!(turn.messages[0].1, "complete");
        turn.text("secret".into(), "fixture-".into(), false);
        turn.text("secret".into(), "secret".into(), false);
        turn.supplements.push("fixture-secret".into());
        turn.sanitize(|text| text.replace("fixture-secret", "[hidden]"));
        assert_eq!(turn.messages[1].1, "[hidden]");
        assert_eq!(turn.supplements, ["[hidden]"]);
        let mut activity = Activity::Item(
            json!({"command":"fixture-secret","changes":[{"diff":"fixture-secret"}]}),
        );
        activity.redact("fixture-secret");
        assert!(
            !serde_json::to_string(&activity)
                .unwrap()
                .contains("fixture-secret")
        );
    }
}
