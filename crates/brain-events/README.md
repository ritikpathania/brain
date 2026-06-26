# brain-events

## Purpose
Command & Event message envelopes, and Event Bus interfaces.

## Responsibilities
* Define synchronous command enum taxonomies (`SessionCommand`, `StorageCommand`, `PluginCommand`, etc.).
* Define asynchronous fire-and-forget event structures (`SystemEvent`, `SessionEvent`, `AgentEvent`, etc.) wrapped in metadata-rich envelopes.
* Implement traits for Event Publishers and Event Subscribers.
* Isolate sync request-response logic from async telemetry/notifications.

## Dependencies
* **Allowed:** `brain-domain`, `brain-core`.
* **Forbidden:** `brain-storage`, `brain-services`, `brain-tui`, `brain-python`, `brain-plugins`.

## Public Interfaces
* Commands: `Command`, `CommandResult`, `SessionCommand`, `StorageCommand`, `PluginCommand`, `ToolCommand`, `ConfigCommand`, `AgentCommand`, `CommandDispatcher`
* Events: `EventEnvelope`, `DomainEvent`, `SystemEvent`, `SessionEvent`, `AgentEvent`, `StorageEvent`, `PluginEvent`, `UIEvent`
* Messaging: `EventPublisher`, `EventSubscriber`

## Owner
Infrastructure Team
