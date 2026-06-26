# brain-domain

## Purpose
Pure domain entities, strongly-typed IDs, DTOs, and API models.

## Responsibilities
* Model unique domain data structures (Node, Edge, Embedding, Message, Conversation).
* Define strongly-typed chronological and standard identifiers.
* Provide DTO wrappers to isolate database entity constraints from API/UI boundaries.
* Implement serialization/deserialization logic for domain types.

## Dependencies
* **Allowed:** Standard library and shared serialization workspace crates (`serde`, `uuid`, `ulid`, `serde_json`).
* **Forbidden:** Any other internal workspace crate (must have zero internal crate dependencies).

## Public Interfaces
* Strongly-Typed IDs: `SessionId`, `RunId`, `NodeId`, `EdgeId`, `PluginId`, `ConversationId`, `MessageId`, `DocumentId`
* Entities: `Node`, `Edge`, `Embedding`, `Message`, `Conversation`, `ToolCall`
* DTOs: `NodeDTO`, `EdgeDTO`, `EmbeddingDTO`, `MemoryDTO`

## Owner
Principal System Architect
