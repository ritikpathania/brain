# Security Policy

The `brain` team takes the security of our relational memory engine, background IPC daemon, and client integration SDKs seriously.

---

## Supported Versions

We provide security updates for the current major release version.

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0.0 | :x:                |

---

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

If you discover a potential security vulnerability in `brain`, please report it privately:

1. **Email**: Send an email to `security@brain-engine.dev` (or open a private security advisory on GitHub if enabled).
2. **Details**: Include as much of the following as possible:
   - Type of issue (e.g., buffer overflow, IPC authorization bypass, process privilege escalation, unsafe deserialization).
   - Step-by-step instructions or proof-of-concept code to reproduce the issue.
   - Affected crate or module (e.g., `brain-daemon`, `brain-storage`, `brain-services`).
   - Potential impact of the vulnerability.

---

## Response Timeline

- **Initial Response**: Within 48 hours of receiving the report.
- **Triage & Status Update**: Within 7 business days.
- **Fix & Advisory Release**: Coordinated fix disclosure within 30 days depending on severity.

---

## Security Principles

- **Zero Privilege Escalation**: The daemon (`brain-daemon`) operates strictly as a local, non-root user process listening on a Unix Domain Socket with restricted permissions (`0600`).
- **Input Validation**: All IPC payloads undergo strict JSON Schema validation before reaching the runtime layer.
- **Memory Safety**: Core logic is implemented in safe Rust. `unsafe` blocks are restricted, documented, and audited.
