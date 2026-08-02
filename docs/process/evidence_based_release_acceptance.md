# RFC 001: Evidence-Based Release Acceptance (EBRA)

- **Status**: Active Reference Specification
- **Version**: 1.0.0
- **Category**: Engineering Process & Release Quality
- **Date**: 2026-08-02

---

## 1. Abstract

This specification defines **Evidence-Based Release Acceptance (EBRA)**, a technology-agnostic reference methodology and evidence model for release acceptance testing, post-incident investigations, and engineering quality evaluations.

EBRA establishes strict epistemic boundaries separating dynamic runtime execution, static artifact inspection, verified facts, diagnostic hypotheses, and engineering assessments. It enforces a unidirectional flow of reasoning to ensure that release decisions emerge strictly from reproducible evidence rather than intuition, implementation familiarity, or hindsight bias.

> **Scope Note**: EBRA is a reference methodology for general software product acceptance. It is not a substitute for domain-specific regulatory, safety, or compliance frameworks (e.g., IEC 62443, DO-178C, FDA SaMD). Such frameworks may extend, but should not weaken, the evidentiary principles defined here.

---

## 2. Motivation & Problem Statement

Software engineering organizations frequently experience a disconnect between **Implementation Quality** (code architecture, unit test coverage, domain invariants) and **Release Quality** (packaging integrity, onboarding ergonomics, documentation accuracy).

Traditional release testing often fails to discover critical onboarding defects because it is performed by engineers who possess deep implementation context. Evaluators unconsciously bypass setup friction, supply missing environment variables, and forgive ambiguous CLI output. Furthermore, traditional bug reports frequently blur the distinction between observed execution data and diagnostic hypothesis, causing remediation teams to investigate unverified root-cause paths.

---

## 3. Goals and Non-Goals

### 3.1 Goals
- **Epistemic Discipline**: Establish clean, enforceable boundaries between observations, verified facts, hypotheses, and release judgments.
- **Reproducibility**: Ensure every finding is backed by immutable, timestamped execution or inspection evidence.
- **Enforced Naivety**: Eliminate evaluator bias by evaluating production release artifacts without prior source code inspection.
- **Traceability**: Link findings, evidence, facts, assessments, and release decisions via explicit identifiers.

### 3.2 Non-Goals
- EBRA **MUST NOT** be used as a replacement for automated continuous integration (CI) or unit test suites.
- EBRA **MUST NOT** be used as a substitute for formal security audits, cryptographic reviews, or regulatory compliance (e.g., medical, avionics, finance).
- EBRA **MUST NOT** be used as an internal code debugging methodology.

---

## 4. Terminology

| Term | Definition |
|---|---|
| `OBS-xxx` | **Observed Runtime Evidence**: Verbatim stdout, stderr, exit codes, and timestamps recorded during dynamic execution. |
| `INS-xxx` | **Inspection Evidence**: Static properties verified by direct examination of source files, configuration manifests, build scripts, or release archives. |
| `CFG-xxx` | **Configuration Evidence**: Reproducibility metadata capturing the exact environment at evaluation time — toolchain versions, OS, feature flags, environment variables, locale, plugin list, and dependency lock file hashes. Not runtime observation and not artifact inspection; it is the precondition snapshot that makes a report reproducible. |
| `VF-xxx` | **Verified Fact**: A statement directly supported by one or more evidence items without introducing assumptions, causal reasoning, or interpretation beyond what the evidence entails. |
| **Likely Cause** | A diagnostic hypothesis offering the best-supported technical explanation for an observation. Always optional. |
| `ASMT-xxx` | **Assessment**: Structured evaluation of severity, priority, user impact, and actionable recommendations. |
| `FIND-xxx` | **Finding**: A traceable, self-contained record grouping metadata, evidence, facts, cause analysis, and assessment. |
| **Enforced Naivety** | The practice of black-box evaluation without prior inspection of implementation code, simulating a first-time user path. |

---

## 5. Normative Language

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **MAY**, and **OPTIONAL** in this document are to be interpreted as described in BCP 14, [RFC 2119](https://tools.ietf.org/html/rfc2119) and [RFC 8174](https://tools.ietf.org/html/rfc8174).

---

## 6. Principles

1. **Observe before explaining**: Evaluators **MUST** record execution output prior to diagnostic root-cause analysis.
2. **Record before investigating**: Evaluators **MUST** capture exact stdout, stderr, and exit codes before inspecting code or configurations.
3. **Separate evidence from inference**: Evaluators **MUST NOT** present a diagnostic hypothesis as an observed fact.
4. **Separate inference from assessment**: Evaluators **MUST** keep evidence and technical hypotheses distinct from severity ratings, priority tags, and release decisions.
5. **Declare scope, preconditions, and assumptions explicitly**: Evaluators **MUST** declare evaluated scope, preconditions, and baseline assumptions.
6. **State confidence when reasoning beyond evidence**: Evaluators **MUST** explicitly rate certainty (`High`, `Medium`, `Low`) whenever proposing an unverified hypothesis.
7. **Judge the released product, not the implementation**: Evaluators **MUST** evaluate the user experience from production release artifacts, independent of internal code quality.
8. **Prefer reproducible observations over intuition**: Every finding **MUST** be traceable to discrete evidence IDs.
9. **Preserve evidence integrity**: Evaluators **MUST** preserve original evidence items exactly as captured. Corrections or interpretations **MUST NOT** overwrite original observations.

---

## 7. Evidence Model

Information flows strictly downward. Release decisions **SHALL** be justified by documented evidence, verified facts, and explicit engineering assessments.

```text
Configuration Evidence (CFG-xxx) [Environment snapshot at T_start]
            │
            ├─────────────────────────────────────────────────────────┐
            │                                                         │
Observed Runtime Evidence (OBS-xxx) [Captured at T_timestamp]        │
            │                                                         │
            ├──────────────► Verified Facts (VF-xxx) ◄───────────────┘
            │                     │
Inspection Evidence (INS-xxx) ────┘
                                  │
                                  ▼
                    Likely Cause [Optional] (Hypothesis + Confidence)
                                  │
                                  ▼
                    Assessment (ASMT-xxx) (Severity, Priority, Impact)
                                  │
                                  ▼
                    Release Decision (Criteria-Gated)
```

Configuration Evidence anchors the entire report to a specific environment snapshot. Evidence items **SHOULD** be ordered chronologically within a finding to enable exact replay.

---

## 8. Traceability Model

Every element in an EBRA report **MUST** use immutable, report-unique identifiers:

| Prefix | Element |
|---|---|
| `CFG-xxx` | Configuration Evidence item (environment snapshot) |
| `OBS-xxx` | Observed Runtime Evidence item |
| `INS-xxx` | Inspection Evidence item |
| `VF-xxx` | Verified Fact item |
| `ASMT-xxx` | Assessment item |
| `FIND-xxx` | Container Finding |

### Immutability & Reference Rules
1. Identifiers **MUST** be immutable within a report.
2. If an observation is superseded by a subsequent test run, the original identifier **MUST NOT** be deleted or reused; the new item **MUST** explicitly reference the original identifier (e.g., `OBS-008 supersedes OBS-002`).

---

## 9. Rules

### Rule 1: Enforced Naivety
The evaluator **MUST NOT** inspect implementation source code to determine how a feature should work prior to attempting its use. Implementation inspection is permitted only *after* observed runtime behavior has been recorded.

### Rule 2: Epistemic Neutrality in Evidence
Evaluative terms (`broken`, `flawed`, `buggy`, `incorrect`) **MUST NOT** appear in Evidence sections. Evidence sections **MUST** contain only neutral, verbatim execution outputs, exact commands, timestamps, and quantitative measurements.

### Rule 3: Boundary Declarations
Every acceptance report **MUST** include four explicit boundary sections:

| Section | Contents |
|---|---|
| **In Scope** | Components, interfaces, and user flows evaluated. |
| **Out of Scope** | Explicitly excluded audits (e.g., security penetration, regulatory compliance). |
| **Preconditions** | Initial system state required before test execution (e.g., clean database, port unallocated). |
| **Assumptions** | Preserved baseline invariants the evaluator accepts without verification (e.g., standard OS filesystem permissions, no pre-existing daemon). |

### Rule 4: Re-test after Fix
Every finding closed by a code or configuration change **MUST** be revalidated using the **original reproduction steps** from its Evidence Trail. New test cases written to cover a fix **MUST NOT** substitute for rerunning the original reproduction.

> **Rationale**: A fix may cause new tests to pass while the original defect remains. The only confirmation that the original reported behavior is resolved is re-executing the original reproduction steps and observing the expected output.

---

## 10. Evidence Preservation

Evidence items, once captured, are immutable records of what occurred during evaluation. Their integrity is essential to the traceability model.

### Retention

Runtime evidence **SHOULD** be retained in its original form whenever practical. Examples of retainable evidence:

- Terminal transcripts (stdout, stderr, exit codes)
- Shell session recordings
- Screenshots or screen recordings of UI behavior
- Tarball manifests (`tar -tzf` output)
- HTTP response bodies and status codes
- Log file excerpts (timestamped, verbatim)
- Package contents or file checksums

### Referencing

Evidence **SHOULD** be referenced by identifier (`OBS-xxx`, `INS-xxx`) rather than rewritten in findings. Findings **MUST NOT** paraphrase evidence in a way that changes its meaning.

### Immutability

Evidence **MUST NOT** be modified after capture. If a subsequent test run produces different results, a new evidence item **MUST** be created with a new identifier (e.g., `OBS-008 supersedes OBS-002`). The original item **MUST** remain in the report.

### Attachment

Evidence **MAY** be attached as artifacts (files, screenshots, session transcripts) linked from the report. Attachment is encouraged when the evidence is too large to embed inline.

### Configuration Evidence (`CFG-xxx`)

Every EBRA report **SHOULD** include at least one `CFG-xxx` item capturing the complete environment snapshot at the start of the evaluation session. This is distinct from runtime evidence (what happened) and inspection evidence (what an artifact contains) — it records the conditions under which the report was produced.

Recommended fields:

| Field | Example |
|---|---|
| Operating system & version | `macOS 15.2 (arm64)` |
| Rust toolchain | `rustc 1.81.0 (stable)` |
| `Cargo.lock` hash | `sha256:a3f9...` |
| Python version | `3.12.4` |
| Shell & version | `zsh 5.9` |
| Terminal emulator | `iTerm2 3.5.2` |
| Locale | `en_US.UTF-8` |
| Relevant environment variables | `BRAIN_SOCKET_PATH`, `PYO3_PYTHON` |
| Active feature flags | `["streaming", "consensus"]` |
| Installed plugins | `[]` |

`CFG-xxx` items **MUST NOT** be modified after capture. They exist to enable exact reproduction of the evaluation environment.

---

## 11. Finding Specification

Every finding **MUST** conform to the following template:

```markdown
### FIND-00X: [Short Neutral Title]

#### 1. Metadata
- **Component**: [CLI | Daemon | SDK | Documentation | Packaging]
- **Reproducibility**: [Always | Intermittent (X/Y runs) | Unknown]

#### 2. Evidence Trail

##### Observed Runtime Evidence (OBS-001)
- **Timestamp**: `2026-08-02T11:26:24Z`
- **Environment**: macOS arm64 (Apple Silicon), Rust 1.80.0
- **Preconditions**: No `brain-daemon` running in background.
- **Execution Command**:
  ```bash
  ./brain daemon start
  ```
- **Expected Output**:
  ```
  Daemon started successfully (PID: 1234).
  ```
- **Actual Output (Verbatim)**:
  ```
  Error: brain-daemon executable could not be found.
  Exit Code: 1
  ```

##### Inspection Evidence (INS-001)
- **Timestamp**: `2026-08-02T11:26:25Z`
- **Artifact Inspected**: `package.sh` and `brain-darwin-arm64.tar.gz`
- **Observation**:
  ```bash
  tar -tzf brain-darwin-arm64.tar.gz
  # Output: brain, INSTALL.md, README.md, CHANGELOG.md (brain-daemon absent)
  ```

##### Verified Facts
- **VF-001**: The release archive contains `brain` but not `brain-daemon`. (Supported by: `INS-001`)
- **VF-002**: Executing `./brain daemon start` from the release archive produces exit code 1. (Supported by: `OBS-001`)

#### 3. Likely Cause (Optional)
*(If no evidence-supported hypothesis exists, record **Unknown** rather than speculating.)*
- **Hypothesis**: The packaging script was authored prior to decoupling the daemon into a standalone binary crate.
- **Confidence**: High

#### 4. Assessment (ASMT-001)
- **Severity**: Critical (Blocks core system functionality)
- **Priority**: P0 (Release Blocker — binary archive non-functional for primary workflow)
- **Impact**: First-time users downloading the official release archive cannot start or interact with the background memory engine.
- **Recommendation**: Update `package.sh` to copy both `brain` and `brain-daemon` into the distribution directory before archiving.
```

---

## 12. Assessment & Epistemic Confidence Model

### Severity vs. Priority

These **MUST** be evaluated independently:

| Axis | Definition | Scale |
|---|---|---|
| **Severity** | Technical depth of impact on system capability or stability | `Critical`, `High`, `Medium`, `Low` |
| **Priority** | Scheduling urgency for remediation | `P0` (release blocker), `P1` (public quality), `P2` (enhancement), `P3` (cosmetic) |

A finding may be `High Severity / P2 Priority` (e.g., a severe edge case affecting <1% of users) or `Low Severity / P0 Priority` (e.g., a minor documentation error that blocks first-time user onboarding).

### Epistemic Confidence Scale

When proposing a diagnostic hypothesis under **Likely Cause**, evaluators **MUST** assign one of:

| Rating | Meaning |
|---|---|
| **High** | Supported directly by multiple evidence items with no competing plausible explanation. |
| **Medium** | Supported by evidence, but alternative explanations remain plausible. |
| **Low** | Limited supporting evidence or multiple competing explanations exist. |
| **Unknown** | No evidence-backed hypothesis available. Use this rather than speculating. |

---

## 13. Decision Framework

### Objective Release Decision Criteria

| Decision | Criterion |
|---|---|
| **Ready** | Zero open P0/P1 findings; all in-scope areas verified. |
| **Ready with Minor Issues** | Zero open P0/P1 findings; remaining findings are documented P2/P3 issues. |
| **Needs Testing** | Execution evidence is incomplete or blocked by missing preconditions. |
| **Not Ready** | At least one confirmed P0 or P1 finding remains unresolved. |

---

## 14. Quality Axes

EBRA distinguishes three orthogonal quality axes. A product may score differently on each, and a deficiency on any one axis is independently sufficient to block release.

| Axis | Evaluated By | Examples |
|---|---|---|
| **Implementation Quality** | Code review, unit tests, architecture audits | Test coverage, domain invariants, API correctness, algorithmic soundness |
| **Release Quality** | EBRA acceptance testing (Enforced Naivety) | Package integrity, documentation accuracy, CLI ergonomics, first-run experience |
| **Operational Quality** | Post-deployment evaluation, runbook review | Upgrade paths, data migrations, backup and restore, observability, restart behavior, failure recovery, rolling deployment |

### Axis Independence

These axes are **independent**. A product may be:
- **High Implementation / Low Release**: Well-engineered code that is unshippable due to packaging or documentation failures.
- **High Release / Low Operational**: A product that onboards correctly but has no defined upgrade or recovery path.
- **High Operational / Low Implementation**: A product with mature operational runbooks but accumulating internal technical debt.

EBRA acceptance testing primarily evaluates **Release Quality**. Operational Quality evaluation requires separate runbook and operational acceptance testing, which is outside EBRA's direct scope but is referenced here to prevent conflation with Release Quality.

A project may score **High Implementation Quality / Low Release Quality** — well-engineered code that is nonetheless unshippable due to packaging or documentation failures. EBRA evaluates the release axis directly; operational quality evaluation is a recommended companion process.

---

## 15. Conformance

An acceptance report **CONFORMS** to EBRA 1.0 if and only if all of the following requirements are met:

- [ ] **Scope, Preconditions, and Assumptions** are explicitly declared.
- [ ] **Enforced Naivety** rule was maintained during initial testing (Rule 1).
- [ ] At least one **Configuration Evidence** item (`CFG-xxx`) capturing the evaluation environment is present (Section 10).
- [ ] All **Evidence Items** (`OBS-xxx`, `INS-xxx`, `CFG-xxx`) use report-unique, immutable identifiers.
- [ ] Evidence sections contain **zero evaluative or subjective language** (Rule 2).
- [ ] All **Verified Facts** (`VF-xxx`) reference at least one supporting Evidence ID without introducing unverified assumptions.
- [ ] Every **Hypothesis** has an explicit **Epistemic Confidence Rating** (`High`, `Medium`, `Low`, or `Unknown`).
- [ ] **Severity** and **Priority** are evaluated separately under Assessment (`ASMT-xxx`).
- [ ] The **Release Decision** strictly matches the criteria table in Section 13.
- [ ] Every closed finding has been **revalidated using its original reproduction steps** (Rule 4).
- [ ] Evidence items are referenced by identifier and have not been modified after capture (Section 10).
- [ ] A **Threats to Validity** section is present, identifying environmental factors that may limit generalization (Section 18).

---

## 16. Applicability

EBRA is applicable to:
- Pre-release acceptance testing for public or internal software releases.
- Post-incident root-cause investigations and postmortems.
- Regression testing and bug report verification.
- Developer experience (DX) and SDK onboarding audits.
- API, service, and infrastructure release gates.

---

## 17. Limitations

EBRA **MUST NOT** be considered a substitute for:
- Automated CI/CD unit or integration testing suites.
- Security penetration audits or cryptographic verification.
- Regulatory compliance frameworks (medical, avionics, finance).

---

## 18. Threats to Validity

Acceptance conclusions drawn from an EBRA report are valid evidence about the **tested configuration**, not necessarily about every possible deployment environment. Every report **SHOULD** include a Threats to Validity section identifying factors that may limit the generalizability of its findings.

### Environmental Scope

Findings are bounded by the evaluation environment. Factors that may affect reproducibility or generalization include:

| Factor | Examples |
|---|---|
| Operating system & version | macOS vs. Linux; kernel version; filesystem semantics (case-sensitivity, extended attributes) |
| CPU architecture | `arm64` vs. `x86_64`; endianness |
| Toolchain versions | Rust compiler version; Python interpreter; linker |
| Dependency versions | `Cargo.lock` state; transitive dependency updates |
| Shell & terminal | `zsh` vs. `bash`; terminal emulator; Unicode support; color depth |
| Locale & encoding | `en_US.UTF-8` vs. `C`; timezone |
| Network topology | Air-gapped vs. connected; proxy configuration |
| Installed plugins | Plugin list may alter behavior or output |
| Feature flags | Enabled/disabled features change the code path under test |
| Environment variables | Variables that alter runtime behavior (`BRAIN_SOCKET_PATH`, etc.) |
| Available system resources | RAM, disk, open file descriptor limits |

### What Threats to Validity Does Not Mean

A Threats to Validity section does **not** weaken the findings. It precisely scopes them. A finding observed on macOS arm64 is a confirmed finding on macOS arm64. The section informs readers about which configurations remain untested and require independent verification.

### Recommended Format

```markdown
## Threats to Validity

**Tested configuration**: macOS 15.2 arm64, Rust 1.81.0, zsh 5.9, en_US.UTF-8.

**Factors not tested**:
- Linux (x86_64) — packaging, file path, and socket behavior may differ.
- Windows — not in scope for this release.
- Non-UTF-8 locales — terminal output rendering untested.
- Networked daemon deployments — only local UDS socket tested.
```

---

## 19. Future Extensions

Future versions of this specification **MAY** introduce additional evidence classes (e.g., telemetry traces, network packet captures, performance profiles) provided they preserve the unidirectional evidence hierarchy and normative traceability rules defined in this RFC.
