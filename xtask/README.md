# `xtask`

`xtask` provides workspace automation tasks, contract generation, and documentation verification commands.

## Purpose
Implements cargo extension tasks: contract generation (`generate-contracts`), verification (`verify`), and composable documentation checks (`docs all`).

## Public Surface
- `cargo xtask docs [links|readmes|indexes|snippets|frontmatter|all]`
- `cargo xtask generate-contracts`
- `cargo xtask verify`

## Out of Scope
- Production binary code shipped to end users.

## Documentation Links
- **[Documentation Governance Policy](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/governance/GOVERNANCE.md)**
