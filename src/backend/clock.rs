use super::config::ClockConfig;
use std::time::Instant;

#[derive(Default)]
pub struct ClockState {
    pub started: Option<Instant>,
}
impl ClockState {
    pub fn toggle(&mut self, config: &ClockConfig) -> String {
        if self.started.take().is_some() {
            config.clock_in_cancel_message.clone()
        } else {
            self.started = Some(Instant::now());
            config.clock_in_message.clone()
        }
    }
    pub fn poll(&mut self, config: &ClockConfig) -> Option<String> {
        if self
            .started
            .is_some_and(|t| t.elapsed().as_secs_f64() >= config.work_reminder_hours * 3600.)
        {
            self.started = None;
            Some(config.clock_out_message.clone())
        } else {
            None
        }
    }
}
