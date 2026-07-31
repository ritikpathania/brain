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
from brain_sdk import BrainClient

client = BrainClient()
response = client.query("Find all active session contexts")
print(response)
```

## Documentation Links
- **[Installation Guide](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/guides/installation.md)**
- **[Plugin API Specification](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/plugin-api.md)**
