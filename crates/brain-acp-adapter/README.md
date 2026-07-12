# Agent Client Protocol (ACP) Adapter

This crate implements the Agent Client Protocol (ACP) adapter for the Brain memory engine.

## Protocol Compatibility Matrix

*   **Supported Protocol Version:** `2024-11-05`
*   **Implemented Baseline Methods:**
    *   `initialize`: Performs protocol version capability handshake.
    *   `session/new`: Generates or initializes a stateful conversation session.
    *   `session/prompt`: Dispatches client prompts dynamically through the generic capability registry.
*   **Supported Notifications:**
    *   `session/cancel`: Safely interrupts active prompt runs by triggering the task cancellation token.
    *   `session/update`: Dispatches real-time progress update events back to the client editor.
*   **Intentionally Unsupported Methods:**
    *   `authenticate` / `logout`: Authentication and user credential verification are deferred.
*   **Intentionally Deferred Capabilities:**
    *   `session/load`: Reloading and restoring historic session states from disk is deferred.
    *   `session/set_mode`: Switch between agent operating modes (e.g. planner/exec) is deferred.
*   **Deviations:**
    *   None. The JSON-RPC 2.0 implementation conforms strictly to the standard stdio message envelope.
