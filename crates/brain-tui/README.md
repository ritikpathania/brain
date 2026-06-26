# brain-tui

## Purpose
Thread-isolated Ratatui user interface.

## Responsibilities
* Manage console UI drawing and keyboard/mouse event cycles.
* Run UI loop on a dedicated, isolated render thread.
* Communicate with the core system using commands and events without blocking.

## Dependencies
* **Allowed:** `brain-domain`, `brain-events`.
* **Forbidden:** Direct SQLite database engines or Python GIL executors. (UI render threads cannot run blocking business logic/queries).

## Public Interfaces
* TUI initialization hooks and command/event listeners.

## Owner
Frontend Team
