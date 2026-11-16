//! Shared HTTP client execution, status code validation, and async runtime bridging.

use std::future::Future;
use std::time::Duration;

use brain_core::errors::BrainError;

/// Bridges an async future into synchronous contexts across MultiThread, CurrentThread,
/// and external non-Tokio thread environments.
pub(crate) fn block_on_future<F, T>(f: F) -> Result<T, BrainError>
where
    F: Future<Output = Result<T, BrainError>> + Send + 'static,
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(f))
            }
            _ => std::thread::scope(|s| {
                s.spawn(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| BrainError::Internal {
                            message: format!("Failed to create tokio runtime for provider: {}", e),
                        })?;
                    rt.block_on(f)
                })
                .join()
                .map_err(|e| BrainError::Internal {
                    message: format!("Thread panicked during provider execution: {:?}", e),
                })?
            }),
        },
        Err(_) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| BrainError::Internal {
                    message: format!("Failed to create tokio runtime for provider: {}", e),
                })?;
            rt.block_on(f)
        }
    }
}

/// Maps a `reqwest::Error` to a strongly typed `BrainError`.
pub(crate) fn map_reqwest_error(
    err: reqwest::Error,
    url: &str,
    provider_name: &str,
    timeout: Duration,
) -> BrainError {
    if err.is_timeout() {
        BrainError::Timeout {
            elapsed_ms: timeout.as_millis() as u64,
            message: format!(
                "{} request to {} timed out after {}ms",
                provider_name,
                url,
                timeout.as_millis()
            ),
        }
    } else if err.is_connect() {
        BrainError::Network {
            message: format!("{} connection failed to {}: {}", provider_name, url, err),
            url: Some(url.to_string()),
        }
    } else {
        BrainError::Network {
            message: format!("{} request to {} failed: {}", provider_name, url, err),
            url: Some(url.to_string()),
        }
    }
}

/// Validates HTTP status code and maps 4xx/5xx responses to typed `BrainError`s.
pub(crate) async fn validate_response_status(
    response: reqwest::Response,
    provider_name: &str,
    model_id: &str,
) -> Result<String, BrainError> {
    let status = response.status();
    let status_code = status.as_u16();
    let body = response.text().await.unwrap_or_default();

    if status.is_success() {
        Ok(body)
    } else if status_code == 401 || status_code == 403 {
        Err(BrainError::Authorization {
            message: format!(
                "{} authorization failed (HTTP {}): {}",
                provider_name, status_code, body
            ),
        })
    } else if status_code == 400 || status_code == 422 {
        Err(BrainError::Validation {
            message: format!(
                "{} bad request (HTTP {}): {}",
                provider_name, status_code, body
            ),
        })
    } else if status_code == 429 {
        Err(BrainError::Model {
            model_id: model_id.to_string(),
            message: format!("{} rate limit exceeded (HTTP 429): {}", provider_name, body),
        })
    } else if status_code >= 500 {
        Err(BrainError::Model {
            model_id: model_id.to_string(),
            message: format!(
                "{} server error (HTTP {}): {}",
                provider_name, status_code, body
            ),
        })
    } else {
        Err(BrainError::Model {
            model_id: model_id.to_string(),
            message: format!(
                "{} request failed with HTTP {}: {}",
                provider_name, status_code, body
            ),
        })
    }
}
