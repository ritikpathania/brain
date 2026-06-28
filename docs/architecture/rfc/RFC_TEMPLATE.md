# RFC-XXX: [Title of Amendment]

**Author:** [Author Name]  
**Date:** [Date]  
**Status:** [Proposed / Draft / Approved / Rejected]  
**Reference RFCs:** [e.g. RFC-001 / RFC-002]

---

## 1. Executive Summary
Provide a brief 1-2 paragraph description of the proposed change, the motivation behind it, and what problem it solves.

## 2. Affected Subsystems
List all crates and packages impacted by this change:
- `crates/brain-domain`
- `crates/...`
- `python/brain_ai`

## 3. Proposal Details
Explain the new design in detail. Include any structural, mathematical, or algorithmic details.

## 4. Contract Changes (Interfaces & Types)
Specify exact signature modifications:

### Rust Crate Signature Changes
```rust
// Show exact code additions / modifications
```

### Python API Signature Changes
```python
# Show exact code additions / modifications
```

## 5. Backward Compatibility Plan
Detail how this change interacts with existing persistent databases (SQLite migrations), old command schemas, and event envelopes.

## 6. Verification & Testing Plan
Describe how the implementer must test the correctness of this change:
- Unit tests to write.
- Integration test scenarios.
- Benchmark validation target.
