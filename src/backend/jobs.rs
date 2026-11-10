//! Bounded structured background jobs. Owns clients and cleanup, never UI state.
use super::{
    codex::{Client, Event},
    config::CodexConfig,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    time::{Duration, Instant},
};

pub struct Request<T> {
    pub owner: String,
    pub payload: T,
    pub instructions: String,
    pub schema: Value,
    pub prompt: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Completed,
    Failed,
    Cancelled,
}

pub struct Outcome<T> {
    pub owner: String,
    pub payload: T,
    pub status: Status,
    pub result: Result<String, String>,
}

struct Queued<T> {
    request: Request<T>,
    connection: CodexConfig,
}
struct Active<T> {
    client: Client,
    owner: String,
    payload: T,
    connection: CodexConfig,
    started: Instant,
    messages: BTreeMap<String, String>,
    final_text: Option<String>,
}

pub struct StructuredJobs<T> {
    queue: VecDeque<Queued<T>>,
    active: Option<Active<T>>,
}
impl<T> Default for StructuredJobs<T> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            active: None,
        }
    }
}
impl<T> StructuredJobs<T> {
    pub fn enqueue(&mut self, request: Request<T>, connection: CodexConfig) -> Result<(), String> {
        if self.queue.len() >= 4 {
            return Err("后台任务队列已满，本轮跳过；正常对话不受影响。".into());
        }
        self.queue.push_back(Queued {
            request,
            connection,
        });
        Ok(())
    }
    pub fn is_running(&self) -> bool {
        self.active.is_some()
    }

    /// Poll with a per-frame budget. A terminal outcome is returned only after
    /// the client has released its process tree and temporary working directory.
    pub fn poll(
        &mut self,
        config: &CodexConfig,
        root: &Path,
        wake: impl Fn() + Send + 'static,
    ) -> Option<Outcome<T>> {
        let mut settled = None;
        if let Some(active) = &mut self.active {
            if active.connection.connection_changed(config) {
                settled = Some((
                    Status::Cancelled,
                    Err("连接设置已变更，后台任务已停止。".into()),
                ));
            } else if active.started.elapsed() >= Duration::from_secs(90) {
                settled = Some((Status::Cancelled, Err("后台任务超时，已停止。".into())));
            } else {
                let frame = Instant::now();
                for _ in 0..64 {
                    if frame.elapsed() >= Duration::from_millis(4) {
                        break;
                    }
                    let Some(event) = active.client.poll() else {
                        break;
                    };
                    match event {
                        Event::Text { id, text, replace } => {
                            if replace {
                                active.final_text = Some(text.clone());
                                active.messages.insert(id, text);
                            } else {
                                active.messages.entry(id).or_default().push_str(&text);
                            }
                            if active.messages.values().map(String::len).sum::<usize>() > 32768 {
                                settled = Some((
                                    Status::Failed,
                                    Err("后台任务结果超过 32 KiB，未接受。".into()),
                                ));
                                break;
                            }
                        }
                        Event::Finished { interrupted: false } => {
                            settled = Some(match active.final_text.take() {
                                Some(text) if !text.trim().is_empty() => {
                                    (Status::Completed, Ok(text))
                                }
                                _ => (Status::Failed, Err("后台任务缺少最终结果。".into())),
                            });
                            break;
                        }
                        Event::Finished { interrupted: true } => {
                            settled = Some((Status::Cancelled, Err("后台任务已中断。".into())));
                            break;
                        }
                        Event::Failed(_) | Event::Stopped(_) | Event::Prompt(_) => {
                            settled =
                                Some((Status::Failed, Err("后台任务失败，未接受结果。".into())));
                            break;
                        }
                        _ => (),
                    }
                }
            }
        }
        if let Some((status, result)) = settled {
            let active = self.active.take().unwrap();
            drop(active.client);
            return Some(Outcome {
                owner: active.owner,
                payload: active.payload,
                status,
                result,
            });
        }
        if self.active.is_none()
            && let Some(queued) = self.queue.pop_front()
        {
            let request = queued.request;
            if queued.connection.connection_changed(config) {
                return Some(Outcome {
                    owner: request.owner,
                    payload: request.payload,
                    status: Status::Cancelled,
                    result: Err("连接设置已变更，已跳过旧连接下排队的任务。".into()),
                });
            }
            let client = match Client::start_structured(
                config,
                root,
                request.instructions,
                request.schema,
                wake,
            ) {
                Ok(client) => client,
                Err(_) => {
                    return Some(Outcome {
                        owner: request.owner,
                        payload: request.payload,
                        status: Status::Failed,
                        result: Err("无法启动后台任务，请检查连接设置。".into()),
                    });
                }
            };
            if client.submit(request.prompt, false).is_err() {
                drop(client);
                return Some(Outcome {
                    owner: request.owner,
                    payload: request.payload,
                    status: Status::Failed,
                    result: Err("无法发送后台任务。".into()),
                });
            }
            self.active = Some(Active {
                client,
                owner: request.owner,
                payload: request.payload,
                connection: queued.connection,
                started: Instant::now(),
                messages: BTreeMap::new(),
                final_text: None,
            });
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> Vec<&T> {
        self.queue
            .iter()
            .map(|queued| &queued.request.payload)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn mock_active(
        client: Client,
        owner: String,
        payload: T,
        connection: CodexConfig,
    ) -> Self {
        Self {
            queue: VecDeque::new(),
            active: Some(Active {
                client,
                owner,
                payload,
                connection,
                started: Instant::now(),
                messages: BTreeMap::new(),
                final_text: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn background_jobs_bound_the_queue_and_cancel_before_using_changed_connections() {
        let root = tempfile::tempdir().unwrap();
        let config = CodexConfig::default();
        let mut jobs = StructuredJobs::default();
        for id in 0..4 {
            jobs.enqueue(
                Request {
                    owner: "a".into(),
                    payload: id,
                    instructions: "fixture".into(),
                    schema: json!({}),
                    prompt: "never sent".into(),
                },
                config.clone(),
            )
            .unwrap();
        }
        assert!(
            jobs.enqueue(
                Request {
                    owner: "a".into(),
                    payload: 5,
                    instructions: "fixture".into(),
                    schema: json!({}),
                    prompt: "never sent".into()
                },
                config.clone()
            )
            .is_err()
        );
        let mut changed = config.clone();
        changed.development_mode = true;
        for id in 0..4 {
            let outcome = jobs.poll(&changed, root.path(), || {}).unwrap();
            assert!(outcome.status == Status::Cancelled);
            assert_eq!(outcome.payload, id);
            assert!(outcome.result.is_err());
            assert!(!jobs.is_running());
        }
        assert!(jobs.pending().is_empty());

        let mut active = StructuredJobs::mock_active(
            Client::mock_events(vec![], vec![]),
            "b".into(),
            7,
            config.clone(),
        );
        active.active.as_mut().unwrap().started = Instant::now() - Duration::from_secs(91);
        let outcome = active.poll(&config, root.path(), || {}).unwrap();
        assert!(outcome.status == Status::Cancelled);
        assert!(outcome.result.unwrap_err().contains("超时"));
        assert!(!active.is_running());
    }

    #[test]
    fn structured_jobs_require_bounded_final_results_and_yield_during_event_bursts() {
        let root = tempfile::tempdir().unwrap();
        let config = CodexConfig::default();
        for (events, expected) in [
            (vec![Event::Finished { interrupted: false }], Status::Failed),
            (
                vec![Event::Text {
                    id: "r".into(),
                    text: "x".repeat(32769),
                    replace: true,
                }],
                Status::Failed,
            ),
            (
                vec![Event::Finished { interrupted: true }],
                Status::Cancelled,
            ),
            (
                vec![
                    Event::Text {
                        id: "r".into(),
                        text: "{\"events\":[]}".into(),
                        replace: true,
                    },
                    Event::Finished { interrupted: false },
                ],
                Status::Completed,
            ),
        ] {
            let mut jobs = StructuredJobs::mock_active(
                Client::mock_events(events, vec![]),
                "owner".into(),
                42,
                config.clone(),
            );
            let outcome = jobs.poll(&config, root.path(), || {}).unwrap();
            assert!(outcome.status == expected);
            assert_eq!(outcome.owner, "owner");
            assert_eq!(outcome.payload, 42);
            assert_eq!(outcome.result.is_ok(), expected == Status::Completed);
            assert!(!jobs.is_running());
        }
        let mut events: Vec<_> = (0..128)
            .map(|_| Event::Text {
                id: "r".into(),
                text: "x".into(),
                replace: false,
            })
            .collect();
        events.push(Event::Text {
            id: "r".into(),
            text: "final".into(),
            replace: true,
        });
        events.push(Event::Finished { interrupted: false });
        let mut jobs = StructuredJobs::mock_active(
            Client::mock_events(events, vec![]),
            "owner".into(),
            (),
            config.clone(),
        );
        assert!(jobs.poll(&config, root.path(), || {}).is_none());
        assert!(jobs.is_running());
        let mut result = None;
        for _ in 0..130 {
            if let Some(outcome) = jobs.poll(&config, root.path(), || {}) {
                result = Some(outcome);
                break;
            }
        }
        assert_eq!(result.unwrap().result.unwrap(), "final");
    }
}
