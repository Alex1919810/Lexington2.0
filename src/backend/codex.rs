use super::config::CodexConfig;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum Event {
    Context(Box<crate::roleplay::ContextPlan>),
    Ready,
    ThreadReady(String),
    Activity(crate::development::Activity),
    TestResult(Box<crate::development::testing::TestRun>),
    Text {
        id: String,
        text: String,
        replace: bool,
    },
    Finished {
        interrupted: bool,
    },
    Failed(String),
    Notice(String),
    Prompt(Prompt),
    Stopped(String),
}

#[derive(Debug)]
pub struct Prompt {
    pub id: Value,
    pub title: String,
    pub details: String,
    pub questions: Vec<Question>,
}
#[derive(Debug)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub options: Vec<String>,
    pub secret: bool,
}

enum Input {
    Submit(String, bool, Option<String>),
    Execute(crate::development::testing::TestRequest),
    Preview(String),
    Scene(crate::roleplay::SceneState),
    Interrupt,
    Reply(Value, Value),
    Incoming(Value),
    End(String),
    Shutdown,
}

/// One app-server process and one conversation for the lifetime of the desktop app.
pub struct Client {
    input: mpsc::Sender<Input>,
    events: mpsc::Receiver<Event>,
    child: Arc<Mutex<Process>>,
    worker: Option<JoinHandle<()>>,
    temporary_directory: Option<tempfile::TempDir>,
    secrets: Vec<String>,
}

struct ProtocolSetup {
    instructions: Option<String>,
    session: Option<crate::roleplay::RoleplaySession>,
    secrets: Vec<String>,
    resume: Option<String>,
    schema: Option<Value>,
}
#[cfg(windows)]
fn windows_executable() -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        candidates
            .extend(std::env::split_paths(&paths).map(|directory| directory.join("codex.exe")));
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        for suffix in [".local/bin/codex.exe", ".cargo/bin/codex.exe"] {
            candidates.push(Path::new(&home).join(suffix));
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let base = Path::new(&appdata).join("npm/node_modules/@openai/codex");
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        let target = if cfg!(target_arch = "aarch64") {
            "aarch64-pc-windows-msvc"
        } else {
            "x86_64-pc-windows-msvc"
        };
        for vendor in [
            base.join("vendor"),
            base.join(format!("node_modules/@openai/codex-win32-{arch}/vendor")),
        ] {
            for suffix in ["codex/codex.exe", "bin/codex.exe"] {
                candidates.push(vendor.join(target).join(suffix));
            }
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

struct Process {
    child: Child,
    #[cfg(windows)]
    job: crate::tools::windows::Job,
    stopped: bool,
}
impl Process {
    fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        #[cfg(windows)]
        self.job.terminate();
        // npm launchers may spawn the real server. Kill our isolated group before reaping
        // its leader so descendants cannot retain stdout or continue work after app exit.
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Client {
    pub fn start_structured(
        config: &CodexConfig,
        root: &Path,
        instructions: String,
        schema: Value,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let temporary = tempfile::Builder::new()
            .prefix("lexington-memory-")
            .tempdir()
            .map_err(|_| "无法创建后台提炼工作目录。")?;
        let mut isolated = config.clone();
        isolated.working_directory = Some(temporary.path().into());
        let mut command = Self::command(&isolated, root)?;
        for setting in [
            "features.shell_tool=false",
            "features.apps=false",
            "features.plugins=false",
            "features.multi_agent=false",
            "features.browser_use=false",
            "features.computer_use=false",
            "features.image_generation=false",
            "features.skill_search=false",
            "features.tool_suggest=false",
            "web_search=\"disabled\"",
            "tools.update_plan.enabled=false",
        ] {
            command.args(["-c", setting]);
        }
        let mut client =
            Self::launch_options(command, Some(instructions), None, wake, Some(schema), None)?;
        client.temporary_directory = Some(temporary);
        Ok(client)
    }
    pub fn start_in_workspace(
        config: &CodexConfig,
        root: &Path,
        character: &str,
        resume: Option<String>,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let mut card = crate::roleplay::session_card(root, character)?;
        if config.development_mode
            && let Some(card) = &mut card
        {
            card.vector_memory.enabled = false;
        }
        let session = card
            .map(|card| crate::roleplay::RoleplaySession::new(card).with_memory(root, character));
        let mut instructions = if let Some(session) = &session {
            Some(session.initial_plan().text(true))
        } else {
            crate::roleplay::agent_instructions(root, character)?
        };
        if config.development_mode {
            let instructions = instructions.get_or_insert_with(String::new);
            instructions.push_str("\n\n");
            instructions.push_str(crate::development::INSTRUCTIONS);
        }
        Self::launch_options(
            Self::command(config, root)?,
            instructions,
            session,
            wake,
            None,
            resume,
        )
    }
    fn command(config: &CodexConfig, root: &Path) -> Result<Command, String> {
        let directory = config.working_directory.as_ref().map_or_else(
            || root.to_path_buf(),
            |dir| {
                if dir.is_absolute() {
                    dir.clone()
                } else {
                    root.join(dir)
                }
            },
        );
        let directory = directory
            .canonicalize()
            .map_err(|e| format!("Codex 工作目录不可用：{e}"))?;
        let configured = if config.executable == "codex" {
            std::env::var("LEXINGTON_CODEX_EXECUTABLE")
                .unwrap_or_else(|_| config.executable.clone())
        } else {
            config.executable.clone()
        };
        #[cfg(windows)]
        let executable = if configured == "codex" {
            windows_executable().ok_or(
                "找不到原生 codex.exe，请安装 Codex CLI 或设置 LEXINGTON_CODEX_EXECUTABLE。",
            )?
        } else {
            std::path::PathBuf::from(&configured)
        };
        #[cfg(not(windows))]
        let mut executable = std::path::PathBuf::from(&configured);
        #[cfg(not(windows))]
        if configured == "codex" {
            let in_path = std::env::var_os("PATH").is_some_and(|paths| {
                std::env::split_paths(&paths).any(|p| p.join("codex").is_file())
            });
            if !in_path && let Some(home) = std::env::var_os("HOME") {
                for suffix in [".local/bin/codex", ".cargo/bin/codex"] {
                    let path = Path::new(&home).join(suffix);
                    if path.is_file() {
                        executable = path;
                        break;
                    }
                }
            }
        }
        #[cfg(windows)]
        if executable
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        {
            return Err("请使用原生 codex.exe。".into());
        }
        let mut command = Command::new(executable);
        command
            .args(["app-server", "--stdio"])
            .current_dir(directory);
        #[cfg(windows)]
        crate::tools::proxy::inherit_system_proxy(&mut command);
        Ok(command)
    }
    fn launch_options(
        mut command: Command,
        instructions: Option<String>,
        session: Option<crate::roleplay::RoleplaySession>,
        wake: impl Fn() + Send + 'static,
        schema: Option<Value>,
        resume: Option<String>,
    ) -> Result<Self, String> {
        let environment: HashMap<_, _> = command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        let lookup = |name: &str| {
            environment.get(name).cloned().unwrap_or_else(|| {
                if cfg!(test) {
                    None
                } else {
                    std::env::var(name).ok()
                }
            })
        };
        let mut secrets: Vec<String> = ["OPENAI_API_KEY", "LEXINGTON_MODEL_API_KEY"]
            .iter()
            .filter_map(|name| lookup(name))
            .filter(|v| !v.is_empty())
            .collect();
        let home = lookup("CODEX_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                lookup("HOME")
                    .or_else(|| lookup("USERPROFILE"))
                    .map(|home| Path::new(&home).join(".codex"))
            });
        if let Some(home) = home
            && let Ok(local) = super::codex_config::LocalCodexConfig::read(&home)
            && !local.api_key.is_empty()
        {
            secrets.push(local.api_key);
        }
        secrets.sort_by_key(|key| std::cmp::Reverse(key.len()));
        secrets.dedup();
        let client_secrets = secrets.clone();
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000004);
        }
        let directory = command
            .get_current_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                format!(
                    "无法启动 Codex：{e}。请安装并登录 Codex，或设置 LEXINGTON_CODEX_EXECUTABLE。"
                )
            })?;
        #[cfg(windows)]
        let job = match crate::tools::windows::Job::attach(&child) {
            Ok(job) => job,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("无法隔离 Codex 子进程，未启动会话。".into());
            }
        };
        let stdout = child.stdout.take().expect("piped stdout");
        let mut stdin = child.stdin.take().expect("piped stdin");
        let child = Arc::new(Mutex::new(Process {
            child,
            #[cfg(windows)]
            job,
            stopped: false,
        }));
        let owned_child = child.clone();
        let (input, receiver) = mpsc::channel();
        let reader_input = input.clone();
        let (events, output) = mpsc::channel();
        let worker = thread::spawn(move || {
            let reader = thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    match line {
                        Ok(line) if line.trim().is_empty() => continue,
                        Ok(line) => match serde_json::from_str(&line) {
                            Ok(value) => {
                                if reader_input.send(Input::Incoming(value)).is_err() {
                                    return;
                                }
                            }
                            Err(_) => {
                                let _ = reader_input.send(Input::End(
                                    "Codex 返回了无法识别的消息，请检查 CLI 版本。".into(),
                                ));
                                return;
                            }
                        },
                        Err(_) => break,
                    }
                }
                let _ =
                    reader_input.send(Input::End("Codex 进程已断开，请在设置中重启会话。".into()));
            });
            let emit = |mut event| {
                // Provider errors can echo request details. Keep credentials out of UI/logs.
                for secret in &secrets {
                    match &mut event {
                        Event::Context(plan) => plan.redact(secret),
                        Event::Activity(activity) => activity.redact(secret),
                        Event::TestResult(run) => {
                            run.sanitize(|text| text.replace(secret, "[hidden]"))
                        }
                        Event::Failed(text)
                        | Event::Stopped(text)
                        | Event::Notice(text)
                        | Event::Text { text, .. } => *text = text.replace(secret, "[hidden]"),
                        Event::Prompt(prompt) => {
                            prompt.title = prompt.title.replace(secret, "[hidden]");
                            prompt.details = prompt.details.replace(secret, "[hidden]");
                        }
                        _ => (),
                    }
                }
                let _ = events.send(event);
                wake();
            };
            let result = communicate(
                &mut stdin,
                &directory,
                ProtocolSetup {
                    instructions,
                    session,
                    secrets: secrets.clone(),
                    resume,
                    schema,
                },
                receiver,
                &emit,
            );
            // Reap on every exit path, including failed initialization and broken pipes.
            drop(stdin);
            owned_child.lock().unwrap().stop();
            let _ = reader.join();
            if let Err(error) = result {
                emit(Event::Stopped(error));
            }
        });
        Ok(Self {
            input,
            events: output,
            child,
            worker: Some(worker),
            temporary_directory: None,
            secrets: client_secrets,
        })
    }
    pub fn sanitize(&self, text: &str) -> String {
        self.secrets
            .iter()
            .fold(text.to_owned(), |text, key| text.replace(key, "[hidden]"))
    }
    pub fn submit(&self, text: String, no_approval: bool) -> Result<(), String> {
        self.submit_with_context(text, no_approval, None)
    }
    pub fn submit_with_context(
        &self,
        text: String,
        no_approval: bool,
        context: Option<String>,
    ) -> Result<(), String> {
        self.input
            .send(Input::Submit(text, no_approval, context))
            .map_err(|_| "Codex 已停止，请在设置中重启会话。".into())
    }
    pub fn run_test(
        &self,
        request: crate::development::testing::TestRequest,
    ) -> Result<(), String> {
        request.definition.validate()?;
        self.input
            .send(Input::Execute(request))
            .map_err(|_| "Codex 已停止。".into())
    }
    pub fn interrupt(&self) {
        let _ = self.input.send(Input::Interrupt);
    }
    pub fn preview_context(&self, text: String) -> Result<(), String> {
        self.input
            .send(Input::Preview(text))
            .map_err(|_| "Codex 会话已停止。".into())
    }
    pub fn set_scene(&self, scene: crate::roleplay::SceneState) -> Result<(), String> {
        if [&scene.activity, &scene.style, &scene.next_step]
            .iter()
            .any(|text| text.chars().count() > 1024)
        {
            return Err("每项临时场景最多 1024 字符。".into());
        }
        self.input
            .send(Input::Scene(scene))
            .map_err(|_| "Codex 会话已停止。".into())
    }
    pub fn reply(&self, id: Value, result: Value) {
        let _ = self.input.send(Input::Reply(id, result));
    }
    pub fn poll(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.input.send(Input::Shutdown);
        // Killing here also unblocks a writer if an unresponsive server stopped reading.
        self.child.lock().unwrap().stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn write_message(writer: &mut impl Write, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, &value).map_err(|e| format!("Codex 消息发送失败：{e}"))?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|e| format!("Codex 连接已断开：{e}"))
}
fn request(
    writer: &mut impl Write,
    pending: &mut HashMap<u64, (&'static str, Instant)>,
    next: &mut u64,
    method: &'static str,
    params: Value,
) -> Result<(), String> {
    let id = *next;
    *next += 1;
    pending.insert(id, (method, Instant::now()));
    write_message(writer, json!({"id":id,"method":method,"params":params}))
}
fn communicate(
    writer: &mut impl Write,
    directory: &Path,
    setup: ProtocolSetup,
    receiver: mpsc::Receiver<Input>,
    emit: &impl Fn(Event),
) -> Result<(), String> {
    let ProtocolSetup {
        instructions,
        mut session,
        secrets,
        resume,
        schema,
    } = setup;
    let structured = schema.is_some();
    let sanitize = |text: &str| {
        secrets
            .iter()
            .fold(text.to_owned(), |text, key| text.replace(key, "[hidden]"))
    };
    let mut pending = HashMap::new();
    let mut next = 1;
    request(
        writer,
        &mut pending,
        &mut next,
        "initialize",
        json!({"clientInfo":{"name":"lexington","title":"Lexington","version":env!("CARGO_PKG_VERSION")}}),
    )?;
    let mut thread_id: Option<String> = None;
    let mut original_approval_policy = Value::Null;
    let mut turn_id: Option<String> = None;
    let mut active_request = None;
    let mut approval_items = HashMap::<String, Value>::new();
    let mut queued: Option<(String, bool, Option<String>)> = None;
    let mut queued_test: Option<crate::development::testing::TestRequest> = None;
    let mut running_test: Option<(crate::development::testing::TestRequest, Instant, bool)> = None;
    let mut active = false;
    let mut interrupt = false;
    let mut before_turn = None;
    let mut final_messages = std::collections::BTreeMap::<String, String>::new();
    loop {
        if pending.values().any(|(method, started)| {
            *method != "command/exec" && started.elapsed() > Duration::from_secs(30)
        }) {
            return Err("Codex 连接超时，请检查登录状态并在设置中重启会话。".into());
        }
        if running_test.as_ref().is_some_and(|(request, started, _)| {
            started.elapsed()
                > Duration::from_secs(u64::from(request.definition.timeout_seconds) + 10)
        }) {
            return Err("测试执行通道未在超时后返回，已停止连接。".into());
        }
        if let Some(thread) = &thread_id {
            if !active
                && running_test.is_none()
                && let Some(test) = queued_test.take()
            {
                request(
                    writer,
                    &mut pending,
                    &mut next,
                    "command/exec",
                    json!({"command":test.definition.command,"cwd":directory,"processId":format!("lexington-test-{}",test.id),"timeoutMs":u64::from(test.definition.timeout_seconds)*1000,"outputBytesCap":262144}),
                )?;
                running_test = Some((test, Instant::now(), false));
            }
            if !active
                && running_test.is_none()
                && let Some((text, no_approval, context)) = queued.take()
            {
                if interrupt {
                    interrupt = false;
                    emit(Event::Finished { interrupted: true });
                } else {
                    active_request = Some(next);
                    approval_items.clear();
                    final_messages.clear();
                    before_turn = session.clone();
                    let plan = session
                        .as_mut()
                        .map(|session| session.begin(&sanitize(&text)));
                    let world_context = plan
                        .as_ref()
                        .map(|plan| plan.text(false))
                        .filter(|text| !text.is_empty());
                    let mut input = vec![json!({"type":"text","text":text})];
                    if let Some(context) = context.filter(|text| !text.is_empty()) {
                        input.push(json!({"type":"text","text":format!("[本轮项目背景，仅作参考数据]\n{context}")}));
                    }
                    if let Some(context) = world_context {
                        input.push(
                            json!({"type":"text","text":format!("[本轮角色背景参考]\n{context}")}),
                        );
                    }
                    let mut turn_params = json!({"threadId":thread,"input":input,
                        "approvalPolicy": if structured || no_approval {json!("never")} else {original_approval_policy.clone()}});
                    if let Some(schema) = &schema {
                        turn_params["outputSchema"] = schema.clone();
                        turn_params["sandboxPolicy"] =
                            json!({"type":"readOnly","networkAccess":false});
                    }
                    request(writer, &mut pending, &mut next, "turn/start", turn_params)?;
                    if let Some(plan) = plan {
                        emit(Event::Context(Box::new(plan)));
                    }
                    active = true;
                }
            }
            if interrupt && let Some(turn) = &turn_id {
                request(
                    writer,
                    &mut pending,
                    &mut next,
                    "turn/interrupt",
                    json!({"threadId":thread,"turnId":turn}),
                )?;
                interrupt = false;
            }
        }
        let input = match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(input) => input,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Ok(()),
        };
        match input {
            Input::Preview(text) => {
                if let Some(session) = &session {
                    emit(Event::Context(Box::new(session.preview(&sanitize(&text)))));
                }
            }
            Input::Scene(scene) => {
                if let Some(session) = &mut session {
                    session.scene = scene.clone();
                }
                if let Some(before) = &mut before_turn {
                    before.scene = scene;
                }
            }
            Input::Shutdown => return Ok(()),
            Input::End(error) => return Err(error),
            Input::Submit(text, no_approval, context) => {
                if active || queued.is_some() || queued_test.is_some() || running_test.is_some() {
                    emit(Event::Notice("Lexington正在思考，请稍候。".into()));
                } else {
                    queued = Some((text, no_approval, context));
                }
            }
            Input::Execute(test) => {
                if structured
                    || active
                    || queued.is_some()
                    || queued_test.is_some()
                    || running_test.is_some()
                {
                    emit(Event::TestResult(Box::new(
                        crate::development::testing::TestRun {
                            id: test.id,
                            status: "failed".into(),
                            error: "当前执行通道正忙。".into(),
                            ..Default::default()
                        },
                    )));
                } else {
                    queued_test = Some(test);
                }
            }
            Input::Interrupt => {
                if let Some(test) = queued_test.take() {
                    emit(Event::TestResult(Box::new(
                        crate::development::testing::TestRun {
                            id: test.id,
                            status: "cancelled".into(),
                            ..Default::default()
                        },
                    )));
                }
                if let Some((test, _, cancelled)) = &mut running_test {
                    *cancelled = true;
                    request(
                        writer,
                        &mut pending,
                        &mut next,
                        "command/exec/terminate",
                        json!({"processId":format!("lexington-test-{}",test.id)}),
                    )?;
                }
                if active || queued.is_some() {
                    interrupt = true;
                }
            }
            Input::Reply(id, result) => write_message(writer, json!({"id":id,"result":result}))?,
            Input::Incoming(message) => {
                if let Some(method) = message["method"].as_str() {
                    let params = &message["params"];
                    if let Some(id) = message.get("id") {
                        if structured {
                            write_message(
                                writer,
                                json!({"id":id,"error":{"code":-32601,"message":"Background structured extraction does not support tools or interactive requests"}}),
                            )?;
                            return Err("后台提炼请求了交互或工具，已停止。".into());
                        }
                        let prompt = match method {
                            "item/commandExecution/requestApproval"
                            | "item/fileChange/requestApproval" => {
                                let title = if method.contains("commandExecution") {
                                    "Codex 请求执行命令"
                                } else {
                                    "Codex 请求修改文件"
                                };
                                let item = params["itemId"]
                                    .as_str()
                                    .and_then(|id| approval_items.get(id));
                                let mut details = [
                                    params["reason"].as_str(),
                                    params["command"]
                                        .as_str()
                                        .or_else(|| item.and_then(|i| i["command"].as_str())),
                                    params["cwd"]
                                        .as_str()
                                        .or_else(|| item.and_then(|i| i["cwd"].as_str())),
                                    params["grantRoot"].as_str(),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join("\n");
                                if let Some(item) = item {
                                    for change in item["changes"].as_array().into_iter().flatten() {
                                        details.push_str(&format!(
                                            "\n\n{}\n{}\n{}",
                                            change["path"].as_str().unwrap_or_default(),
                                            change["kind"],
                                            change["diff"].as_str().unwrap_or_default()
                                        ));
                                    }
                                }
                                Some(Prompt {
                                    id: id.clone(),
                                    title: title.into(),
                                    details,
                                    questions: vec![],
                                })
                            }
                            "item/tool/requestUserInput"
                                if params["questions"]
                                    .as_array()
                                    .is_some_and(|q| !q.is_empty()) =>
                            {
                                let questions = params["questions"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .map(|q| Question {
                                        id: q["id"].as_str().unwrap_or_default().into(),
                                        text: q["question"].as_str().unwrap_or_default().into(),
                                        options: q["options"]
                                            .as_array()
                                            .into_iter()
                                            .flatten()
                                            .filter_map(|o| o["label"].as_str().map(str::to_owned))
                                            .collect(),
                                        secret: q["isSecret"].as_bool().unwrap_or(false),
                                    })
                                    .collect();
                                Some(Prompt {
                                    id: id.clone(),
                                    title: "Codex 需要补充信息".into(),
                                    details: String::new(),
                                    questions,
                                })
                            }
                            _ => None,
                        };
                        if let Some(prompt) = prompt {
                            emit(Event::Prompt(prompt));
                        } else {
                            write_message(
                                writer,
                                json!({"id":id,"error":{"code":-32601,"message":"Lexington does not support this request"}}),
                            )?;
                            emit(Event::Notice(format!("Codex 请求暂不支持：{method}")));
                        }
                        continue;
                    }
                    if params["threadId"].as_str() != thread_id.as_deref() {
                        continue;
                    }
                    if !active {
                        continue;
                    }
                    // Notifications can arrive before the turn/start response.
                    if let Some(id) = params["turnId"].as_str() {
                        if turn_id.as_deref().is_some_and(|current| current != id) {
                            continue;
                        }
                        turn_id = Some(id.into());
                    }
                    match method {
                        "item/started" => {
                            if !structured
                                && !matches!(
                                    params["item"]["type"].as_str(),
                                    Some("agentMessage" | "reasoning")
                                )
                            {
                                emit(Event::Activity(crate::development::Activity::Item(
                                    params["item"].clone(),
                                )));
                            }
                            let item = &params["item"];
                            if structured
                                && !matches!(
                                    item["type"].as_str(),
                                    Some("agentMessage" | "reasoning")
                                )
                            {
                                return Err("后台提炼尝试使用工具，已停止。".into());
                            }
                            if matches!(
                                item["type"].as_str(),
                                Some("fileChange" | "commandExecution")
                            ) && let Some(id) = item["id"].as_str()
                            {
                                approval_items.insert(id.into(), item.clone());
                            }
                        }
                        "item/agentMessage/delta" => {
                            if let (Some(id), Some(text)) =
                                (params["itemId"].as_str(), params["delta"].as_str())
                            {
                                emit(Event::Text {
                                    id: id.into(),
                                    text: text.into(),
                                    replace: false,
                                });
                            }
                        }
                        "item/completed" => {
                            if !structured
                                && !matches!(
                                    params["item"]["type"].as_str(),
                                    Some("agentMessage" | "reasoning")
                                )
                            {
                                emit(Event::Activity(crate::development::Activity::Item(
                                    params["item"].clone(),
                                )));
                            }
                            let item = &params["item"];
                            if item["type"] == "agentMessage"
                                && let (Some(id), Some(text)) =
                                    (item["id"].as_str(), item["text"].as_str())
                            {
                                final_messages.insert(id.into(), text.into());
                                emit(Event::Text {
                                    id: id.into(),
                                    text: text.into(),
                                    replace: true,
                                });
                            }
                        }
                        "turn/completed" => {
                            if !active {
                                continue;
                            }
                            if turn_id
                                .as_deref()
                                .is_some_and(|id| params["turn"]["id"].as_str() != Some(id))
                            {
                                continue;
                            }
                            for item in params["turn"]["items"].as_array().into_iter().flatten() {
                                if !structured
                                    && !matches!(
                                        item["type"].as_str(),
                                        Some("agentMessage" | "reasoning")
                                    )
                                {
                                    emit(Event::Activity(crate::development::Activity::Item(
                                        item.clone(),
                                    )));
                                }
                                if item["type"] == "agentMessage"
                                    && let (Some(id), Some(text)) =
                                        (item["id"].as_str(), item["text"].as_str())
                                {
                                    final_messages.insert(id.into(), text.into());
                                    emit(Event::Text {
                                        id: id.into(),
                                        text: text.into(),
                                        replace: true,
                                    });
                                }
                            }
                            if params["turn"]["status"] == "failed"
                                || params["turn"]["status"] == "interrupted"
                            {
                                session = before_turn.take();
                            } else if let Some(session) = &mut session {
                                session.finish(&sanitize(
                                    &final_messages
                                        .values()
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                ));
                                before_turn = None;
                            }
                            if params["turn"]["status"] == "failed" {
                                emit(Event::Failed(
                                    params["turn"]["error"]["message"]
                                        .as_str()
                                        .unwrap_or("Codex 请求失败")
                                        .into(),
                                ));
                            } else {
                                emit(Event::Finished {
                                    interrupted: params["turn"]["status"] == "interrupted",
                                });
                            }
                            active = false;
                            active_request = None;
                            approval_items.clear();
                            turn_id = None;
                            interrupt = false;
                        }
                        "item/commandExecution/outputDelta" => {
                            if let (Some(id), Some(delta)) =
                                (params["itemId"].as_str(), params["delta"].as_str())
                            {
                                emit(Event::Activity(crate::development::Activity::Output {
                                    id: id.into(),
                                    delta: delta.into(),
                                }));
                            }
                        }
                        "turn/diff/updated" => {
                            if let Some(diff) = params["diff"].as_str() {
                                emit(Event::Activity(crate::development::Activity::Diff(
                                    diff.into(),
                                )));
                            }
                        }
                        "turn/plan/updated" => emit(Event::Activity(
                            crate::development::Activity::Plan(params.clone()),
                        )),
                        "item/fileChange/patchUpdated" => {
                            if let Some(id) = params["itemId"].as_str() {
                                emit(Event::Activity(crate::development::Activity::Patch {
                                    id: id.into(),
                                    changes: params["changes"].clone(),
                                }));
                            }
                        }
                        // Recoverable errors are followed by retries; terminal errors by turn/completed.
                        "error" => emit(Event::Notice(
                            params["error"]["message"]
                                .as_str()
                                .unwrap_or("Codex 正在重试连接")
                                .into(),
                        )),
                        _ => {}
                    }
                } else if let Some(id) = message["id"].as_u64()
                    && let Some((method, _)) = pending.remove(&id)
                {
                    // An earlier turn can finish before its acknowledgement, even after a new turn starts.
                    if method == "turn/start" && active_request != Some(id) {
                        continue;
                    }
                    if method == "command/exec" {
                        if let Some((test, started, cancelled)) = running_test.take() {
                            let exit_code = message["result"]["exitCode"].as_i64();
                            let error = message
                                .get("error")
                                .and_then(|e| e["message"].as_str())
                                .unwrap_or(if exit_code.is_none() {
                                    "测试结果缺少退出码。"
                                } else {
                                    ""
                                });
                            emit(Event::TestResult(Box::new(
                                crate::development::testing::TestRun {
                                    id: test.id,
                                    definition_id: test.definition.id,
                                    name: test.definition.name,
                                    command: test.definition.command,
                                    duration_ms: started.elapsed().as_millis().min(u64::MAX as u128)
                                        as u64,
                                    status: if cancelled {
                                        "cancelled"
                                    } else if exit_code == Some(0) && error.is_empty() {
                                        "passed"
                                    } else {
                                        "failed"
                                    }
                                    .into(),
                                    exit_code,
                                    stdout: message["result"]["stdout"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .into(),
                                    stderr: message["result"]["stderr"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .into(),
                                    error: error.into(),
                                    ..Default::default()
                                },
                            )));
                        }
                        continue;
                    }
                    if let Some(error) = message.get("error") {
                        let error = error["message"]
                            .as_str()
                            .unwrap_or("Codex 请求失败")
                            .to_owned();
                        match method {
                            "turn/start" => {
                                session = before_turn.take();
                                active = false;
                                active_request = None;
                                turn_id = None;
                                interrupt = false;
                                emit(Event::Failed(error));
                            }
                            "turn/interrupt" | "command/exec/terminate" => {
                                emit(Event::Notice(error))
                            }
                            _ => return Err(error),
                        }
                        continue;
                    }
                    match method {
                        "initialize" => {
                            write_message(writer, json!({"method":"initialized"}))?;
                            if structured {
                                request(
                                    writer,
                                    &mut pending,
                                    &mut next,
                                    "config/read",
                                    json!({"cwd":directory,"includeLayers":false}),
                                )?;
                                continue;
                            }
                            let mut params = json!({"cwd":directory});
                            if let Some(instructions) = &instructions {
                                params["developerInstructions"] = instructions.as_str().into();
                            }
                            let method = if let Some(id) = &resume {
                                params["threadId"] = id.clone().into();
                                "thread/resume"
                            } else {
                                "thread/start"
                            };
                            request(writer, &mut pending, &mut next, method, params)?;
                            if let Some(session) = &session {
                                emit(Event::Context(Box::new(session.initial_plan())));
                            }
                        }
                        "config/read" if structured => {
                            let mut disabled = serde_json::Map::new();
                            if let Some(servers) =
                                message["result"]["config"]["mcp_servers"].as_object()
                            {
                                for name in servers.keys() {
                                    disabled.insert(
                                        name.clone(),
                                        json!({"enabled":false,"required":false}),
                                    );
                                }
                            }
                            request(
                                writer,
                                &mut pending,
                                &mut next,
                                "thread/start",
                                json!({"cwd":directory,"developerInstructions":instructions,"ephemeral":true,"sandbox":"read-only","approvalPolicy":"never","config":{"mcp_servers":disabled}}),
                            )?;
                        }
                        "thread/start" | "thread/resume" => {
                            // turn/start overrides persist, so retain the local baseline to restore it.
                            original_approval_policy = message["result"]
                                .get("approvalPolicy")
                                .filter(|p| !p.is_null())
                                .cloned()
                                .ok_or("Codex 未返回审批策略，请检查 CLI 版本。")?;
                            thread_id = Some(
                                message["result"]["thread"]["id"]
                                    .as_str()
                                    .ok_or("Codex 未返回会话编号")?
                                    .into(),
                            );
                            emit(Event::ThreadReady(thread_id.clone().unwrap()));
                            emit(Event::Ready);
                        }
                        "turn/start" => {
                            turn_id = Some(
                                message["result"]["turn"]["id"]
                                    .as_str()
                                    .ok_or("Codex 未返回任务编号")?
                                    .into(),
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
impl Client {
    pub(crate) fn mock(mode: &str) -> Self {
        Self::mock_with_instructions(mode, None)
    }
    pub(crate) fn mock_events(events: Vec<Event>, secrets: Vec<String>) -> Self {
        let mut client = Self::mock("normal");
        let (sender, receiver) = mpsc::channel();
        for event in events {
            sender.send(event).unwrap();
        }
        client.events = receiver;
        client.secrets = secrets;
        client
    }
    pub(crate) fn mock_with_instructions(mode: &str, instructions: Option<String>) -> Self {
        Self::mock_session(mode, instructions, None)
    }
    pub(crate) fn mock_session(
        mode: &str,
        instructions: Option<String>,
        session: Option<crate::roleplay::RoleplaySession>,
    ) -> Self {
        Self::mock_options(mode, instructions, session, None)
    }
    pub(crate) fn mock_structured(mode: &str) -> Self {
        Self::mock_options(
            mode,
            Some(crate::roleplay::character_memory::INSTRUCTIONS.into()),
            None,
            Some(crate::roleplay::character_memory::output_schema()),
        )
    }
    fn mock_options(
        mode: &str,
        instructions: Option<String>,
        session: Option<crate::roleplay::RoleplaySession>,
        schema: Option<Value>,
    ) -> Self {
        let instructions = instructions.or_else(|| {
            session
                .as_ref()
                .map(|session| session.initial_plan().text(true))
        });
        let mut command = Command::new(
            std::env::var_os("LEXINGTON_TEST_PYTHON")
                .unwrap_or_else(|| if cfg!(windows) { "python" } else { "python3" }.into()),
        );
        command
            .args([
                "-X",
                "utf8",
                "-u",
                "-c",
                include_str!("../../tests/fixtures/codex-server.py"),
                mode,
            ])
            .current_dir(std::env::temp_dir().canonicalize().unwrap());
        Self::launch_options(command, instructions, session, || {}, schema, None).unwrap()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_background_threads_use_schema_disable_inherited_mcp_and_reject_interactions() {
        let source = crate::roleplay::character_memory::Extraction::new(
            "a".into(),
            "我喜欢温热的茶",
            "收到。",
        );
        let client = Client::mock_structured("memory-success");
        client.submit(source.prompt(), false).unwrap();
        let event = wait_for(&client, |event| {
            matches!(event, Event::Text { replace: true, .. })
        });
        assert!(
            matches!(event,Event::Text {text,..} if serde_json::from_str::<Value>(&text).unwrap()["events"][0]["title"]=="温茶偏好")
        );
        wait_for(&client, |event| {
            matches!(event, Event::Finished { interrupted: false })
        });
        let blocked = Client::mock_structured("memory-interactive");
        blocked.submit(source.prompt(), false).unwrap();
        let event = loop {
            let event = blocked.events.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(!matches!(event, Event::Prompt(_)));
            if matches!(event, Event::Stopped(_)) {
                break event;
            }
        };
        assert!(matches!(event,Event::Stopped(text) if text.contains("已停止")));
    }

    #[test]
    fn context_preview_and_aborted_turns_do_not_mutate_future_world_state() {
        let root =
            std::env::temp_dir().join(format!("lexington-abort-memory-{}", rand::random::<u64>()));
        let mut card = crate::roleplay::RoleCard::default();
        card.vector_memory.enabled = true;
        card.vector_memory.source = crate::roleplay::EmbeddingSource::Remote;
        card.vector_memory.model = "fixture".into();
        card.vector_memory.base_url = "http://127.0.0.1:1/v1".into();
        card.context.scan_messages = 1;
        card.world_book.push(crate::roleplay::WorldEntry {
            keywords: "interrupt,fail".into(),
            content: "activated background".into(),
            sticky_turns: 3,
            ..Default::default()
        });
        let client = Client::mock_session(
            "context",
            None,
            Some(crate::roleplay::RoleplaySession::new(card).with_memory(&root, "a")),
        );
        client.preview_context("interrupt".into()).unwrap();
        let event = wait_for(
            &client,
            |event| matches!(event, Event::Context(plan) if plan.preview),
        );
        assert!(
            matches!(event, Event::Context(plan) if plan.turn == 1 && plan.text(false).contains("activated background"))
        );
        client.submit("interrupt".into(), false).unwrap();
        wait_for(&client, |event| matches!(event, Event::Text { .. }));
        client.submit("interrupt".into(), false).unwrap(); // Rejected while active.
        client.interrupt();
        wait_for(&client, |event| {
            matches!(event, Event::Finished { interrupted: true })
        });
        for fail in [false, true] {
            if fail {
                client.submit("fail".into(), false).unwrap();
                wait_for(&client, |event| matches!(event, Event::Failed(_)));
            }
            client.preview_context("ordinary".into()).unwrap();
            let event = wait_for(
                &client,
                |event| matches!(event, Event::Context(plan) if plan.preview),
            );
            assert!(
                matches!(event, Event::Context(plan) if plan.turn == 1 && !plan.text(false).contains("activated background"))
            );
        }
        client.submit("persona-status".into(), false).unwrap();
        let event = wait_for(&client, |event| {
            matches!(event, Event::Text { replace: true, .. })
        });
        assert!(matches!(event, Event::Text {text, ..} if text == "turn-has-no-persona:3"));
        wait_for(&client, |event| matches!(event, Event::Finished { .. }));
        drop(client);
        let store = crate::tools::vector_store::VectorStore::open(
            &root.join("config/roleplay-vectors.sqlite3"),
        )
        .unwrap();
        let documents = store.pending("a", "fixture-inspection", 8).unwrap();
        assert_eq!(documents.len(), 2);
        assert!(
            documents
                .iter()
                .all(|doc| !doc.text.contains("interrupt") && !doc.text.contains("fail"))
        );
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn role_card_world_book_is_separate_from_the_original_message_on_the_wire() {
        let mut card = crate::roleplay::RoleCard::default();
        card.context.scan_messages = 1;
        card.world_book = vec![crate::roleplay::WorldEntry {
            keywords: "world-match".into(),
            content: "匹配的背景".into(),
            ..Default::default()
        }];
        let client = Client::mock_session(
            "context",
            None,
            Some(crate::roleplay::RoleplaySession::new(card)),
        );
        for (message, count) in [("world-match", 2), ("world-miss", 1)] {
            client.submit(message.into(), false).unwrap();
            let event = wait_for(&client, |event| {
                matches!(event, Event::Text { replace: true, .. })
            });
            let Event::Text { text, .. } = event else {
                unreachable!()
            };
            let input: Vec<Value> = serde_json::from_str(&text).unwrap();
            assert_eq!(input.len(), count);
            assert_eq!(input[0]["text"], message);
            if count == 2 {
                assert!(input[1]["text"].as_str().unwrap().contains("匹配的背景"));
            }
            wait_for(&client, |event| matches!(event, Event::Finished { .. }));
        }
    }

    #[test]
    fn character_persona_is_sent_once_on_thread_start_and_turns_reuse_it() {
        let root = std::env::temp_dir().join(format!(
            "lexington-protocol-persona-{}",
            rand::random::<u64>()
        ));
        let directory = root.join("resource/agents");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("characters.json"),
            r#"{"characters":{"a":"a.md"}}"#,
        )
        .unwrap();
        std::fs::write(directory.join("a.md"), "角色甲\n只属于甲").unwrap();
        for (character, mode) in [("a", "persona"), ("unconfigured", "normal")] {
            let instructions = crate::roleplay::agent_instructions(&root, character).unwrap();
            let client = Client::mock_with_instructions(mode, instructions);
            for round in 1..=2 {
                client.submit("persona-status".into(), false).unwrap();
                let event = wait_for(&client, |event| {
                    matches!(event, Event::Text { replace: true, .. })
                });
                assert!(
                    matches!(event, Event::Text { text, .. } if text == format!("turn-has-no-persona:{round}"))
                );
                wait_for(&client, |event| matches!(event, Event::Finished { .. }));
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn structured_background_clients_cannot_run_standalone_tests() {
        let client = Client::mock_structured("memory-success");
        client
            .run_test(crate::development::testing::TestRequest {
                id: 1,
                definition: crate::development::testing::TestDefinition {
                    id: 1,
                    name: "fixture".into(),
                    command: vec!["fixture-test".into(), "pass".into()],
                    timeout_seconds: 10,
                },
            })
            .unwrap();
        let Event::TestResult(result) =
            wait_for(&client, |event| matches!(event, Event::TestResult(_)))
        else {
            unreachable!()
        };
        assert_eq!(result.status, "failed");
        assert!(result.exit_code.is_none());
    }

    #[test]
    fn standalone_tests_use_the_existing_sandbox_channel_without_model_turns_and_cancel() {
        use crate::development::testing::{TestDefinition, TestRequest};
        let client = Client::mock("normal");
        for (id, argument, status) in [
            (1, "pass", "passed"),
            (2, "fail", "failed"),
            (3, "error", "failed"),
            (4, "wait", "cancelled"),
        ] {
            client
                .run_test(TestRequest {
                    id,
                    definition: TestDefinition {
                        id: 11,
                        name: "fixture".into(),
                        command: vec![
                            "fixture-test".into(),
                            argument.into(),
                            "literal with spaces".into(),
                        ],
                        timeout_seconds: 60,
                    },
                })
                .unwrap();
            if argument == "wait" {
                std::thread::sleep(Duration::from_millis(40));
                client.interrupt();
            }
            let event = wait_for(&client, |event| matches!(event, Event::TestResult(_)));
            let Event::TestResult(run) = event else {
                unreachable!()
            };
            assert_eq!(run.id, id);
            assert_eq!(run.status, status);
            if argument == "pass" {
                assert_eq!(run.exit_code, Some(0));
                assert!(run.stdout.contains("literal with spaces"));
            }
        }
        client.submit("resume-status".into(), false).unwrap();
        let Event::Text { text, .. } = wait_for(&client, |event| {
            matches!(event, Event::Text { replace: true, .. })
        }) else {
            unreachable!()
        };
        assert!(text.starts_with("thread/start:"));
        wait_for(&client, |event| matches!(event, Event::Finished { .. }));
    }

    #[test]
    fn inherited_launch_has_no_model_or_provider_overrides() {
        let root = tempfile::tempdir().unwrap();
        let config = CodexConfig {
            executable: "fixture-only.exe".into(),
            ..Default::default()
        };
        let command = Client::command(&config, root.path()).unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["app-server", "--stdio"]
        );
        assert!(
            !command
                .get_envs()
                .any(|(name, _)| name == "LEXINGTON_MODEL_API_KEY")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_rejects_shell_launchers_before_starting_a_process() {
        for executable in [
            "codex.cmd",
            "CODEX.BAT",
            r"C:\Program Files\Codex\codex.CmD",
        ] {
            let config = CodexConfig {
                executable: executable.into(),
                ..Default::default()
            };
            assert!(Client::command(&config, &std::env::temp_dir()).is_err());
        }
    }

    fn wait_for(client: &Client, matches: impl Fn(&Event) -> bool) -> Event {
        loop {
            let event = client
                .events
                .recv_timeout(Duration::from_secs(5))
                .expect("Codex event timeout");
            if matches(&event) {
                return event;
            }
            assert!(
                !matches!(event, Event::Stopped(_)),
                "unexpected disconnect: {event:?}"
            );
        }
    }
    #[test]
    fn no_approval_applies_per_turn_and_restores_original_policy_without_restarting() {
        for mode in ["normal", "granular"] {
            let client = Client::mock(mode);
            // The first input can request the mode even before startup completes.
            for (text, enabled) in [
                ("no-approval", true),
                ("restore", false),
                ("no-approval-again", true),
            ] {
                client.submit(text.into(), enabled).unwrap();
                wait_for(&client, |e| {
                    matches!(e, Event::Finished { interrupted: false })
                });
            }
        }
    }
    #[test]
    fn startup_queues_input_streams_unicode_and_reuses_thread_without_duplicate_final_text() {
        let client = Client::mock("normal");
        // Submit before Ready, as a user can do immediately after launching the app.
        client
            .submit("你好 \"提督\"\n第二行".into(), false)
            .unwrap();
        wait_for(&client, |e| matches!(e, Event::Ready));
        for (turn, text) in [
            (1, "你好 \"提督\"\n第二行"),
            (2, "fast"),
            (3, "late-ack"),
            (4, "第四次"),
        ] {
            if turn != 1 {
                client.submit(text.into(), false).unwrap();
            }
            let mut reply = String::new();
            loop {
                match client.events.recv_timeout(Duration::from_secs(5)).unwrap() {
                    Event::Text { text, replace, .. } => {
                        if replace {
                            reply = text;
                        } else {
                            reply.push_str(&text);
                        }
                    }
                    Event::Finished { interrupted: false } => break,
                    event => panic!("unexpected event: {event:?}"),
                }
            }
            assert_eq!(reply, format!("第{turn}轮：{text}"));
        }
        let child = client.child.clone();
        drop(client);
        assert!(
            child.lock().unwrap().child.try_wait().unwrap().is_some(),
            "exiting must reap the child process"
        );
    }
    #[test]
    fn exit_cleans_descendants_holding_stdout_without_hanging() {
        let client = Client::mock("normal");
        wait_for(&client, |e| matches!(e, Event::Ready));
        client.submit("descendant".into(), false).unwrap();
        let Event::Text { text, .. } = wait_for(&client, |e| matches!(e, Event::Text { .. }))
        else {
            unreachable!()
        };
        let pid: i32 = text.parse().unwrap();
        wait_for(&client, |e| matches!(e, Event::Finished { .. }));
        let (done, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            drop(client);
            let _ = done.send(());
        });
        let result = receiver.recv_timeout(Duration::from_secs(5));
        if result.is_err() {
            // Release the inherited pipe even when the regression returns, so the test can fail.
            #[cfg(unix)]
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            #[cfg(windows)]
            unsafe {
                use windows_sys::Win32::System::Threading::*;
                if let Ok(process) = crate::tools::windows::Handle::new(OpenProcess(
                    PROCESS_TERMINATE,
                    0,
                    pid as u32,
                )) {
                    TerminateProcess(process.0, 1);
                }
            }
        }
        worker.join().unwrap();
        assert!(
            result.is_ok(),
            "a launcher descendant retained stdout and blocked shutdown"
        );
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::*};
            if let Ok(process) =
                crate::tools::windows::Handle::new(OpenProcess(PROCESS_SYNCHRONIZE, 0, pid as u32))
            {
                assert_eq!(WaitForSingleObject(process.0, 1000), WAIT_OBJECT_0);
            }
        }
        #[cfg(target_os = "linux")]
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            assert_eq!(
                stat.split_once(") ").unwrap().1.chars().next(),
                Some('Z'),
                "descendant is still running"
            );
        }
    }
    #[test]
    fn approvals_questions_and_unsupported_requests_do_not_hang_turns() {
        let client = Client::mock("normal");
        wait_for(&client, |e| matches!(e, Event::Ready));
        client.submit("approve".into(), false).unwrap();
        let Event::Prompt(prompt) = wait_for(&client, |e| matches!(e, Event::Prompt(_))) else {
            unreachable!()
        };
        assert!(prompt.details.contains("cargo test"));
        assert!(prompt.questions.is_empty());
        client.reply(prompt.id, json!({"decision":"decline"}));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        client.submit("file-approval".into(), false).unwrap();
        let Event::Prompt(prompt) = wait_for(&client, |e| matches!(e, Event::Prompt(_))) else {
            unreachable!()
        };
        assert!(prompt.details.contains("example.txt"));
        assert!(prompt.details.contains("-before\n+after"));
        client.reply(prompt.id, json!({"decision":"decline"}));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        client.submit("question".into(), false).unwrap();
        let Event::Prompt(prompt) = wait_for(&client, |e| matches!(e, Event::Prompt(_))) else {
            unreachable!()
        };
        assert_eq!(prompt.questions[0].text, "选择方案");
        assert_eq!(prompt.questions[0].options, ["A", "B"]);
        client.reply(prompt.id, json!({"answers":{"choice":{"answers":["A"]}}}));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        client.submit("unsupported".into(), false).unwrap();
        wait_for(&client, |e| matches!(e, Event::Notice(_)));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
    }
    #[test]
    fn interrupt_and_turn_error_allow_the_next_message_and_crashes_are_reported() {
        let client = Client::mock("normal");
        wait_for(&client, |e| matches!(e, Event::Ready));
        client.submit("interrupt".into(), false).unwrap();
        wait_for(&client, |e| matches!(e, Event::Text { .. }));
        client.interrupt();
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: true })
        });
        client.submit("fail".into(), false).unwrap();
        let Event::Failed(error) = wait_for(&client, |e| matches!(e, Event::Failed(_))) else {
            unreachable!()
        };
        assert_eq!(error, "模拟请求失败");
        client.submit("恢复".into(), false).unwrap();
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        let crashed = Client::mock("crash");
        wait_for(&crashed, |e| matches!(e, Event::Ready));
        crashed.submit("hello".into(), false).unwrap();
        wait_for(&crashed, |e| matches!(e, Event::Stopped(_)));
        let rejected = Client::mock("startup-error");
        let Event::Stopped(error) = wait_for(&rejected, |e| matches!(e, Event::Stopped(_))) else {
            unreachable!()
        };
        assert_eq!(error, "请先登录 Codex");
    }
}
