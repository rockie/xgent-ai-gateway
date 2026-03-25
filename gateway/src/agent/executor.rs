use std::collections::HashMap;

use async_trait::async_trait;
use xgent_proto::TaskAssignment;

/// Result of executing a task assignment.
pub struct ExecutionResult {
    pub success: bool,
    pub result: String,
    pub error_message: String,
    pub headers: HashMap<String, String>,
}

/// Trait for task executors. Each execution mode (CLI, sync-api, async-api)
/// implements this trait to provide mode-specific task dispatch.
#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_result_has_required_fields() {
        let result = ExecutionResult {
            success: true,
            result: "test-result".to_string(),
            error_message: String::new(),
            headers: HashMap::new(),
        };
        assert!(result.success);
        assert_eq!(result.result, "test-result");
        assert!(result.error_message.is_empty());
        assert!(result.headers.is_empty());
    }

    #[test]
    fn execution_result_failure() {
        let result = ExecutionResult {
            success: false,
            result: String::new(),
            error_message: "something went wrong".to_string(),
            headers: HashMap::new(),
        };
        assert!(!result.success);
        assert!(result.result.is_empty());
        assert_eq!(result.error_message, "something went wrong");
    }
}
