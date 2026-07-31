# `daemon`

`daemon` manages background execution processes, IPC Unix Domain Sockets, and HTTP telemetry endpoints.

## Purpose
Runs as a long-running background service hosting the memory engine, handling IPC requests, and maintaining read model projections.

## Public Surface
- `DaemonServer`: Unix Domain Socket & HTTP telemetry server.

## Out of Scope
- Interactive TUI terminal widget rendering.

## Documentation Links
- **[Protocol Specification](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/protocol.md)**
- **[Maintenance Guide](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/guides/maintenance.md)**
