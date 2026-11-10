//! Client-side commands; app-server does not interpret CLI slash text.
use serde_json::Value;
use std::path::Path;

pub const COMMANDS: &[(&str, &str)] = &[
    ("/status", "会话状态"),
    ("/model", "查看当前模型"),
    ("/permissions", "查看当前权限"),
    ("/help", "指令帮助"),
];

const CLI_ONLY: &[&str] = &[
    "/plan", "/compact", "/review", "/new", "/resume", "/fork", "/diff", "/init", "/skills",
    "/mcp", "/goal", "/fast", "/mention",
];

pub fn is_command(text: &str) -> bool {
    let command = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    COMMANDS.iter().any(|(name, _)| *name == command) || CLI_ONLY.contains(&command.as_str())
}

pub(super) fn response(
    text: &str,
    settings: &Value,
    usage: &Value,
    directory: &Path,
    no_approval: bool,
) -> String {
    let mut words = text.split_whitespace();
    let command = words.next().unwrap_or_default().to_ascii_lowercase();
    if !COMMANDS.iter().any(|(name, _)| *name == command) {
        return format!(
            "Lexington 暂不支持 {command}，请在 Codex CLI 中使用。输入 /help 查看当前可用指令。"
        );
    }
    if words.next().is_some() {
        return format!(
            "{command} 暂不接受参数。输入 /help 查看用法；模型与权限查询不会修改配置。"
        );
    }
    if command == "/help" {
        return "可用指令：\n/status — 查看会话、模型、权限及已收到的 token 用量\n/model — 查看当前模型与推理强度\n/permissions — 查看审批策略与沙箱\n/help — 查看此帮助\n\n这些查询不会调用模型。模型与连接配置继承本机 Codex，修改后请在设置中重启会话；无需审批开关从下一轮生效。其他 CLI 斜杠指令暂未接入 Lexington。".into();
    }
    let mut lines = Vec::new();
    if command == "/status" {
        lines.push(format!(
            "会话：{}",
            settings["thread"]["id"].as_str().unwrap_or("未返回")
        ));
        lines.push(format!(
            "工作目录：{}",
            settings["cwd"]
                .as_str()
                .map_or_else(|| directory.display().to_string(), str::to_owned)
        ));
        lines.push("状态：已连接，空闲".into());
    }
    if command != "/permissions" {
        lines.push(format!(
            "模型：{}",
            settings["model"].as_str().unwrap_or("未返回")
        ));
        lines.push(format!(
            "提供方：{}",
            settings["modelProvider"].as_str().unwrap_or("未返回")
        ));
        lines.push(format!(
            "推理强度：{}",
            settings["reasoningEffort"]
                .as_str()
                .unwrap_or("默认（未返回）")
        ));
    }
    if command != "/model" {
        let policy = if no_approval {
            Value::String("never".into())
        } else {
            settings["approvalPolicy"].clone()
        };
        let policy = policy
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| policy.to_string());
        lines.push(format!("下轮审批策略：{policy}"));
        lines.push(format!(
            "沙箱：{}",
            settings["sandbox"]["type"].as_str().unwrap_or("未返回")
        ));
    }
    if command == "/status" {
        if let Some(total) = usage["total"]["totalTokens"].as_u64() {
            lines.push(format!("累计 token：{total}"));
        } else {
            lines.push("token 用量：尚未收到".into());
        }
        if let Some(last) = usage["last"]["totalTokens"].as_u64() {
            lines.push(format!("最近一次 token：{last}"));
        }
        if let Some(window) = usage["modelContextWindow"].as_u64() {
            lines.push(format!("上下文窗口：{window} token"));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_recognition_preserves_unknown_words_paths_and_ordinary_text() {
        for text in [
            " /STATUS ",
            "/model other",
            "/permissions",
            "/compact 继续",
            "/help",
        ] {
            assert!(is_command(text), "{text}");
        }
        for text in [
            "",
            "请看 /status",
            "/unknown 中文",
            "/tmp/文件.txt",
            "/status/file",
            "/status.md",
        ] {
            assert!(!is_command(text), "{text}");
        }
        let settings = json!({"approvalPolicy":{"granular":{"rules":false}}});
        let reply = response(
            "/permissions",
            &settings,
            &Value::Null,
            Path::new("."),
            false,
        );
        assert!(reply.contains(r#"{"granular":{"rules":false}}"#));
        assert!(reply.contains("沙箱：未返回"));
        let reply = response(
            "/status",
            &settings,
            &Value::Null,
            Path::new("fixture-directory"),
            false,
        );
        assert!(reply.contains("模型：未返回"));
        assert!(reply.contains("token 用量：尚未收到"));
        assert!(reply.contains("工作目录：fixture-directory"));
    }
}
