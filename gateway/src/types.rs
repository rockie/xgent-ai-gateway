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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_new_generates_valid_uuid_v7() {
        let id = TaskId::new();
        let parsed = uuid::Uuid::parse_str(&id.0).expect("should be valid UUID");
        assert_eq!(parsed.get_version_num(), 7);
    }

    #[test]
    fn task_id_display() {
        let id = TaskId::new();
        let displayed = format!("{}", id);
        assert_eq!(displayed, id.0);
    }

    #[test]
    fn service_name_rejects_empty() {
        let result = ServiceName::new("");
        assert!(result.is_err());
    }

    #[test]
    fn service_name_accepts_valid() {
        let result = ServiceName::new("image-resize");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "image-resize");
    }

    #[test]
    fn transition_pending_to_assigned_ok() {
        let result = TaskState::Pending.try_transition(TaskState::Assigned);
        assert_eq!(result.unwrap(), TaskState::Assigned);
    }

    #[test]
    fn transition_pending_to_completed_err() {
        let result = TaskState::Pending.try_transition(TaskState::Completed);
        assert!(result.is_err());
    }

    #[test]
    fn transition_assigned_to_running_ok() {
        let result = TaskState::Assigned.try_transition(TaskState::Running);
        assert_eq!(result.unwrap(), TaskState::Running);
    }

    #[test]
    fn transition_running_to_completed_ok() {
        let result = TaskState::Running.try_transition(TaskState::Completed);
        assert_eq!(result.unwrap(), TaskState::Completed);
    }

    #[test]
    fn transition_running_to_failed_ok() {
        let result = TaskState::Running.try_transition(TaskState::Failed);
        assert_eq!(result.unwrap(), TaskState::Failed);
    }

    #[test]
    fn transition_completed_to_anything_err() {
        assert!(TaskState::Completed.try_transition(TaskState::Pending).is_err());
        assert!(TaskState::Completed.try_transition(TaskState::Assigned).is_err());
        assert!(TaskState::Completed.try_transition(TaskState::Running).is_err());
        assert!(TaskState::Completed.try_transition(TaskState::Failed).is_err());
    }

    #[test]
    fn transition_failed_to_anything_err() {
        assert!(TaskState::Failed.try_transition(TaskState::Pending).is_err());
        assert!(TaskState::Failed.try_transition(TaskState::Assigned).is_err());
        assert!(TaskState::Failed.try_transition(TaskState::Running).is_err());
        assert!(TaskState::Failed.try_transition(TaskState::Completed).is_err());
    }

    #[test]
    fn task_state_roundtrip_str() {
        for state in [
            TaskState::Pending,
            TaskState::Assigned,
            TaskState::Running,
            TaskState::Completed,
            TaskState::Failed,
        ] {
            let s = state.as_str();
            let parsed = TaskState::from_str(s).unwrap();
            assert_eq!(parsed, state);
        }
    }

    #[test]
    fn task_state_from_str_invalid() {
        assert!(TaskState::from_str("unknown").is_err());
    }

    #[test]
    fn task_state_proto_roundtrip() {
        for state in [
            TaskState::Pending,
            TaskState::Assigned,
            TaskState::Running,
            TaskState::Completed,
            TaskState::Failed,
        ] {
            let i: i32 = state.into();
            let back = TaskState::try_from(i).unwrap();
            assert_eq!(back, state);
        }
    }
}
