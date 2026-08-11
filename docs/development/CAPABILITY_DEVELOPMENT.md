# Capability Development Policy

> **One-line rule**: Every roadmap item is a user capability, not a framework addition.

This document is the practical governance guide for the capability phase of the project.
It lives alongside [ADR 0001](../adr/0001-tui-search-and-presentation-pipeline.md), which
defines the architectural boundaries.  This document defines how new work is scoped,
reviewed, and shipped within those boundaries.

---

## 1. The Four Rules

### Rule 1 — Every roadmap item is a user capability

Before any work begins, the item must answer:

> **What can the user do that they could not do before?**

Not:

> "What new abstraction are we adding?"

Roadmap items are expressed as capabilities:

| ✅ Capability framing | ❌ Framework framing |
|---|---|
| Knowledge Explorer | Extend ViewModel |
| Pinned Memories | Add GroupingStrategy enum |
| Semantic Filters | Refactor AggregationEngine |
| Timeline View | Add Projection stage variant |

If an item cannot be expressed as a user capability, it is framework work.
Framework work requires a stronger justification (see Rule 3).

---

### Rule 2 — Every capability identifies its primary pipeline stage

The [five-stage pipeline](../adr/0001-tui-search-and-presentation-pipeline.md) already
covers every foreseeable capability.  Each new capability must declare one **primary
stage** — the stage whose contract it extends most directly.

```
Retrieval → Aggregation → Projection → Grouping → Rendering
```

A capability may touch multiple stages, but it has exactly one primary owner.

**Reference mapping** (living — update as capabilities are added):

| Capability              | Primary Stage | Secondary Stage |
|-------------------------|---------------|-----------------|
| Knowledge Explorer      | Aggregation   | Projection      |
| Pinned Memories         | Grouping      | Rendering       |
| Semantic Filters        | Aggregation   | Retrieval       |
| Knowledge Relationships | Retrieval     | Projection      |
| Timeline View           | Projection    | Rendering       |
| Memory Editing          | Retrieval     | Projection      |
| Conversation Branching  | Retrieval     | Grouping        |
| Rich Retrieval          | Retrieval     | Aggregation     |

**Pipeline Ownership Rule**: a capability extends one primary stage.
If you find yourself saying "this capability touches every stage equally,"
that is a signal to decompose it into smaller, independently shippable capabilities.

---

### Rule 3 — Architectural changes require an ADR only when they alter documented invariants

An ADR is **required** when a change does any of the following:

- Adds a new pipeline stage
- Removes a pipeline stage
- Moves ownership of a responsibility between stages
- Changes a documented stage invariant
- Changes a cross-stage contract (e.g. the shape of `MemoryResultViewModel`)

An ADR is **not required** when a change:

- Adds a new capability within an existing stage's responsibilities
- Adds new fields to an existing type (additive, non-breaking)
- Adds or updates behavioral tests
- Refactors internals within a single stage without changing its public contract

When in doubt: if the change would make the existing ADR 0001 diagram inaccurate,
an ADR is required.  If the diagram remains accurate, it is not.

---

### Rule 4 — Every capability ships with black-box behavioral tests

Tests follow the **Arrange → Act → Assert** pattern and verify externally
observable behavior only:

- No raw UUIDs appear in rendered output
- No unresolved `Option` values reach the UI
- Placeholder text appears exactly where expected
- Ordering is stable across redraws and state changes
- Keyboard navigation reaches the correct target

Tests must not assert internal implementation details (which function was called,
which struct was instantiated).  They must survive a full internal refactor as
long as the architectural contract is preserved.

#### Acceptance criteria template

Every capability PR must include acceptance criteria in this form:

```text
Arrange
-------
[Seed state: sessions, memories, pins, etc.]

Act
---
[Keyboard or API interaction the user would perform]

Assert
------
• [Observable outcome 1]
• [Observable outcome 2]
• [Observable outcome 3]
```

#### Examples

**Knowledge Explorer**
```text
Arrange
-------
Seed related knowledge nodes across three concept clusters.

Act
---
Open Knowledge Explorer (/explore or equivalent).

Assert
------
• Related memories are grouped by cluster.
• Group ordering is stable across redraws.
• Keyboard navigation moves between groups and items.
• No UUIDs appear in rendered output.
• Selecting an item opens the correct detail view.
```

**Pinned Memories**
```text
Arrange
-------
Pin three memories in a known order.

Act
---
Restart the application.

Assert
------
• All three pins persist after restart.
• Pin order is preserved exactly.
• Unpinning a memory removes it from the pinned group immediately.
• The unpinned memory appears in its natural group position.
```

**Semantic Filters**
```text
Arrange
-------
Seed memories across multiple topics.

Act
---
Apply a topic filter in the search interface.

Assert
------
• Only memories matching the filter appear.
• Removing the filter restores the full result set.
• Result ordering within the filtered set is stable.
• Filter state persists across theme changes and redraws.
```

---

## 2. The Capability Phase

The project lifecycle is:

```
Architecture  ←── ADR-driven, now complete
      ↓
Framework     ←── Pipeline established, now stable
      ↓
Verification  ←── E2E behavioral suite, now in place
      ↓
Capabilities  ←── Current phase
      ↓
Product
```

**Architecture should now be mostly invisible.**

If a capability implementation requires frequent changes to framework code,
that is a signal to pause and ask:

> Are the existing abstractions still holding up?

If yes: the implementation approach needs rethinking.
If no: an ADR conversation is warranted before any code changes.

The goal is that capabilities are added *on top of* the pipeline,
not *through* it.

---

## 3. Warning Signs

The following patterns indicate the policy is being violated:

| Pattern | What it signals |
|---|---|
| PR touches all five pipeline stages | Capability is too large; decompose it |
| PR description says "refactor X" with no user capability | Framework work without justification |
| New type added with no corresponding capability | Premature abstraction |
| Tests assert internal call sequences | Tests are coupled to implementation, not behavior |
| ADR written for a single-stage addition | Over-process; ADR not needed |
| No ADR written for a stage removal | Under-process; ADR required |

---

## 4. Product Health Metrics

As the project moves through the capability phase, track a small set of
**product-oriented** metrics alongside engineering quality gates.
These become a better indicator of project health than the number of architectural
documents or passing tests alone.

| Metric | Stage it measures | Why it matters |
|---|---|---|
| Search success rate | Retrieval | Are users finding what they look for? |
| Median retrieval latency (p50/p95) | Retrieval + Aggregation | Is the pipeline fast enough to feel instant? |
| Keyboard-only task completion rate | Rendering + Navigation | Can users operate entirely without a mouse? |
| Time to find a memory | Full pipeline | End-to-end discoverability |
| E2E behavioral test pass rate | All stages | Are architectural contracts holding? |

### How to use these

- Track them per capability milestone, not per commit.
- A declining metric after a new capability ships is a signal that the capability
  regressed an existing contract — investigate before the next milestone.
- A metric that cannot be measured yet is a signal that the capability does not
  have sufficient Arrange → Act → Assert coverage.

These metrics do not replace the E2E behavioral suite.  They complement it:
the suite proves the contracts hold; the metrics prove the contracts matter.

---

## 5. Relationship to ADR 0001

This document governs **how** work is done within the boundaries that
[ADR 0001](../adr/0001-tui-search-and-presentation-pipeline.md) defines.

ADR 0001 defines:
- What the stages are
- What each stage owns
- What each stage must not do
- The governance trigger for changing those boundaries

This document defines:
- How capabilities are scoped and expressed
- How capabilities are mapped to stages
- What "done" looks like for a capability
- When architectural review is and is not needed

Neither document supersedes the other.  ADR 0001 is the architectural contract;
this document is the development practice that preserves it.
