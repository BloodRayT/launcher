use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    Log(String),
    Progress {
        label: String,
        task: usize,
        total: usize,
    },
    Running,
}

/// Канал событий запуска. В приложении он связан с окном Tauri, в тестах — с заглушкой.
#[derive(Clone)]
pub struct Reporter(Arc<dyn Fn(GameEvent) + Send + Sync>);

impl Reporter {
    pub fn new(sink: impl Fn(GameEvent) + Send + Sync + 'static) -> Self {
        Self(Arc::new(sink))
    }

    pub fn silent() -> Self {
        Self::new(|_| {})
    }

    pub fn log(&self, message: impl Into<String>) {
        (self.0)(GameEvent::Log(message.into()));
    }

    pub fn progress(&self, label: impl Into<String>, task: usize, total: usize) {
        (self.0)(GameEvent::Progress {
            label: label.into(),
            task,
            total,
        });
    }

    pub fn running(&self) {
        (self.0)(GameEvent::Running);
    }
}
