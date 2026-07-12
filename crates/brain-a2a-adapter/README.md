# Agent-to-Agent (A2A) Adapter

This crate implements the Agent-to-Agent (A2A) protocol adapter for the Brain memory engine, enabling direct communication, capability discovery, and task delegation between autonomous agents.

## Protocol Compatibility Matrix

*   **Supported Protocol Version:** `1.0.0`
*   **Implemented Baseline Methods:**
    *   `handshake`: Performs protocol version handshake and lists supported capabilities.
    *   `agent/message`: Triggers capability execution (e.g. search, ingest) on behalf of the client agent.
*   **Supported Notifications:**
    *   `agent/cancel`: Notifies to cancel the active session run.
    *   `agent/update`: Emits progress steps, diagnostics, and completions back to the caller agent.
*   **Intentionally Unsupported / Deferred:**
    *   None.
*   **Deviations:**
    *   None.
