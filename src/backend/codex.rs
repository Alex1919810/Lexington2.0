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
    Ready,
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
    Submit(String),
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
}
struct Process {
    child: Child,
    stopped: bool,
}
impl Process {
    fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
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
    pub fn start(
        config: &CodexConfig,
        root: &Path,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
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
        let mut executable = std::path::PathBuf::from(&config.executable);
        // Desktop launchers often omit the user's CLI installation directory from PATH.
        if config.executable == "codex" {
            let in_path = std::env::var_os("PATH").is_some_and(|paths| {
                std::env::split_paths(&paths).any(|p| {
                    p.join(if cfg!(windows) { "codex.exe" } else { "codex" })
                        .is_file()
                })
            });
            if !in_path
                && let Some(home) =
                    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
            {
                for suffix in [".local/bin/codex", ".cargo/bin/codex"] {
                    let path = Path::new(&home).join(suffix);
                    if path.is_file() {
                        executable = path;
                        break;
                    }
                }
            }
        }
        let mut command = Command::new(executable);
        command
            .args(["app-server", "--stdio"])
            .current_dir(directory);
        Self::launch(command, wake)
    }
    fn launch(mut command: Command, wake: impl Fn() + Send + 'static) -> Result<Self, String> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
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
                format!("无法启动 Codex：{e}。请安装并登录 Codex，或配置 codex.executable。")
            })?;
        let stdout = child.stdout.take().expect("piped stdout");
        let mut stdin = child.stdin.take().expect("piped stdin");
        let child = Arc::new(Mutex::new(Process {
            child,
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
            let emit = |event| {
                let _ = events.send(event);
                wake();
            };
            let result = communicate(&mut stdin, &directory, receiver, &emit);
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
        })
    }
    pub fn submit(&self, text: String) -> Result<(), String> {
        self.input
            .send(Input::Submit(text))
            .map_err(|_| "Codex 已停止，请在设置中重启会话。".into())
    }
    pub fn interrupt(&self) {
        let _ = self.input.send(Input::Interrupt);
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
    receiver: mpsc::Receiver<Input>,
    emit: &impl Fn(Event),
) -> Result<(), String> {
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
    let mut turn_id: Option<String> = None;
    let mut active_request = None;
    let mut approval_items = HashMap::<String, Value>::new();
    let mut queued = None;
    let mut active = false;
    let mut interrupt = false;
    loop {
        if pending
            .values()
            .any(|(_, started)| started.elapsed() > Duration::from_secs(30))
        {
            return Err("Codex 连接超时，请检查登录状态并在设置中重启会话。".into());
        }
        if let Some(thread) = &thread_id {
            if !active && let Some(text) = queued.take() {
                if interrupt {
                    interrupt = false;
                    emit(Event::Finished { interrupted: true });
                } else {
                    active_request = Some(next);
                    approval_items.clear();
                    request(
                        writer,
                        &mut pending,
                        &mut next,
                        "turn/start",
                        json!({"threadId":thread,"input":[{"type":"text","text":text}]}),
                    )?;
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
            Input::Shutdown => return Ok(()),
            Input::End(error) => return Err(error),
            Input::Submit(text) => {
                if active || queued.is_some() {
                    emit(Event::Notice("Lexington正在思考，请稍候。".into()));
                } else {
                    queued = Some(text);
                }
            }
            Input::Interrupt => {
                if active || queued.is_some() {
                    interrupt = true;
                }
            }
            Input::Reply(id, result) => write_message(writer, json!({"id":id,"result":result}))?,
            Input::Incoming(message) => {
                if let Some(method) = message["method"].as_str() {
                    let params = &message["params"];
                    if let Some(id) = message.get("id") {
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
                            let item = &params["item"];
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
                            let item = &params["item"];
                            if item["type"] == "agentMessage"
                                && let (Some(id), Some(text)) =
                                    (item["id"].as_str(), item["text"].as_str())
                            {
                                emit(Event::Text {
                                    id: id.into(),
                                    text: text.into(),
                                    replace: true,
                                });
                            }
                        }
                        "turn/completed" => {
                            if turn_id
                                .as_deref()
                                .is_some_and(|id| params["turn"]["id"].as_str() != Some(id))
                            {
                                continue;
                            }
                            for item in params["turn"]["items"].as_array().into_iter().flatten() {
                                if item["type"] == "agentMessage"
                                    && let (Some(id), Some(text)) =
                                        (item["id"].as_str(), item["text"].as_str())
                                {
                                    emit(Event::Text {
                                        id: id.into(),
                                        text: text.into(),
                                        replace: true,
                                    });
                                }
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
                    if let Some(error) = message.get("error") {
                        let error = error["message"]
                            .as_str()
                            .unwrap_or("Codex 请求失败")
                            .to_owned();
                        match method {
                            "turn/start" => {
                                active = false;
                                active_request = None;
                                turn_id = None;
                                interrupt = false;
                                emit(Event::Failed(error));
                            }
                            "turn/interrupt" => emit(Event::Notice(error)),
                            _ => return Err(error),
                        }
                        continue;
                    }
                    match method {
                        "initialize" => {
                            write_message(writer, json!({"method":"initialized"}))?;
                            request(
                                writer,
                                &mut pending,
                                &mut next,
                                "thread/start",
                                json!({"cwd":directory}),
                            )?;
                        }
                        "thread/start" => {
                            thread_id = Some(
                                message["result"]["thread"]["id"]
                                    .as_str()
                                    .ok_or("Codex 未返回会话编号")?
                                    .into(),
                            );
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
        let mut command = Command::new("python3");
        command
            .args([
                "-u",
                "-c",
                include_str!("../../tests/fixtures/codex-server.py"),
                mode,
            ])
            .current_dir(std::env::temp_dir().canonicalize().unwrap());
        Self::launch(command, || {}).unwrap()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

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
    fn startup_queues_input_streams_unicode_and_reuses_thread_without_duplicate_final_text() {
        let client = Client::mock("normal");
        // Submit before Ready, as a user can do immediately after launching the app.
        client.submit("你好 \"提督\"\n第二行".into()).unwrap();
        wait_for(&client, |e| matches!(e, Event::Ready));
        for (turn, text) in [
            (1, "你好 \"提督\"\n第二行"),
            (2, "fast"),
            (3, "late-ack"),
            (4, "第四次"),
        ] {
            if turn != 1 {
                client.submit(text.into()).unwrap();
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
    #[cfg(unix)]
    fn exit_cleans_descendants_holding_stdout_without_hanging() {
        let client = Client::mock("normal");
        wait_for(&client, |e| matches!(e, Event::Ready));
        client.submit("descendant".into()).unwrap();
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
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
        worker.join().unwrap();
        assert!(
            result.is_ok(),
            "a launcher descendant retained stdout and blocked shutdown"
        );
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
        client.submit("approve".into()).unwrap();
        let Event::Prompt(prompt) = wait_for(&client, |e| matches!(e, Event::Prompt(_))) else {
            unreachable!()
        };
        assert!(prompt.details.contains("cargo test"));
        assert!(prompt.questions.is_empty());
        client.reply(prompt.id, json!({"decision":"decline"}));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        client.submit("file-approval".into()).unwrap();
        let Event::Prompt(prompt) = wait_for(&client, |e| matches!(e, Event::Prompt(_))) else {
            unreachable!()
        };
        assert!(prompt.details.contains("example.txt"));
        assert!(prompt.details.contains("-before\n+after"));
        client.reply(prompt.id, json!({"decision":"decline"}));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        client.submit("question".into()).unwrap();
        let Event::Prompt(prompt) = wait_for(&client, |e| matches!(e, Event::Prompt(_))) else {
            unreachable!()
        };
        assert_eq!(prompt.questions[0].text, "选择方案");
        assert_eq!(prompt.questions[0].options, ["A", "B"]);
        client.reply(prompt.id, json!({"answers":{"choice":{"answers":["A"]}}}));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        client.submit("unsupported".into()).unwrap();
        wait_for(&client, |e| matches!(e, Event::Notice(_)));
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
    }
    #[test]
    fn interrupt_and_turn_error_allow_the_next_message_and_crashes_are_reported() {
        let client = Client::mock("normal");
        wait_for(&client, |e| matches!(e, Event::Ready));
        client.submit("interrupt".into()).unwrap();
        wait_for(&client, |e| matches!(e, Event::Text { .. }));
        client.interrupt();
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: true })
        });
        client.submit("fail".into()).unwrap();
        let Event::Failed(error) = wait_for(&client, |e| matches!(e, Event::Failed(_))) else {
            unreachable!()
        };
        assert_eq!(error, "模拟请求失败");
        client.submit("恢复".into()).unwrap();
        wait_for(&client, |e| {
            matches!(e, Event::Finished { interrupted: false })
        });
        let crashed = Client::mock("crash");
        wait_for(&crashed, |e| matches!(e, Event::Ready));
        crashed.submit("hello".into()).unwrap();
        wait_for(&crashed, |e| matches!(e, Event::Stopped(_)));
        let rejected = Client::mock("startup-error");
        let Event::Stopped(error) = wait_for(&rejected, |e| matches!(e, Event::Stopped(_))) else {
            unreachable!()
        };
        assert_eq!(error, "请先登录 Codex");
    }
}
