# brain-v2

## Purpose
Main CLI/TUI Application executable entry point.

## Responsibilities
* Manage the application boot sequence and startup state machine transitions (Boot -> Load Config -> Init Logging -> Init Storage -> Migrations -> Load Plugins -> Ready -> Running).
* Coordinate the shutdown sequence safely.
* Act as the root executable that wires up resources and spawns worker threads.

## Dependencies
* **Allowed:** Any crate in the workspace.
* **Forbidden:** None (executable boundary).

## Owner
Systems Engineer
