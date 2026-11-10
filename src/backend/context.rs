//! Convert application context into a request-bound evidence manifest.
use crate::development::context::{Block, Channel, Delivery, Manifest};

pub(super) fn complete(
    manifest: &mut Manifest,
    text: &str,
    plan: Option<&crate::roleplay::ContextPlan>,
    instructions: Option<&str>,
    thread: &str,
    preview: bool,
) {
    manifest.preview = preview;
    manifest.thread_id = thread.into();
    manifest.captured_at = crate::development::testing::now();
    if let Some(block) = manifest
        .blocks
        .iter_mut()
        .find(|b| b.channel == Channel::Input)
    {
        block.content = text.into();
        block.included = true;
    } else {
        manifest.blocks.insert(
            0,
            Block {
                id: "input".into(),
                title: "用户输入".into(),
                category: "输入".into(),
                source: "当前输入".into(),
                scope: "本轮首个文本块".into(),
                content: text.into(),
                included: true,
                ..Default::default()
            },
        );
    }
    if let Some(instructions) = instructions.filter(|text| !text.is_empty()) {
        manifest.blocks.push(Block {
            id: "session:instructions".into(),
            title: "Lexington 会话指令".into(),
            category: "会话设定".into(),
            source: manifest.character.clone(),
            scope: "会话（本轮不重发）".into(),
            persistent: true,
            revision: crate::development::memory::context_stamp(instructions),
            validity: "创建或恢复线程时提供；角色或模式变化后新建会话。".into(),
            content: instructions.into(),
            included: true,
            delivery: Delivery::Retained,
            channel: Channel::Roleplay,
        });
    }
    if let Some(plan) = plan {
        if manifest.turn == 0 {
            manifest.turn = plan.turn;
        }
        for block in plan
            .blocks
            .iter()
            .filter(|block| !block.persistent || !block.included)
        {
            manifest.blocks.push(Block {
                id: format!("roleplay:{}", block.id),
                title: block.title.clone(),
                category: block.category.clone(),
                source: manifest.character.clone(),
                scope: if block.persistent { "会话" } else { "本轮" }.into(),
                persistent: block.persistent,
                revision: crate::development::memory::context_stamp(&block.content),
                validity: block.reason.clone(),
                content: format!("\n## {}\n{}\n", block.title, block.content),
                included: block.included,
                channel: Channel::Roleplay,
                ..Default::default()
            });
        }
    }
    for block in &mut manifest.blocks {
        block.delivery = if !block.included {
            Delivery::Excluded
        } else if preview {
            Delivery::Preview
        } else if block.delivery == Delivery::Retained {
            Delivery::Retained
        } else {
            Delivery::Written
        };
    }
}
