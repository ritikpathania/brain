# Brain test readiness

Use the [test matrix](TEST_INFRA.md) to select checks for each change. Review the [invariant ledger](REGRESSION-PREVENTION-AND-INVARIANTS.md) for existing regressions and use [GitHub Actions](https://github.com/ritikpathania/brain/actions/workflows/ci.yml) for results on the exact commit.

| Gate | Readiness |
| --- | --- |
| Rust workspace and architecture | Automated through `cargo xtask verify`. Local daemon tests need Unix sockets and some fixtures need loopback. The timing-sensitive extraction tests can be checked again with lower nextest concurrency if a loaded local runner exceeds the existing limit. |
| Frontend unit, contract and E2E | Owned tests and typecheck run with the pinned Bun version. Real-daemon tests require CLI and daemon binaries and access to PTYs and local sockets. |
| SDKs and Python | SDK tests use local fixtures. The Rust, TypeScript and Python suites are separate entry points. Daemon Python integration needs a built daemon and local sockets. |
| Native archives | Linux package verification passed on `8983119a`. macOS packaging on the same source was blocked on the first attempt by a SQLite pool timeout in a stress test and on retry by the existing 200,000-character extraction limit. Verify both platforms for the exact next release candidate. [CI evidence](https://github.com/ritikpathania/brain/actions/runs/44330425107) |
| Documentation | `cargo xtask docs all` checks links, index coverage, required handbooks, frontmatter and protocol examples. |

For a new phase, record the tested commit, checks run, excluded or blocked cases, and the evidence link before signing off.
