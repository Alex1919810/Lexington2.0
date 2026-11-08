//! Roleplay policy and embedding requests; the generic SQLite store lives in tools.
use crate::tools::vector_store::{Document, SearchHit, VectorStore};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VectorMemorySettings {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub recall_limit: usize,
    pub max_distance: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for VectorMemorySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: String::new(),
            api_key_env: String::new(),
            recall_limit: 3,
            max_distance: 0.35,
            extra: Default::default(),
        }
    }
}
impl VectorMemorySettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=8).contains(&self.recall_limit)
            || !self.max_distance.is_finite()
            || !(0.0..=2.0).contains(&self.max_distance)
        {
            return Err("记忆召回数量需为 1～8 条，余弦距离阈值需为 0～2。".into());
        }
        if !self.enabled {
            return Ok(());
        }
        let url = url::Url::parse(self.base_url.trim()).map_err(|_| "Embedding 基础地址无效。")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("Embedding 需使用不含凭据、查询和片段的 HTTP(S) 基础地址。".into());
        }
        if self.model.trim().is_empty()
            || self.model.len() > 200
            || self.model.contains(['\r', '\n'])
        {
            return Err("请填写 Embedding 模型名称（最多 200 字节）。".into());
        }
        if self.api_key_env.len() > 128
            || !self
                .api_key_env
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || self
                .api_key_env
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_digit)
        {
            return Err("请填写 API Key 的环境变量名称，而非密钥本身；本地无鉴权可留空。".into());
        }
        Ok(())
    }
    fn space(&self) -> String {
        serde_json::json!([
            self.base_url.trim().trim_end_matches('/'),
            self.model.trim()
        ])
        .to_string()
    }
}

#[derive(Clone)]
pub(super) struct VectorMemory {
    path: PathBuf,
    character: String,
    pub settings: VectorMemorySettings,
}
pub fn clear_vector_memory(root: &Path, character: &str) -> Result<(), String> {
    let path = database_path(root);
    if !path.exists() {
        return Ok(());
    }
    VectorStore::open(&path)?.delete(character, None)
}
fn database_path(root: &Path) -> PathBuf {
    root.join("config/roleplay-vectors.sqlite3")
}
impl VectorMemory {
    pub fn new(root: &Path, character: &str, settings: VectorMemorySettings) -> Self {
        Self {
            path: database_path(root),
            character: character.into(),
            settings,
        }
    }
    pub fn recall(&self, message: &str) -> Result<Vec<SearchHit>, String> {
        self.settings.validate()?;
        if !self.settings.enabled || message.trim().is_empty() {
            return Ok(vec![]);
        }
        let store = VectorStore::open(&self.path)?;
        let space = self.settings.space();
        let pending = store.pending(&self.character, &space, 8)?;
        // Avoid contacting an embedding service before any successful conversation exists.
        if pending.is_empty() && store.pending(&self.character, "", 1)?.is_empty() {
            return Ok(vec![]);
        }
        let query: String = message.chars().take(2000).collect();
        let inputs: Vec<&str> = pending
            .iter()
            .map(|document| document.text.as_str())
            .chain(std::iter::once(query.as_str()))
            .collect();
        let vectors = embed(&self.settings, &inputs)?;
        for (document, vector) in pending.iter().zip(&vectors) {
            store.set_embedding(&self.character, document, &space, vector)?;
        }
        store.search(
            &self.character,
            &space,
            vectors.last().unwrap(),
            self.settings.recall_limit,
            self.settings.max_distance,
        )
    }
    pub fn remember(&self, user: &str, assistant: &str) -> Result<(), String> {
        if !self.settings.enabled {
            return Ok(());
        }
        let store = VectorStore::open(&self.path)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let turn = format!("{:016x}", rand::random::<u64>());
        for (role, text) in [("用户", user), ("角色", assistant)] {
            let chars: Vec<char> = text.chars().take(8192).collect();
            for (index, chunk) in chars.chunks(1000).enumerate() {
                let body: String = chunk.iter().collect();
                if body.trim().is_empty() {
                    continue;
                }
                store.put(
                    &self.character,
                    &Document {
                        id: format!("{turn}:{role}:{index}"),
                        text: format!("{role}：{body}"),
                        created_at: now,
                    },
                )?;
            }
        }
        // Bound per-character storage. Deleting a row deletes its vector in the same operation.
        store.prune(&self.character, 2000)
    }
}

fn embed(settings: &VectorMemorySettings, inputs: &[&str]) -> Result<Vec<Vec<f32>>, String> {
    let key = if settings.api_key_env.is_empty() {
        None
    } else {
        Some(
            std::env::var(&settings.api_key_env)
                .map_err(|_| "Embedding API Key 环境变量未设置；本地无鉴权请清空变量名称。")?,
        )
    };
    if key
        .as_ref()
        .is_some_and(|key| key.trim().is_empty() || key.contains(['\r', '\n']))
    {
        return Err("Embedding API Key 环境变量内容无效。".into());
    }
    let agent =
        ureq::AgentBuilder::new()
            // Local embedding servers must not be sent through an inherited proxy.
            .try_proxy_from_env(!url::Url::parse(&settings.base_url).is_ok_and(|url| {
                matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
            }))
            .timeout(Duration::from_secs(3))
            .redirects(0)
            .build();
    let endpoint = format!(
        "{}/embeddings",
        settings.base_url.trim().trim_end_matches('/')
    );
    let mut request = agent
        .post(&endpoint)
        .set("Content-Type", "application/json");
    if let Some(key) = &key {
        request = request.set("Authorization", &format!("Bearer {key}"));
    }
    let response = request.send_string(&serde_json::json!({"model":settings.model.trim(),"input":inputs,"encoding_format":"float"}).to_string())
        .map_err(|_| "Embedding 请求失败或超时；本轮继续对话，未召回历史记忆。")?;
    let mut bytes = vec![];
    response
        .into_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取 Embedding 响应。")?;
    if bytes.len() > 1024 * 1024 {
        return Err("Embedding 响应超过 1 MiB。".into());
    }
    #[derive(Deserialize)]
    struct Response {
        data: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        index: usize,
        embedding: Vec<f32>,
    }
    let response: Response =
        serde_json::from_slice(&bytes).map_err(|_| "Embedding 响应格式错误。")?;
    let mut vectors = vec![None; inputs.len()];
    for item in response.data {
        if item.index >= vectors.len()
            || vectors[item.index].is_some()
            || item.embedding.is_empty()
            || item.embedding.len() > 4096
            || item.embedding.iter().any(|v| !v.is_finite())
            || item.embedding.iter().all(|v| *v == 0.)
        {
            return Err("Embedding 响应的索引或向量无效。".into());
        }
        vectors[item.index] = Some(item.embedding);
    }
    let vectors: Vec<Vec<f32>> = vectors
        .into_iter()
        .collect::<Option<_>>()
        .ok_or("Embedding 响应缺少向量。")?;
    if vectors
        .iter()
        .any(|vector| vector.len() != vectors[0].len())
    {
        return Err("Embedding 响应向量维度不一致。".into());
    }
    Ok(vectors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roleplay::{RoleCard, RoleplaySession};
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
        time::Instant,
    };

    fn fixture(
        responses: Vec<Option<Value>>,
    ) -> (String, mpsc::Receiver<Value>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let (send, receive) = mpsc::channel();
        let worker = thread::spawn(move || {
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(8);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("fixture accept: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert!(line.starts_with("POST /v1/embeddings "));
                let mut length = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("Content-Length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                }
                assert!(length < 100000);
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).unwrap();
                let request: Value = serde_json::from_slice(&bytes).unwrap();
                let body = response
                    .unwrap_or_else(|| {
                        let data: Vec<_> = request["input"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .enumerate()
                            .rev()
                            .map(|(index, text)| {
                                let vector = if text.as_str().unwrap().contains('茶') {
                                    vec![1., 0.]
                                } else {
                                    vec![0., 1.]
                                };
                                json!({"index":index,"embedding":vector})
                            })
                            .collect();
                        json!({"data":data})
                    })
                    .to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
                send.send(request).unwrap();
            }
        });
        (base, receive, worker)
    }

    #[test]
    fn history_recall_survives_restart_isolates_roles_and_models_and_obeys_budget() {
        let root = std::env::temp_dir().join(format!("lexington-recall-{}", rand::random::<u64>()));
        let (base, requests, server) = fixture(vec![None, None, None, None]);
        let mut card = RoleCard {
            vector_memory: VectorMemorySettings {
                enabled: true,
                base_url: base,
                model: "fixture".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut session = RoleplaySession::new(card.clone()).with_memory(&root, "a");
        assert!(session.preview("茶").preview);
        assert!(!database_path(&root).exists());
        session.begin("去年你送给我红茶");
        session.finish("那是我们一起挑选的茶。");
        assert!(session.preview("茶").text(false).is_empty());
        assert!(requests.try_recv().is_err());
        let mut restarted = RoleplaySession::new(card.clone()).with_memory(&root, "a");
        let plan = restarted.begin("想喝茶");
        assert!(plan.text(false).contains("去年你送给我红茶"));
        assert!(plan.text(false).contains("不视为已确认事实"));
        assert!(!plan.text(true).contains("红茶"));
        assert!(plan.turn_used <= plan.turn_budget);
        let first = requests.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(first["input"].as_array().unwrap().len(), 3);
        card.context.turn_budget = 0;
        let plan = RoleplaySession::new(card.clone())
            .with_memory(&root, "a")
            .begin("茶");
        assert!(plan.text(false).is_empty());
        assert!(
            plan.blocks
                .iter()
                .any(|b| b.category == "历史记忆" && b.reason.contains("超过本轮字符预算"))
        );
        requests.recv_timeout(Duration::from_secs(2)).unwrap();
        let other = VectorMemory::new(&root, "b", card.vector_memory.clone());
        assert!(other.recall("coffee").unwrap().is_empty());
        other.remember("coffee preference", "coffee reply").unwrap();
        let hits = other.recall("coffee").unwrap();
        assert!(hits.iter().all(|hit| !hit.document.text.contains("茶")));
        requests.recv_timeout(Duration::from_secs(2)).unwrap();
        card.vector_memory.model = "fixture2".into();
        let changed = VectorMemory::new(&root, "a", card.vector_memory);
        assert!(!changed.recall("茶").unwrap().is_empty());
        let reindex = requests.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(reindex["model"], "fixture2");
        assert_eq!(reindex["input"].as_array().unwrap().len(), 3);
        clear_vector_memory(&root, "a").unwrap();
        assert!(changed.recall("茶").unwrap().is_empty());
        let store = VectorStore::open(&database_path(&root)).unwrap();
        assert_eq!(store.pending("b", "different", 8).unwrap().len(), 2);
        drop(store);
        server.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disabled_memory_is_inert_and_invalid_embedding_data_is_rejected_without_echo() {
        let root = std::env::temp_dir().join(format!("lexington-inert-{}", rand::random::<u64>()));
        let disabled = VectorMemory::new(&root, "a", Default::default());
        disabled.remember("user", "assistant").unwrap();
        assert!(disabled.recall("query").unwrap().is_empty());
        assert!(!root.exists());
        let bad = vec![
            Some(json!({"error":"fixture-private-error"})),
            Some(json!({"data":[]})),
            Some(json!({"data":[{"index":0,"embedding":[1.]},{"index":0,"embedding":[1.]}]})),
            Some(json!({"data":[{"index":1,"embedding":[1.]}]})),
            Some(json!({"data":[{"index":0,"embedding":[0.,0.]}]})),
        ];
        let count = bad.len();
        let (base, requests, server) = fixture(bad);
        let mut settings = VectorMemorySettings {
            enabled: true,
            base_url: base,
            model: "fixture".into(),
            ..Default::default()
        };
        for _ in 0..count {
            let error = embed(&settings, &["fixture input"]).err().unwrap();
            assert!(!error.contains("fixture-private-error"));
            requests.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        server.join().unwrap();
        settings.base_url = "http://user:fixture-secret@example.invalid/v1".into();
        assert!(!settings.validate().unwrap_err().contains("fixture-secret"));
        settings.base_url = "http://127.0.0.1:1/v1".into();
        settings.api_key_env = "not-a-key-value".into();
        assert!(settings.validate().is_err());
    }
}
