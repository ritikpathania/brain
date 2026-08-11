# Claude Code vs. Brain TUI — Gap Analysis & Recommendations
> Research Audit Phase 10 · 2026-08-10
> All recommendations strictly categorized: **ADOPT**, **ADAPT**, **PRESERVE**, **REJECT**, or **INVESTIGATE**.

---

## 1. Summary of Classifications

| Recommendation | Classification | Rationale & Evidence |
|---|---|---|
| Reflowing slash completion directly below prompt | **ADAPT** | Matches Claude's interaction grammar without altering prompt anchor |
| Unseen message divider line when scrolled up | **ADOPT** | High usability value during active response streaming |
| Warm coral accent & muted gray typography | **ADAPT** | Achieves Claude-family visual calm while retaining `ThemeToken` authority |
| Single rounded containers & quiet horizontal rules | **ADOPT** | Reduces visual clutter; eliminates card-dashboard boxes |
| Hook-driven scriptable status line command | **REJECT** | Violates Brain's internal domain state control for latency/metrics |
| 5-hour rate-limit & billing meter bars | **REJECT** | Irrelevant to Brain's local relational memory architecture |
| Multi-agent sub-agent identity color rotations | **REJECT** | Brain is built on a unified single-session cognitive model |
| Bottom-anchored Home prompt line | **REJECT** | Brain's ~67% height prompt clamping on tall screens is ergonomically superior |
| Brain Mascot, BRAIN identity & Relational Engine | **PRESERVE** | Core native product identity; non-negotiable |
| 3-Pane Graph Exploration & Node Inspector | **PRESERVE** | Brain-native strength for memory graph analysis |
| Typewriter Queue chunk pacing | **PRESERVE** | Ensures smooth microsecond stream rendering without tearing |
| Vim mode input bindings | **INVESTIGATE** | Future roadmap item requiring input parser work |

---

## 2. Detailed Gap Breakdown

### Gap 1: Slash Completion Positioning & Reflow

- **Classification**: **ADAPT**
- **Claude Evidence** (`OBSERVED` & `SOURCE-CONFIRMED` in `PromptInputFooterSuggestions.tsx`):
  When typing `/`, the suggestions list renders **directly below the prompt line** with `content reflowing upward` and the prompt remaining the visual interaction anchor.
- **Brain Current Behavior**:
  Slash completion overlay list is rendered below the prompt line (`palette_area`), with status line hidden while completion owns the lower screen region.
- **Proposed Change**:
  Keep completion below prompt; ensure exact reflow alignment and cursor isolation.
- **Architectural Impact**: Presentation layer only (`renderer.rs`). Zero domain changes.
- **Risk**: Very low.
- **Contract Violation**: None.

---

### Gap 2: Visual Restraint & Box Border Rules

- **Classification**: **ADAPT**
- **Claude Evidence** (`SOURCE-CONFIRMED` in `DESIGN.md`):
  Prefers single coherent rounded containers (`BorderType::Rounded`), quiet horizontal rules (`─`), and generous whitespace over multi-nested panel boxes.
- **Brain Current Behavior**:
  Uses clean single rounded containers (`panel.rs`, `input.rs`).
- **Proposed Change**:
  Avoid adding nested card frames around inner widgets. Let whitespace dictate section hierarchy.
- **Architectural Impact**: Presentation layer only (`ui/widgets/`).
- **Risk**: Low.
- **Contract Violation**: None.

---

### Gap 3: Status Footer Customization & Rate Limits

- **Classification**: **REJECT**
- **Claude Evidence** (`SOURCE-CONFIRMED` in `StatusLine.tsx`):
  Claude allows executing external shell hook commands (`settings.statusLine.command`) and renders 5-hour / 7-day rate-limit progress meters.
- **Brain Current Behavior**:
  Brain status footer (`status_footer.rs`) displays UDS daemon connectivity, workspace status, query latency in ms, and indexed memory counts.
- **Proposed Change**:
  Do NOT copy Claude's hook script or billing bars. Keep Brain's internal domain status semantics.
- **Architectural Impact**: N/A (Rejected).
- **Risk**: High if adopted (violates domain control).
- **Contract Violation**: Yes if adopted.

---

### Gap 4: Brain Native Product Identity

- **Classification**: **PRESERVE**
- **Claude Evidence**: N/A (Claude-specific branding).
- **Brain Current Behavior**:
  Brain mascot, BRAIN identity, Relational Memory Engine tagline ("Think once. Remember forever."), memory context metrics, and graph explorer.
- **Proposed Change**:
  Preserve 100%. Never replace Brain branding with Claude branding.
- **Architectural Impact**: None.
- **Risk**: Zero.
- **Contract Violation**: N/A.
