# brain-config

## Purpose
Settings schema, loading hierarchy, and precedence resolver.

## Responsibilities
* Model application configurations (`DatabaseSettings`, `ModelSettings`, `SessionSettings`, `BrainSettings`).
* Resolve configurations using a 6-stage precedence pipeline (Defaults -> Global Config -> Project Config -> Environment -> CLI Flags -> Overrides).
* Trigger config reload evaluations.

## Dependencies
* **Allowed:** `brain-domain`.
* **Forbidden:** Any database/storage dependencies, networking layers, or UI components.

## Public Interfaces
* Schemas: `BrainSettings`, `DatabaseSettings`, `ModelSettings`, `SessionSettings`
* Resolution: Settings loading and parsing utilities.

## Owner
Systems Engineer
