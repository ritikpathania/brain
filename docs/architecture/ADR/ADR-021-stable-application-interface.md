# ADR-021: Stable Application Interface

## Status
Proposed

## Context
In [ADR-020 (Protocol Independence)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-020-protocol-independence.md), we established the boundary between the protocol-agnostic Brain Runtime and external interfaces. However, to implement external interfaces and client SDKs without introducing tight coupling or code duplication, we must define the semantic contract that crosses these boundaries.

Without a well-defined boundary contract, different protocol translation layers might interpret queries, mutations, or stream lifetimes differently, causing inconsistent behavior across external transports.

## Relationship to ADR-020
* **ADR-020** defines the *architectural boundaries* and isolation rules (ensuring runtime and adapters are separate).
* **ADR-021** defines the *logical contracts* crossing those boundaries (what semantic operations, interaction patterns, and behaviors the interface guarantees).

```
   ADR-020 (Boundaries)
┌───────────────────────┐
│     Adapters Layer    │
└──────────┬────────────┘
           │   ◄─── ADR-021 (Contract Crossing)
┌──────────▼────────────┐
│ Stable App Interface  │
└──────────┬────────────┘
           │
┌──────────▼────────────┐
│  Brain Runtime Core   │
└───────────────────────┘
```

## Decision
We establish a transport-neutral, capability-based Application Interface. This interface describes the abstract semantic interactions between external protocol adapters and the Brain Runtime without exposing internal subsystems or referencing specific transport protocols.

### 1. Design Principles
* **Transport Independence**: The semantics of any request or response must not change depending on whether it is carried over Unix Domain Sockets, HTTP, WebSockets, or memory buffers.
* **Protocol Neutrality**: The interface does not assume or embed concepts from external protocols (such as JSON-RPC request structures, HTTP verbs, or tool-calling envelopes).
* **Deterministic Contracts**: Given the same request payload and database state, the interface returns identical semantic results across all transports.
* **Long-Term Versionability**: The contract must evolve additively to allow older adapters and client SDKs to interact with newer runtime engines.
* **Composable Operations**: Complex behaviors are built by combining simple, atomic interface operations rather than custom interface entry points.
* **Observable Execution**: Operations report execution metadata, progress, and warnings natively to allow adapters to project diagnostics.
* **Cancellation Safety**: Long-running operations must support cooperative cancellation without causing resource leaks or database corruption.

### 2. Explicit Non-Goals
The Application Interface is strictly restricted from exposing:
* Rust-specific types that do not have language-neutral equivalents (e.g., custom async traits, pointers).
* Core database tables, indices, or raw SQL queries.
* Subsystem repositories, managers, or internal service classes.
* Storage engine lock handles, transactions, or FFI PyO3 boundary details.

### 3. Operation Taxonomy
We classify all operations entering the runtime into five distinct categories:
* **Read**: Retrieve graph snapshots, node properties, or search query results without side effects.
* **Write**: Commit nodes, edges, or property changes. Writes are transactional and commit-oriented.
* **Workflow**: User-initiated orchestration composed of multiple application operations.
* **Subscription**: Observe real-time changes to the graph, active session states, or ingestion queues.
* **Administrative**: Runtime lifecycle, maintenance, diagnostics, indexing, consolidation, decay, system health, database watermarks, or configuration schemas.

### 4. Interaction Patterns
Adapters translate their transport-specific semantics into one of these five abstract interaction patterns:
* **Request-Response (Synchronous)**: A single input payload yields a single output payload (e.g., write transaction, simple config validation).
* **Request-Stream (Progressive/Chunked)**: A single request yields a sequence of events, representing either chunked results (e.g., streaming retrieval matches) or progressive telemetry (e.g., indexing progress updates).
* **Fire-and-Forget**: A one-way command that the runtime accepts and schedules without returning execution progress or results.
* **Long-Running Operation**: A request initiates a background job. The interface immediately returns a reference to track or cancel the job, and emits state transitions until termination.
* **Subscription (Publisher-Subscriber)**: A persistent connection where the runtime pushes events asynchronously as they occur (e.g., graph mutations or error events).

### 5. Translation Boundary Flow
External adapters exist solely to translate between external protocols and the transport-neutral contract defined by the Application Interface:

```text
External Client
      │ (Protocol Messages)
      ▼
Protocol Adapter
      │ [1] Validate incoming payload
      │ [2] Translate protocol DTO to Application DTO
      ▼
Application Interface
      │ [3] Dispatch transport-neutral command/query
      ▼
Brain Runtime Core
```

### 6. DTO Ownership
* **Application DTOs**: Owned entirely by the Application Interface layer. These are transport-neutral representations of requests, responses, errors, and progress events.
* **Protocol DTOs**: Owned by the respective protocol adapters. These adapt and map the Application DTOs to fit specific protocol standards.
* The Runtime Core does not expose its internal domain entities or DB schemas; it converts them to Application DTOs before crossing the boundary.

### 7. Capability Negotiation
To allow old and new adapters to interoperate, the interface exposes capability metadata. Adapters query this interface to negotiate:
* **Protocol/Schema Versions**: Supported semantic versions of the payload contracts.
* **Behavior Profiles**: Explicit declaration of capability support flags:
  * `supports_streaming`
  * `supports_subscriptions`
  * `supports_cancellation`
  * `supports_workflows`
  * `supports_progress`
* **Feature Profiles**: Sets of optional capabilities supported by the runtime (e.g. vector search vs. lexical-only).
* **Operation Support**: Checking if a specific workflow or query type is available.
* **Experimental Flags**: Declaring opt-in support for unstable features.

### 8. Evolution Rules
The interface contracts evolve according to strict rules:
* **Additive Changes**: New fields in request payloads must be optional. New fields in response payloads must be handled gracefully (ignored) by older clients.
* **Semantic Stability**: Semantic behavior of existing operations must not change within a compatibility version.
* **Deprecation Cycle**: Obsolete operations or fields are marked as deprecated for a minimum of one major version iteration before removal.
* **Fallback Behavior**: Clients must safely process unknown fields by ignoring them rather than failing to parse the envelope.

### 9. Streaming Philosophy
Streams are used only when:
* Latency constraints require returning partial results immediately (e.g., partial result delivery or progressive rendering).
* Granular progress reporting is required for long-running workflows.
* The payload size is too large to fit comfortably in a single memory buffer.
Streams must not be used for basic, low-overhead point lookups or commands.

### 10. Architectural Invariants
* **Semantic Parity**: Every operation has exactly one semantic meaning independent of transport.
* **Deterministic Contracts**: Requests with identical inputs and runtime states must yield identical outputs.
* **Expose Capabilities, Not Implementation**: The interface contracts expose only what the runtime can do, never how it stores or computes the result.
* **Passive Translation**: Translation layers must map payloads structure-for-structure. They must never introduce business rules or reinterpret query semantics.
* **Additive Growth**: Payload contracts must evolve additively to prevent breaking backward compatibility.
* **Idempotency Transparency**: The interface must explicitly define whether an operation is idempotent, non-idempotent, or conditionally idempotent. Adapters must never infer this.

## Alternatives Considered
* **Direct JSON-RPC integration**: Rejected because exposing JSON-RPC directly in the core runtime forces all clients (including local CLI or future REST microservices) to adopt a single protocol structure.
* **Direct Domain Entity exposure**: Rejected because domain schemas are highly volatile during early development and exposing them directly breaks the hexagonal boundary.

## Success Criteria
This ADR is complete and successful if an adapter can be built and integrated without introducing transport-specific semantics into the runtime.
