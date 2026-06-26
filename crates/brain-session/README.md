# brain-session

## Purpose
Session Context state machine and volatile cache.

## Responsibilities
* Manage short-term memory (STM) sliding windows.
* Implement volatile token matching inverted indexing for fast session retrieval.
* Track active conversation state prior to long-term database consolidation.

## Dependencies
* **Allowed:** `brain-domain`, `brain-core`.
* **Forbidden:** Direct SQLite database engines or Python GIL executors.

## Public Interfaces
* Volatile context managers, token indices, and short-term memory cache hooks.

## Owner
Core Development Team
