use crate::error::GatewayError;
use std::fmt;

/// Newtype wrapping a UUID v7 task identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Newtype for service names, validated non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceName(pub String);

impl ServiceName {
    pub fn new(s: impl Into<String>) -> Result<Self, GatewayError> {
        let s = s.into();
        if s.is_empty() {
            return Err(GatewayError::InvalidRequest(
                "service name must not be empty".to_string(),
            ));
        }
        Ok(Self(s))
    }
}

impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ServiceName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Task lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

impl TaskState {
    /// Attempt a state transition, returning the new state or an error if invalid.
    pub fn try_transition(&self, to: TaskState) -> Result<TaskState, GatewayError> {
        let valid = matches!(
            (self, to),
            (TaskState::Pending, TaskState::Assigned)
                | (TaskState::Assigned, TaskState::Running)
                | (TaskState::Running, TaskState::Completed)
                | (TaskState::Running, TaskState::Failed)
        );

        if valid {
            Ok(to)
        } else {
            Err(GatewayError::InvalidStateTransition {
                from: self.as_str().to_string(),
                to: to.as_str().to_string(),
            })
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::Pending => "pending",
            TaskState::Assigned => "assigned",
            TaskState::Running => "running",
            TaskState::Completed => "completed",
            TaskState::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, GatewayError> {
        match s {
            "pending" => Ok(TaskState::Pending),
            "assigned" => Ok(TaskState::Assigned),
            "running" => Ok(TaskState::Running),
            "completed" => Ok(TaskState::Completed),
            "failed" => Ok(TaskState::Failed),
            other => Err(GatewayError::InvalidRequest(format!(
                "unknown task state: {other}"
            ))),
        }
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Conversion from proto TaskState (i32) to domain TaskState.
impl TryFrom<i32> for TaskState {
    type Error = GatewayError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(TaskState::Pending),
            2 => Ok(TaskState::Assigned),
            3 => Ok(TaskState::Running),
            4 => Ok(TaskState::Completed),
            5 => Ok(TaskState::Failed),
            other => Err(GatewayError::InvalidRequest(format!(
                "unknown proto task state: {other}"
            ))),
        }
    }
}

/// Conversion from domain TaskState to proto TaskState (i32).
impl From<TaskState> for i32 {
    fn from(state: TaskState) -> Self {
        match state {
            TaskState::Pending => 1,
            TaskState::Assigned => 2,
            TaskState::Running => 3,
            TaskState::Completed => 4,
            TaskState::Failed => 5,
        }
    }
}
