use crate::server::protocol::ClientRequest;
use brain_application::dispatcher::ApplicationRequest;
use brain_integrations::IngestionEnvelope;
use brain_integrations::dto::v1::SearchQuery;

/// UDS protocol router mapping wire protocol actions to application request variants.
pub struct ProtocolRouter;

impl ProtocolRouter {
    /// Routes a UDS client request to the corresponding typed application request if possible.
    pub fn route(request: &ClientRequest) -> Result<Option<ApplicationRequest>, String> {
        let (action, body) = match request {
            ClientRequest::Versioned(req) => (req.action.as_str(), req.body.as_str()),
            ClientRequest::Legacy(req) => (req.action.as_str(), req.payload.as_str()),
        };

        match action {
            // New versioned actions
            "v1/status" => Ok(Some(ApplicationRequest::Status)),
            "v1/metrics" => Ok(Some(ApplicationRequest::Metrics)),
            "v1/diagnostics" => Ok(Some(ApplicationRequest::Diagnostics)),
            "v1/capabilities" => Ok(Some(ApplicationRequest::Capabilities)),
            "v1/reflect" => Ok(Some(ApplicationRequest::Reflect)),
            "v1/search" => {
                let query: SearchQuery = serde_json::from_str(body)
                    .map_err(|e| format!("Failed to parse SearchQuery: {}", e))?;
                Ok(Some(ApplicationRequest::Search(query)))
            }
            "v1/ingest" => {
                let envelope: IngestionEnvelope = serde_json::from_str(body)
                    .map_err(|e| format!("Failed to parse IngestionEnvelope: {}", e))?;
                Ok(Some(ApplicationRequest::Ingest(envelope)))
            }
            "v1/replay" => {
                let after_sequence: u64 = serde_json::from_str(body)
                    .or_else(|_| body.parse::<u64>())
                    .map_err(|e| format!("Failed to parse replay sequence: {}", e))?;
                Ok(Some(ApplicationRequest::Replay { after_sequence }))
            }
            "v1/inspect_node" => Ok(Some(ApplicationRequest::InspectNode {
                id: body.to_string(),
            })),
            "v1/subscribe" => {
                let after_sequence: Option<u64> = if body.trim().is_empty() {
                    None
                } else {
                    serde_json::from_str(body)
                        .or_else(|_| body.parse::<u64>().map(Some))
                        .unwrap_or(None)
                };
                Ok(Some(ApplicationRequest::Subscribe { after_sequence }))
            }
            "v1/projections" => Ok(Some(ApplicationRequest::ListProjectionStatus)),
            "v1/rebuild_projection" => {
                let name: String = serde_json::from_str(body)
                    .unwrap_or_else(|_| body.trim().to_string());
                Ok(Some(ApplicationRequest::RebuildProjection { name }))
            }

            // Legacy fallbacks (deprecated)
            "status" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'status'. Please upgrade client.");
                Ok(Some(ApplicationRequest::Status))
            }
            "metrics" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'metrics'. Please upgrade client.");
                Ok(Some(ApplicationRequest::Metrics))
            }
            "heartbeat" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'heartbeat'. Please upgrade client.");
                Ok(Some(ApplicationRequest::Status))
            }
            "query" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'query'. Please upgrade client.");
                let query = SearchQuery {
                    text: body.to_string(),
                    kinds: None,
                    pagination: None,
                };
                Ok(Some(ApplicationRequest::Search(query)))
            }
            "ingest" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'ingest'. Please upgrade client.");
                let event = brain_integrations::IngestionEvent::Text {
                    content: body.to_string(),
                    metadata: std::collections::BTreeMap::new(),
                };
                let envelope = IngestionEnvelope {
                    event_model_version: "1.0".to_string(),
                    identity: brain_integrations::EventIdentity {
                        event_id: brain_domain::EventId::new(),
                        parent_event_id: None,
                        workspace_id: brain_domain::WorkspaceId::new("uds-legacy"),
                        client_id: brain_domain::ClientId::new("uds-legacy"),
                        adapter_id: brain_domain::AdapterId::new("uds-legacy"),
                        session_id: brain_domain::SessionId::new(),
                        conversation_id: None,
                        timestamp: chrono::Utc::now(),
                    },
                    event,
                };
                Ok(Some(ApplicationRequest::Ingest(envelope)))
            }
            "ingest_event" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'ingest_event'. Please upgrade client.");
                let envelope: IngestionEnvelope = serde_json::from_str(body)
                    .map_err(|e| format!("Failed to parse IngestionEnvelope: {}", e))?;
                Ok(Some(ApplicationRequest::Ingest(envelope)))
            }
            "replay" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'replay'. Please upgrade client.");
                let after_sequence: u64 = body.parse().unwrap_or(0);
                Ok(Some(ApplicationRequest::Replay { after_sequence }))
            }
            "inspect_node" => {
                tracing::warn!("UDS Protocol Router: Received deprecated legacy action 'inspect_node'. Please upgrade client.");
                Ok(Some(ApplicationRequest::InspectNode {
                    id: body.to_string(),
                }))
            }
            "handshake" | "disconnect" => Ok(None),

            _ => Err(format!("Unknown action '{}'", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::protocol::{ClientRequest, LegacyRequest, VersionedRequest};

    fn make_versioned(action: &str, body: &str) -> ClientRequest {
        ClientRequest::Versioned(VersionedRequest {
            version: "1.0".to_string(),
            msg_type: "Request".to_string(),
            id: 1,
            action: action.to_string(),
            body: body.to_string(),
            workspace_context: Vec::new(),
        })
    }

    fn make_legacy(action: &str, payload: &str) -> ClientRequest {
        ClientRequest::Legacy(LegacyRequest {
            action: action.to_string(),
            payload: payload.to_string(),
        })
    }

    #[test]
    fn test_route_versioned_status() {
        let req = make_versioned("v1/status", "");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        assert!(matches!(res, ApplicationRequest::Status));
    }

    #[test]
    fn test_route_versioned_metrics() {
        let req = make_versioned("v1/metrics", "");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        assert!(matches!(res, ApplicationRequest::Metrics));
    }

    #[test]
    fn test_route_versioned_diagnostics() {
        let req = make_versioned("v1/diagnostics", "");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        assert!(matches!(res, ApplicationRequest::Diagnostics));
    }

    #[test]
    fn test_route_versioned_capabilities() {
        let req = make_versioned("v1/capabilities", "");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        assert!(matches!(res, ApplicationRequest::Capabilities));
    }

    #[test]
    fn test_route_versioned_search() {
        let query_json = r#"{"text":"test","kinds":null,"pagination":null}"#;
        let req = make_versioned("v1/search", query_json);
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        if let ApplicationRequest::Search(q) = res {
            assert_eq!(q.text, "test");
        } else {
            panic!("Expected ApplicationRequest::Search");
        }
    }

    #[test]
    fn test_route_versioned_replay() {
        let req = make_versioned("v1/replay", "42");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        if let ApplicationRequest::Replay { after_sequence } = res {
            assert_eq!(after_sequence, 42);
        } else {
            panic!("Expected ApplicationRequest::Replay");
        }
    }

    #[test]
    fn test_route_versioned_inspect_node() {
        let req = make_versioned("v1/inspect_node", "node-123");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        if let ApplicationRequest::InspectNode { id } = res {
            assert_eq!(id, "node-123");
        } else {
            panic!("Expected ApplicationRequest::InspectNode");
        }
    }

    #[test]
    fn test_route_versioned_subscribe() {
        let req = make_versioned("v1/subscribe", "");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        assert!(matches!(res, ApplicationRequest::Subscribe { after_sequence: None }));

        let req_with_seq = make_versioned("v1/subscribe", "123");
        let res_with_seq = ProtocolRouter::route(&req_with_seq).unwrap().unwrap();
        if let ApplicationRequest::Subscribe { after_sequence: Some(seq) } = res_with_seq {
            assert_eq!(seq, 123);
        } else {
            panic!("Expected Subscribe with after_sequence = Some(123)");
        }
    }

    #[test]
    fn test_route_legacy_status() {
        let req = make_legacy("status", "");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        assert!(matches!(res, ApplicationRequest::Status));
    }

    #[test]
    fn test_route_legacy_query() {
        let req = make_legacy("query", "test-legacy");
        let res = ProtocolRouter::route(&req).unwrap().unwrap();
        if let ApplicationRequest::Search(q) = res {
            assert_eq!(q.text, "test-legacy");
            assert!(q.kinds.is_none());
        } else {
            panic!("Expected ApplicationRequest::Search");
        }
    }

    #[test]
    fn test_route_unknown_action() {
        let req = make_versioned("v1/unknown", "");
        let res = ProtocolRouter::route(&req);
        assert!(res.is_err());
    }
}
