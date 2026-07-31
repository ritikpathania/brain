# `brain-sdk-rs`

`brain-sdk-rs` provides the native Rust Client SDK for communicating with the Brain background daemon.

## Purpose
Provides ergonomic, strongly typed async Rust client handles for querying, ingesting data, and managing sessions over UDS or HTTP sockets.

## Public Surface
- `BrainClient`: Primary async client connection handle.
- `ClientConfig`: Socket path and timeout configuration options.

## Out of Scope
- Direct domain state mutations or storage database management.

## Documentation Links
- **[IPC Protocol Spec](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/protocol.md)**
