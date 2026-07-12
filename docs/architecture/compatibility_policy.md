# Compatibility & Versioning Policy

This document defines the compatibility matrix, versioning lifecycle, and deprecation policies for the Brain memory engine across all layer boundaries (Core Runtime, Protocol Adapters, and client SDKs).

---

## 1. Core Architecture Versioning

We version the **Application Interface** (defined in `brain-application` as the stable entry boundary) using Semantic Versioning (SemVer) rules:

| Version Change | Trigger | SemVer Rule | Compatibility Expectation |
| --- | --- | --- | --- |
| **Major (X.0.0)** | Breaking changes to DTO structures or capability execution signatures | `X` increments | Requires upgrading adapters and clients |
| **Minor (1.Y.0)** | Additive changes (new capabilities, optional fields added to existing DTOs) | `Y` increments | Backward-compatible within the same major range |
| **Patch (1.0.Z)** | Bug fixes, internal performance enhancements, or pure internal refactoring | `Z` increments | Fully backward-compatible |

### Compatibility Range rule
Protocol adapters and client SDKs target compatibility ranges:
*   A client targeting version `1.x` is compatible with any runtime exposing an Application Interface version matching `1.x`.

---

## 2. Component SemVer Alignment

To prevent versioning confusion, we enforce the **Interface Alignment Rule**:

```text
TypeScript SDK Major (X.y.z)
      ==
Protocol Adapter Major (X.y.z)
      ==
Application Interface Major (X.y.z)
```

### SDK Compatibility Matrix
*   **SDK v1.x** is guaranteed to compile and execute against **Application Interface v1.x** runtimes.
*   If the Application Interface undergoes a major breaking change to `v2.0.0`, a corresponding **SDK v2.x** and **Adapter v2.x** suite must be released to handle the updated contracts.

---

## 3. Capability Negotiation

Rather than negotiating independent version tables for each capability (which introduces high operational complexity), adapters perform **Presence-Based Negotiation** during handshake and initialization:

1.  **Handshake Payload**: Adapters advertise the global `applicationInterface` version string (e.g. `"1.0.0"`) and a simple flat list of available capabilities (e.g. `["search", "ingest", "workflow"]`).
2.  **Compatibility Resolution**: The caller client verifies that the runtime's interface version satisfies its compatibility range (e.g. `1.x`) and checks that the specific capability it needs is present in the advertised list.

> [!TIP]
> **Future Evolution — Implemented vs. Enabled**:
> If the system expands to include a plugin ecosystem or policy-driven enterprise deployments, we may differentiate between "implemented" capabilities and "enabled" capabilities. For example, a capability like `administration` or `workflow` might be implemented in the codebase but disabled by deployment policies. In this scenario, the negotiation response should advertise the status of each capability (e.g., `{"name": "workflow", "enabled": false}`) rather than just presence.

---

## 4. API Deprecation Window Policy

When evolving the Application Interface:
*   **Minor Additions**: Optional parameters are preferred over breaking contract revisions to ensure backward compatibility.
*   **Deprecation Cycle**: Deprecated parameters or capabilities must be supported for at least **two minor versions (N-2)** before they can be completely removed in a subsequent major version release.
*   **Documentation first**: Deprecation notices must be explicitly documented in changelogs and code comments before any runtime deprecation mechanisms are introduced.
