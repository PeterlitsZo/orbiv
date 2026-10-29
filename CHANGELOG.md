# Changelog

Notable changes to this project are documented here.

## v0.3.0 - 2026-10-26

### Added

- Added `#[orbiv::migration(...)]` to generate migration metadata methods and
  apply async-trait support, backed by the new `orbiv-macros` crate.

### Changed

- Used the README as the core crate documentation and updated the tutorial
  and basic example to use the migration attribute macro.
- Expanded API documentation for migration errors and history records.

### Fixed

- Fixed the README include path so the core crate builds from its published
  package.

## v0.2.0 - 2026-09-15

### Added

- Added a lightweight rusqlite source without SQLx or async runtime
  dependencies.
- Added `Migrator::current_version`.

### Changed

- Cached successful source installation per `Migrator` instance.
- Documented core APIs and the idempotent `MigratorSource::install` contract.

## v0.1.1 - 2026-08-07

### Changed

- Completed crate metadata and added Cargo installation instructions.

## v0.1.0 - 2026-08-07

### Added

- Added the core framework with validated history, failure safeguards, and
  stepped rollback.
- Added SQLx sources for PostgreSQL, MySQL, and SQLite.
- Added a Redis source, examples, a tutorial, and release tooling.

### Changed

- Renamed the project and crates from Orbit to Orbiv.
