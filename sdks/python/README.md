# `brain-python` SDK

Python Client SDK for communicating with the `brain` relational memory engine.

## Prerequisites
- **Python**: Version >= 3.10 required.

## Installation
```bash
pip install brain-memory-sdk
```

## Quick Start
```python
from brain_sdk import BrainClient, IngestionEvent

with BrainClient() as client:
    event = IngestionEvent.message(role="user", content="Hello Brain")
    ack = client.send(event)
    print(f"Ingested event sequence: {ack.sequence}, event_id: {ack.event_id}")
```

## Documentation Links
- **[Installation Guide](../../docs/guides/installation.md)**
- **[Plugin API Specification](../../docs/reference/plugin-api.md)**
