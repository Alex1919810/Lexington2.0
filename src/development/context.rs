//! Application-owned context evidence; never claims to describe the full model history.
use super::{Workspace, memory};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Channel {
    #[default]
    Input,
    Project,
    Roleplay,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Delivery {
    #[default]
    Prepared,
    Written,
    Retained,
    Excluded,
    Preview,
}
impl Delivery {
    pub fn label(self) -> &'static str {
        match self {
            Self::Prepared => "待发送",
            Self::Written => "已写入请求",
            Self::Retained => "沿用会话设定",
            Self::Excluded => "未载入",
            Self::Preview => "预览",
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Block {
    pub id: String,
    pub title: String,
    pub category: String,
    pub source: String,
    pub scope: String,
    pub persistent: bool,
    pub revision: String,
    pub validity: String,
    pub content: String,
    pub included: bool,
    pub channel: Channel,
    pub delivery: Delivery,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    pub character: String,
    pub workspace: String,
    pub thread_id: String,
    pub turn: usize,
    pub captured_at: u64,
    pub preview: bool,
    pub blocks: Vec<Block>,
}
impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: 1,
            character: String::new(),
            workspace: String::new(),
            thread_id: String::new(),
            turn: 0,
            captured_at: 0,
            preview: false,
            blocks: Vec::new(),
        }
    }
}
impl std::fmt::Debug for Manifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Manifest")
            .field("version", &self.version)
            .field("preview", &self.preview)
            .field("blocks", &self.blocks.len())
            .finish()
    }
}
impl Manifest {
    pub fn text(&self, channel: Channel) -> String {
        self.blocks
            .iter()
            .filter(|block| block.included && block.channel == channel && !block.persistent)
            .map(|block| block.content.as_str())
            .collect::<Vec<_>>()
            .join(if channel == Channel::Roleplay {
                ""
            } else {
                "\n\n"
            })
    }

    pub fn sanitize(&mut self, filter: impl Fn(&str) -> String) {
        self.character = filter(&self.character);
        self.workspace = filter(&self.workspace);
        self.thread_id = filter(&self.thread_id);
        for block in &mut self.blocks {
            for text in [
                &mut block.id,
                &mut block.title,
                &mut block.category,
                &mut block.source,
                &mut block.scope,
                &mut block.revision,
                &mut block.validity,
                &mut block.content,
            ] {
                *text = filter(text);
            }
        }
    }
}

/// Build once for the request. The returned text is also the actual project input.
pub fn project_manifest(project: Option<&Workspace>, enabled: bool, character: &str) -> Manifest {
    let mut manifest = Manifest {
        character: character.into(),
        workspace: project.map_or_else(String::new, |p| p.path.display().to_string()),
        ..Default::default()
    };
    let Some(project) = project else {
        return manifest;
    };
    // Disabled mode must not read memory evidence files or prepare project content.
    if !enabled {
        manifest.blocks.push(Block {
            id: "project:disabled".into(),
            title: "项目背景".into(),
            category: "项目".into(),
            source: manifest.workspace.clone(),
            scope: "本轮".into(),
            validity: "development 模式关闭，未附加诊断、测试或项目记忆。".into(),
            channel: Channel::Project,
            ..Default::default()
        });
        return manifest;
    }
    let (text, report) = memory::context(&project.path, &project.memories);
    manifest.blocks.push(Block {
        id: "project:memories".into(),
        title: "已核对的项目记忆".into(),
        category: "项目记忆".into(),
        source: manifest.workspace.clone(),
        scope: "本轮".into(),
        revision: format!(
            "{} / {}",
            project.memory_revision,
            memory::context_stamp(&text)
        ),
        validity: "仅载入已确认、启用且依据文件匹配的条目；依据变化后重新核对。".into(),
        included: !text.is_empty(),
        content: text,
        channel: Channel::Project,
        ..Default::default()
    });
    for (id, status) in report {
        if status != "可载入"
            && let Some(entry) = project.memories.iter().find(|entry| entry.id == id)
        {
            manifest.blocks.push(Block {
                id: format!("project:memory:{id}"),
                title: entry.title.clone(),
                category: "项目记忆".into(),
                source: entry.source_file.clone(),
                scope: "本轮".into(),
                validity: status,
                channel: Channel::Project,
                ..Default::default()
            });
        }
    }
    if let Some(snapshot) = &project.diagnostics {
        let text = snapshot.context();
        manifest.blocks.push(Block {
            id: "project:diagnostics".into(),
            title: "编辑器诊断".into(),
            category: "诊断".into(),
            source: "VS Code 诊断快照".into(),
            scope: "本轮".into(),
            revision: snapshot.captured_at.to_string(),
            validity: "已过滤磁盘版本过期的文档；未保存文档只代表编辑器快照。".into(),
            included: !text.is_empty(),
            content: text,
            channel: Channel::Project,
            ..Default::default()
        });
    }
    let mut remaining = 6000;
    let mut exhausted = false;
    for definition in &project.tests {
        let text = format!("{}: {}\n", definition.name, definition.display_command());
        let size =
            format!("[登记的测试命令，仅供选择，未自动运行；历史结果不代表当前代码]\n{text}")
                .chars()
                .count();
        let included = !exhausted && size <= remaining;
        if included {
            remaining -= size;
        } else {
            exhausted = true;
        }
        manifest.blocks.push(Block {
            id: format!("project:test:{}", definition.id),
            title: definition.name.clone(),
            category: "测试配置".into(),
            source: manifest.workspace.clone(),
            scope: "本轮".into(),
            revision: memory::context_stamp(&text),
            validity: if included {
                "登记的测试命令，仅供选择，未自动运行；历史结果不代表当前代码"
            } else {
                "超出测试命令的 6000 字符预算"
            }
            .into(),
            content: if included {
                format!("[登记的测试命令，仅供选择，未自动运行；历史结果不代表当前代码]\n{text}")
            } else {
                String::new()
            },
            included,
            channel: Channel::Project,
            ..Default::default()
        });
    }
    for run in &project.test_runs {
        manifest.blocks.push(Block {
            id: format!("project:test-run:{}", run.id),
            title: format!("历史测试：{}", run.name),
            category: "历史测试记录".into(),
            source: manifest.workspace.clone(),
            scope: "历史".into(),
            revision: run.started_at.to_string(),
            validity: format!(
                "历史状态 {}，退出码 {:?}；未自动附加。显式分析失败时才将所选结果作为输入发送。",
                run.status, run.exit_code
            ),
            channel: Channel::Project,
            ..Default::default()
        });
    }
    manifest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::development::{
        memory::ProjectMemory,
        testing::{TestDefinition, TestRun},
    };

    #[test]
    fn project_manifest_reports_exclusions_and_uses_the_same_selected_text_as_the_request() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.rs");
        std::fs::write(&source, "original").unwrap();
        let mut entry = ProjectMemory {
            id: 1,
            title: "规则".into(),
            content: "verified-evidence".into(),
            source_file: "source.rs".into(),
            confirmed: true,
            enabled: true,
            ..Default::default()
        };
        entry.prepare(root.path()).unwrap();
        let project = Workspace {
            path: root.path().into(),
            memories: vec![entry],
            tests: vec![TestDefinition {
                id: 2,
                name: "定向测试".into(),
                command: vec!["cargo".into(), "test".into(), "one argument".into()],
                ..Default::default()
            }],
            test_runs: vec![TestRun {
                id: 3,
                name: "旧测试".into(),
                stdout: "old-output".into(),
                status: "passed".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let enabled = project_manifest(Some(&project), true, "a");
        assert!(enabled.text(Channel::Project).contains("verified-evidence"));
        assert!(enabled.text(Channel::Project).contains("one argument"));
        assert!(!enabled.text(Channel::Project).contains("old-output"));
        assert!(
            enabled
                .blocks
                .iter()
                .any(|block| block.id == "project:test-run:3" && !block.included)
        );
        std::fs::write(&source, "changed").unwrap();
        let changed = project_manifest(Some(&project), true, "a");
        assert!(!changed.text(Channel::Project).contains("verified-evidence"));
        assert!(
            changed
                .blocks
                .iter()
                .any(|block| block.validity == "依据已变化" && !block.included)
        );
        let disabled = project_manifest(Some(&project), false, "a");
        assert!(disabled.text(Channel::Project).is_empty());
        assert_eq!(disabled.blocks.len(), 1);
    }
}
