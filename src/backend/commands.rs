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
        return "可用指令：\n/status — 查看会话、模型、权限、token 用量及账号剩余额度\n/model — 查看当前模型与推理强度\n/permissions — 查看审批策略与沙箱\n/help — 查看此帮助\n\n这些查询不会调用模型；/status 会向 Codex 查询最新账号额度。模型与连接配置继承本机 Codex，修改后请在设置中重启会话；无需审批开关从下一轮生效。其他 CLI 斜杠指令暂未接入 Lexington。".into();
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

pub(super) fn rate_limits(result: Result<&Value, &str>) -> String {
    let result = match result {
        Ok(result) => result,
        Err(error) => return format!("账号额度：查询失败（{error}）"),
    };
    let buckets: Vec<_> = result["rateLimitsByLimitId"]
        .as_object()
        .filter(|buckets| !buckets.is_empty())
        .map(|buckets| {
            buckets
                .iter()
                .map(|(id, value)| (id.as_str(), value))
                .collect()
        })
        .unwrap_or_else(|| vec![("Codex", &result["rateLimits"])]);
    let mut lines = vec!["账号额度：".into()];
    for (id, bucket) in buckets {
        let name = bucket["limitName"]
            .as_str()
            .filter(|name| !name.trim().is_empty())
            .or_else(|| {
                bucket["limitId"]
                    .as_str()
                    .filter(|id| !id.trim().is_empty())
            })
            .unwrap_or(id);
        if let Some(plan) = bucket["planType"].as_str() {
            lines.push(format!("{name} · 套餐：{plan}"));
        }
        for (field, fallback) in [("primary", "主额度"), ("secondary", "次额度")] {
            let window = &bucket[field];
            if !window.is_object() {
                continue;
            }
            let duration = match window["windowDurationMins"]
                .as_u64()
                .filter(|mins| *mins > 0)
            {
                Some(10080) => "每周额度（7天）".into(),
                Some(mins) if mins % 1440 == 0 => format!("{}天额度", mins / 1440),
                Some(mins) if mins % 60 == 0 => format!("{}小时额度", mins / 60),
                Some(mins) => format!("{mins}分钟额度"),
                None => fallback.into(),
            };
            let remaining = window["usedPercent"]
                .as_i64()
                .map(|used| format!("剩余 {}%", 100_i64.saturating_sub(used).clamp(0, 100)))
                .unwrap_or_else(|| "剩余比例未提供".into());
            let reset = window["resetsAt"]
                .as_i64()
                .and_then(crate::tools::time::local)
                .map(|date| {
                    format!(
                        "重置：{}-{:02}-{:02} {:02}:{:02}（本地时间）",
                        i64::from(date.tm_year) + 1900,
                        date.tm_mon + 1,
                        date.tm_mday,
                        date.tm_hour,
                        date.tm_min
                    )
                })
                .unwrap_or_else(|| "重置时间未提供".into());
            lines.push(format!("{name} · {duration}：{remaining}；{reset}"));
        }
        if bucket["credits"]["unlimited"].as_bool() == Some(true) {
            lines.push(format!("{name} · 积分：无限"));
        } else if let Some(balance) = bucket["credits"]["balance"].as_str() {
            lines.push(format!("{name} · 积分余额：{balance}"));
        }
    }
    if let Some(count) = result["rateLimitResetCredits"]["availableCount"].as_u64() {
        lines.push(format!("可用额度重置次数：{count}"));
    }
    if let Some(allowed) = result["ordinaryUsageAllowed"].as_bool() {
        lines.push(format!(
            "套餐内使用：{}",
            if allowed {
                "当前可用"
            } else {
                "当前不可用"
            }
        ));
    }
    if lines.len() == 1 {
        lines.push("当前登录方式或提供方未返回可用额度数据。".into());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quota_windows_use_server_durations_percentages_and_multiple_buckets() {
        let reply = rate_limits(Ok(&json!({
            "rateLimits":{"limitId":"legacy-duplicate","primary":{"usedPercent":99,"windowDurationMins":300}},
            "rateLimitsByLimitId":{
                "codex":{"planType":"plus","primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":1800000000},"secondary":{"usedPercent":35,"windowDurationMins":10080,"resetsAt":1800600000}},
                "other":{"limitName":"其他模型","secondary":{"usedPercent":120,"windowDurationMins":60},"credits":{"unlimited":true}}
            },
            "rateLimitResetCredits":{"availableCount":2},
            "ordinaryUsageAllowed":false
        })));
        assert!(reply.contains("codex · 5小时额度：剩余 75%"), "{reply}");
        assert!(reply.contains("codex · 每周额度（7天）：剩余 65%"));
        assert!(reply.contains("重置：") && reply.contains("（本地时间）"));
        assert!(reply.contains("其他模型 · 1小时额度：剩余 0%；重置时间未提供"));
        assert!(reply.contains("其他模型 · 积分：无限"));
        assert!(reply.contains("可用额度重置次数：2"));
        assert!(reply.contains("套餐内使用：当前不可用"));
        assert!(!reply.contains("legacy-duplicate"));
    }

    #[test]
    fn quota_legacy_sparse_and_unknown_windows_never_invent_a_weekly_balance() {
        let reply = rate_limits(Ok(&json!({"rateLimits":{
            "primary":{"usedPercent":0,"windowDurationMins":15,"resetsAt":i64::MAX},
            "secondary":{"usedPercent":100,"windowDurationMins":null,"resetsAt":null},
            "credits":{"hasCredits":true,"unlimited":false,"balance":"12.34"}
        },"rateLimitsByLimitId":{}})));
        assert!(reply.contains("15分钟额度：剩余 100%；重置时间未提供"));
        assert!(reply.contains("次额度：剩余 0%"));
        assert!(reply.contains("积分余额：12.34"));
        assert!(!reply.contains("每周"));
        let reply = rate_limits(Ok(
            &json!({"rateLimits":{"primary":{"windowDurationMins":1440}}}),
        ));
        assert!(reply.contains("1天额度：剩余比例未提供"));
        assert!(!reply.contains("100%"));
        let reply = rate_limits(Ok(&json!({"rateLimits":{}})));
        assert!(reply.contains("未返回可用额度数据"));
        assert!(!reply.contains('%'));
    }

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
