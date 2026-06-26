# brain-plugins

## Purpose
Plugin manifest parser, Lifecycle state engine, and Dependency resolver.

## Responsibilities
* Parse and validate plugin `manifest.toml` profiles.
* Resolve dynamic dependency graphs for registered plugins.
* Manage state transitions (Discovered -> Loaded -> Active -> Suspended) for plugins.
* Enforce security capability validation during initialization.

## Dependencies
* **Allowed:** `brain-domain`, `brain-core`.
* **Forbidden:** User Interface (TUI) components.

## Public Interfaces
* Manifest validation helpers and plugin state transition coordinators.

## Owner
Extensibility Team
