//! Runtime event definitions and bus.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Runtime event types.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    /// Container created.
    ContainerCreated { id: String, timestamp: i64 },
    /// Container started.
    ContainerStarted { id: String, timestamp: i64 },
    /// Container stopped.
    ContainerStopped { id: String, timestamp: i64 },
    /// Container paused.
    ContainerPaused { id: String, timestamp: i64 },
    /// Container resumed.
    ContainerResumed { id: String, timestamp: i64 },
    /// Container deleted.
    ContainerDeleted { id: String, timestamp: i64 },
}

/// Event bus for runtime events.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<RuntimeEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }
}

impl EventBus {
    /// Create a new event bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to events.
    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.sender.subscribe()
    }

    /// Publish an event.
    pub fn publish(&self, event: RuntimeEvent) {
        // Ignore SendError (no subscribers)
        let _ = self.sender.send(event);
    }
}
