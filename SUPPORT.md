# Support & Community Guide

Thank you for using the Brain Relational Memory Engine! Here is how to get help, ask questions, or report bugs.

---

## 📚 Documentation & Guides

Before opening an issue, check the official documentation:

* **[Documentation Overview](docs/README.md)**: Main entry point for all architectural and subsystem documentation.
* **[Installation Guide](docs/guides/installation.md)**: Prerequisites, build setups, and package initialization.
* **[Architecture Specification](docs/architecture/overview.md)**: Deep dive into the runtime lifecycle, projections, and storage engine.
* **[IPC Wire Protocol Reference](docs/reference/protocol.md)**: UDS socket frames and JSON RPC message definitions.

---

## 💬 Community & Discussions

* **[GitHub Discussions](https://github.com/ritikpathania/brain/discussions)**: Best place for general Q&A, design ideas, feature brainstorming, and integration help.
* **[GitHub Issues](https://github.com/ritikpathania/brain/issues)**: Use for reporting confirmed bugs or submitting detailed feature proposals.

---

## 🐛 Reporting Bugs

If you encounter unexpected behavior or errors:

1. Check existing issues on the **[Issue Tracker](https://github.com/ritikpathania/brain/issues)** to avoid duplicates.
2. Use the **[Bug Report Template](.github/ISSUE_TEMPLATE/bug_report.yml)**.
3. Include relevant system details:
   - Operating system and version (macOS / Linux)
   - Rust toolchain version (`rustc --version`)
   - Logs or output from `cargo xtask verify` or `brain daemon status`
