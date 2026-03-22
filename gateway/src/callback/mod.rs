use std::time::Duration;

/// Deliver a callback notification to the given URL with exponential backoff retry.
///
/// POSTs `{"task_id": "...", "state": "..."}` to the URL. On failure, retries up to
/// `max_retries` times with exponential backoff (delay = initial_delay_ms * 2^(attempt-1)).
/// Callback failure is log-only -- it never returns an error to the caller.
///
/// If `callback_counter` is provided, increments it with "success" on successful delivery
/// or "exhausted" when all retries are exhausted.
pub async fn deliver_callback(
    client: reqwest::Client,
    url: String,
    task_id: String,
    state: String,
    max_retries: u32,
    initial_delay_ms: u64,
    callback_counter: Option<prometheus::CounterVec>,
) {
    let body = serde_json::json!({
        "task_id": task_id,
        "state": state,
    });

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = initial_delay_ms * 2u64.pow(attempt - 1);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    task_id = %task_id,
                    url = %url,
                    attempt = attempt,
                    "callback delivered successfully"
                );
                if let Some(ref counter) = callback_counter {
                    counter.with_label_values(&["success"]).inc();
                }
                return;
            }
            Ok(resp) => {
                tracing::warn!(
                    task_id = %task_id,
                    url = %url,
                    status = %resp.status(),
                    attempt = attempt + 1,
                    max = max_retries + 1,
                    "callback delivery failed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    task_id = %task_id,
                    url = %url,
                    error = %e,
                    attempt = attempt + 1,
                    max = max_retries + 1,
                    "callback delivery error"
                );
            }
        }
    }
    tracing::error!(
        task_id = %task_id,
        url = %url,
        "callback delivery exhausted all retries"
    );
    if let Some(ref counter) = callback_counter {
        counter.with_label_values(&["exhausted"]).inc();
    }
}

/// Validate that a callback URL is well-formed and uses http or https scheme.
pub fn validate_callback_url(url_str: &str) -> Result<(), String> {
    match url::Url::parse(url_str) {
        Ok(parsed) => {
            if parsed.scheme() != "http" && parsed.scheme() != "https" {
                Err(format!(
                    "callback URL must use http or https scheme, got: {}",
                    parsed.scheme()
                ))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(format!("invalid callback URL: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_callback_url_valid_http() {
        assert!(validate_callback_url("http://example.com/hook").is_ok());
    }

    #[test]
    fn test_validate_callback_url_valid_https() {
        assert!(validate_callback_url("https://example.com/hook").is_ok());
    }

    #[test]
    fn test_validate_callback_url_invalid_scheme() {
        let result = validate_callback_url("ftp://example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("http or https"));
    }

    #[test]
    fn test_validate_callback_url_malformed() {
        let result = validate_callback_url("not a url");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid callback URL"));
    }
}
