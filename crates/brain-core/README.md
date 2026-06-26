# brain-core

## Purpose
Trait specifications, Repository interfaces, and custom Error types.

## Responsibilities
* Define core Repository interfaces for database CRUD abstraction.
* Specify Agent interfaces (chat, extraction, embedding, planning).
* Define system-wide error hierarchy enum (`BrainError`).
* Specify interfaces for Tool and Plugin lifecycle execution.

## Dependencies
* **Allowed:** `brain-domain`.
* **Forbidden:** `brain-storage`, `brain-events`, `brain-services`, `brain-tui`, `brain-python`, `brain-plugins`.

## Public Interfaces
* Repositories: `NodeRepository`, `EdgeRepository`, `EmbeddingRepository`, `SessionRepository`, `ConfigRepository`
* Agents: `ChatAgent`, `ExtractionAgent`, `EmbeddingAgent`, `PlannerAgent`
* Extensibility: `PluginLifecycle`, `PluginRegistryLookup`, `Tool`, `ToolRegistry`, `PluginState`, `ToolMetadata`
* Error Handling: `BrainError`

## Owner
Principal System Architect
