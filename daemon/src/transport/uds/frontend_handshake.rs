//! Canonical frontend handshake negotiation for the versioned UDS transport.

use std::collections::BTreeSet;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;

use brain_integrations::dto::v1::{
    FrontendHandshakeErrorCode, FrontendHandshakeFailure, FrontendHandshakeRequest,
    FrontendHandshakeResponse,
};
use brain_services::{CURRENT_PROTOCOL_VERSION, MIN_SUPPORTED_PROTOCOL_VERSION};

/// Protocol family accepted by the canonical frontend facade.
pub const FRONTEND_PROTOCOL: &str = "brain.frontend";

const SUPPORTED_CAPABILITIES: [&str; 3] = [
    "generation.cancel.v1",
    "generation.stream.v1",
    "sessions.v1",
];

/// Parses and negotiates a frontend compatibility request.
pub fn negotiate(
    payload: &str,
    backend_version: &str,
) -> Result<FrontendHandshakeResponse, FrontendHandshakeFailure> {
    let request: FrontendHandshakeRequest =
        serde_json::from_str(payload).map_err(|error| FrontendHandshakeFailure {
            code: FrontendHandshakeErrorCode::MalformedHandshakeRequest,
            message: format!("Malformed frontend handshake request: {error}"),
            supported_minimum_protocol_version: None,
            supported_maximum_protocol_version: None,
            missing_required_capabilities: Vec::new(),
        })?;

    if request.protocol != FRONTEND_PROTOCOL {
        return Err(FrontendHandshakeFailure {
            code: FrontendHandshakeErrorCode::UnsupportedFrontendMode,
            message: format!(
                "Unsupported frontend protocol '{}'; expected '{FRONTEND_PROTOCOL}'",
                request.protocol
            ),
            supported_minimum_protocol_version: None,
            supported_maximum_protocol_version: None,
            missing_required_capabilities: Vec::new(),
        });
    }

    if request.minimum_protocol_version > request.maximum_protocol_version {
        return Err(FrontendHandshakeFailure {
            code: FrontendHandshakeErrorCode::MalformedHandshakeRequest,
            message: format!(
                "Malformed frontend handshake request: minimum protocol version {} exceeds maximum {}",
                request.minimum_protocol_version, request.maximum_protocol_version
            ),
            supported_minimum_protocol_version: None,
            supported_maximum_protocol_version: None,
            missing_required_capabilities: Vec::new(),
        });
    }

    if request.maximum_protocol_version < MIN_SUPPORTED_PROTOCOL_VERSION
        || request.minimum_protocol_version > CURRENT_PROTOCOL_VERSION
    {
        return Err(FrontendHandshakeFailure {
            code: FrontendHandshakeErrorCode::UnsupportedProtocolVersion,
            message: format!(
                "Frontend protocol range {}..{} is unsupported; backend supports {}..{}",
                request.minimum_protocol_version,
                request.maximum_protocol_version,
                MIN_SUPPORTED_PROTOCOL_VERSION,
                CURRENT_PROTOCOL_VERSION
            ),
            supported_minimum_protocol_version: Some(MIN_SUPPORTED_PROTOCOL_VERSION),
            supported_maximum_protocol_version: Some(CURRENT_PROTOCOL_VERSION),
            missing_required_capabilities: Vec::new(),
        });
    }

    let supported: BTreeSet<&str> = SUPPORTED_CAPABILITIES.into_iter().collect();
    let required: BTreeSet<&str> = request
        .required_capabilities
        .iter()
        .map(String::as_str)
        .collect();
    let missing_required_capabilities: Vec<String> = required
        .difference(&supported)
        .map(|capability| (*capability).to_string())
        .collect();
    if !missing_required_capabilities.is_empty() {
        return Err(FrontendHandshakeFailure {
            code: FrontendHandshakeErrorCode::MissingRequiredCapability,
            message: format!(
                "Brain daemon does not support required frontend capabilities: {}",
                missing_required_capabilities.join(", ")
            ),
            supported_minimum_protocol_version: None,
            supported_maximum_protocol_version: None,
            missing_required_capabilities,
        });
    }

    let optional: BTreeSet<&str> = request
        .optional_capabilities
        .iter()
        .map(String::as_str)
        .collect();
    let accepted_optional_capabilities = optional
        .intersection(&supported)
        .map(|capability| (*capability).to_string())
        .collect();

    Ok(FrontendHandshakeResponse {
        selected_protocol_version: request
            .maximum_protocol_version
            .min(CURRENT_PROTOCOL_VERSION),
        backend_name: "brain-daemon".to_string(),
        backend_version: backend_version.to_string(),
        daemon_state: "ready".to_string(),
        supported_capabilities: SUPPORTED_CAPABILITIES
            .into_iter()
            .map(str::to_string)
            .collect(),
        accepted_optional_capabilities,
    })
}

/// Builds the versioned UDS response envelope for a canonical frontend
/// handshake without exposing handshake details to the connection router.
fn response_envelope(
    payload: &str,
    backend_version: &str,
    request_id: &serde_json::Value,
) -> Result<serde_json::Value, serde_json::Error> {
    match negotiate(payload, backend_version) {
        Ok(body) => Ok(serde_json::json!({
            "version": "1.0",
            "type": "Response",
            "id": request_id,
            "status": "success",
            "body": serde_json::to_string(&body)?,
        })),
        Err(failure) => Ok(serde_json::json!({
            "version": "1.0",
            "type": "Error",
            "id": request_id,
            "status": "error",
            "body": serde_json::to_string(&failure)?,
        })),
    }
}

/// Writes the complete versioned frontend-handshake response to a UDS client.
///
/// The router owns action selection; this module owns the handshake DTO,
/// negotiation result, and exact versioned envelope shape.
pub async fn write_response(
    writer: &Arc<tokio::sync::Mutex<OwnedWriteHalf>>,
    request_id: &serde_json::Value,
    payload: &str,
    backend_version: &str,
) -> std::io::Result<()> {
    let response = response_envelope(payload, backend_version, request_id).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("serialize frontend handshake response: {error}"),
        )
    })?;
    let mut response_json = serde_json::to_string(&response).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("serialize frontend handshake envelope: {error}"),
        )
    })?;
    response_json.push('\n');
    let mut locked_writer = writer.lock().await;
    locked_writer.write_all(response_json.as_bytes()).await?;
    locked_writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FrontendHandshakeRequest {
        FrontendHandshakeRequest {
            protocol: FRONTEND_PROTOCOL.to_string(),
            minimum_protocol_version: 1,
            maximum_protocol_version: 1,
            frontend_name: "test-frontend".to_string(),
            frontend_version: "1.0.0".to_string(),
            frontend_build_id: None,
            required_capabilities: vec!["sessions.v1".to_string()],
            optional_capabilities: vec!["memory.provenance.v1".to_string()],
        }
    }

    #[test]
    fn selects_the_highest_overlapping_protocol_and_only_supported_optionals() {
        let payload = serde_json::to_string(&request()).expect("serialize request");

        let response = negotiate(&payload, "test-version").expect("negotiate request");

        assert_eq!(response.selected_protocol_version, 1);
        assert_eq!(response.backend_version, "test-version");
        assert_eq!(
            response.accepted_optional_capabilities,
            Vec::<String>::new()
        );
    }

    #[test]
    fn rejects_missing_required_capabilities_deterministically() {
        let mut request = request();
        request.required_capabilities.push("tools.v9".to_string());
        let payload = serde_json::to_string(&request).expect("serialize request");

        let failure = negotiate(&payload, "test-version").expect_err("missing capability fails");

        assert_eq!(
            failure.code,
            FrontendHandshakeErrorCode::MissingRequiredCapability
        );
        assert_eq!(failure.missing_required_capabilities, vec!["tools.v9"]);
    }

    #[test]
    fn response_envelope_preserves_versioned_success_and_typed_failure_shapes() {
        let request_id = serde_json::json!(42);
        let valid_payload = serde_json::to_string(&request()).expect("serialize request");

        let success = response_envelope(&valid_payload, "test-version", &request_id)
            .expect("serialize success envelope");
        assert_eq!(success["version"], "1.0");
        assert_eq!(success["type"], "Response");
        assert_eq!(success["id"], request_id);
        assert_eq!(success["status"], "success");

        let failure = response_envelope("not-json", "test-version", &request_id)
            .expect("serialize error envelope");
        assert_eq!(failure["version"], "1.0");
        assert_eq!(failure["type"], "Error");
        assert_eq!(failure["id"], request_id);
        assert_eq!(failure["status"], "error");
        let body: FrontendHandshakeFailure =
            serde_json::from_str(failure["body"].as_str().expect("error body string"))
                .expect("typed error body");
        assert_eq!(
            body.code,
            FrontendHandshakeErrorCode::MalformedHandshakeRequest
        );
    }
}
