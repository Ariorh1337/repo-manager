use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn color(&self) -> egui::Color32 {
        match self {
            LogLevel::Info => egui::Color32::LIGHT_GRAY,
            LogLevel::Warning => egui::Color32::YELLOW,
            LogLevel::Error => egui::Color32::LIGHT_RED,
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            LogLevel::Info => "[I]",
            LogLevel::Warning => "[!]",
            LogLevel::Error => "[E]",
        }
    }
}

pub struct Logger {
    logs: Vec<LogEntry>,
    max_logs: usize,
}

impl Default for Logger {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl Logger {
    pub fn new(max_logs: usize) -> Self {
        Self {
            logs: Vec::new(),
            max_logs,
        }
    }

    pub fn info<T: Into<String>>(&mut self, message: T) {
        self.add_log(LogLevel::Info, message.into());
    }

    pub fn warning<T: Into<String>>(&mut self, message: T) {
        self.add_log(LogLevel::Warning, message.into());
    }

    pub fn error<T: Into<String>>(&mut self, message: T) {
        self.add_log(LogLevel::Error, message.into());
    }

    fn add_log(&mut self, level: LogLevel, message: String) {
        self.logs.push(LogEntry {
            timestamp: SystemTime::now(),
            level,
            message,
        });

        if self.logs.len() > self.max_logs {
            self.logs.remove(0);
        }
    }

    pub fn logs(&self) -> &[LogEntry] {
        &self.logs
    }

    pub fn clear(&mut self) {
        self.logs.clear();
    }

    pub fn error_count(&self) -> usize {
        self.logs
            .iter()
            .filter(|log| matches!(log.level, LogLevel::Error))
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.logs
            .iter()
            .filter(|log| matches!(log.level, LogLevel::Warning))
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.logs.len()
    }
}
