# Historical Migration Reports

This directory houses historical documents, transition audits, and codebase cleanup reports. 

> [!IMPORTANT]
> **Historical Archive Only**: The documents in this folder represent historical reports, audit logs, and checklists compiled during major architectural migrations (such as moving from a React/Ink-based TUI to a native Rust/Ratatui immediate-mode architecture). They are kept here for evolutionary context and do not define the current runtime architecture.
>
> For current, active architectural specifications, please refer to the **[Architecture Overview](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/overview.md)** instead.

## Document Index

*   **[native_ratatui_migration_report.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/migration/native_ratatui_migration_report.md)**: Sign-off document tracking the rewrite of the interactive frontend from JavaScript (Ink) to native Rust (Ratatui).
*   **[parity_audit_report.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/migration/parity_audit_report.md)**: Parity checklists, latency benchmarks, and validation logs comparing the old engine behavior against the new native Rust engine.
*   **[purge_report.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/migration/purge_report.md)**: Summary of deleted legacy files, package dependencies removed, and overall codebase cleanup metrics post-migration.
