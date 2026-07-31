# `brain-integrations`

`brain-integrations` defines application interface Data Transfer Objects (DTOs) and serialization schemas shared across external system adapters.

## Purpose
Acts as the central DTO contract registry for specta contract generation, event envelopes, and capability payload definitions.

## Public Surface
- `Value`: Primitive contract value enum.
- `Capability`: Registered tool/service capability descriptor.
- `IngestionEvent`: Struct for raw ingestion payloads.
- `IngestionEnvelope`: Envelope wrapper for event ingestion.

## Out of Scope
- Direct database persistence or UDS socket handling.

## Documentation Links
- **[Contract Generation Workflow](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/generation_workflow.md)**
- **[Protocol Specification](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/protocol.md)**
