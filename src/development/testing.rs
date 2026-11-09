//! Test definitions and actual results; execution belongs to the existing backend.
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TestDefinition {
    pub id: u64,
    pub name: String,
    pub command: Vec<String>,
    pub timeout_seconds: u32,
}
impl TestDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 120
            || self.command.is_empty()
            || self.command.len() > 128
            || self.command[0].trim().is_empty()
            || self
                .command
                .iter()
                .any(|s| s.contains('\0') || s.len() > 8192)
            || !(1..=3600).contains(&self.timeout_seconds)
        {
            return Err("请填写测试名称、程序和参数，超时范围为 1～3600 秒。".into());
        }
        Ok(())
    }
    pub fn display_command(&self) -> String {
        self.command
            .iter()
            .map(|arg| serde_json::to_string(arg).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TestRun {
    pub id: u64,
    pub definition_id: u64,
    pub name: String,
    pub command: Vec<String>,
    pub started_at: u64,
    pub duration_ms: u64,
    pub status: String,
    pub exit_code: Option<i64>,
    pub stdout: String,
    pub stderr: String,
    pub error: String,
}
impl TestRun {
    pub fn sanitize(&mut self, filter: impl Fn(&str) -> String) {
        self.name = filter(&self.name);
        self.stdout = filter(&self.stdout);
        self.stderr = filter(&self.stderr);
        self.error = filter(&self.error);
        for arg in &mut self.command {
            *arg = filter(arg);
        }
    }
}
#[derive(Clone)]
pub struct TestRequest {
    pub id: u64,
    pub definition: TestDefinition,
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_definitions_preserve_argument_boundaries_and_validate_timeout() {
        let mut test = TestDefinition {
            name: "定向测试".into(),
            command: vec![
                "cargo".into(),
                "test".into(),
                "one argument with spaces".into(),
                "$(literal)".into(),
            ],
            timeout_seconds: 60,
            ..Default::default()
        };
        test.validate().unwrap();
        assert!(
            test.display_command()
                .contains("\"one argument with spaces\"")
        );
        test.timeout_seconds = 0;
        assert!(test.validate().is_err());
        test.timeout_seconds = 60;
        test.command.clear();
        assert!(test.validate().is_err());
    }
}
