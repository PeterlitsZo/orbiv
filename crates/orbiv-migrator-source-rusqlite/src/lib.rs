//! A lightweight SQLite migrator source for Orbiv, backed by `rusqlite`.
//!
//! `rusqlite` performs synchronous database operations. Consequently, the
//! [`MigratorSource`] methods implemented by this crate may briefly block the
//! async executor while they access the migration metadata table.

use std::sync::{Arc, Mutex, MutexGuard};

use orbiv::{MigrationRecord, MigratorSource, OrbivError, OrbivResult};
use rusqlite::{Connection, params};

/// A shareable rusqlite connection.
///
/// `rusqlite::Connection` is `Send` but not `Sync`, so access is serialized by
/// a mutex. Sharing one connection also allows an application handler and this
/// migrator source to use the same in-memory SQLite database.
pub type SharedConnection = Arc<Mutex<Connection>>;

/// Stores Orbiv migration records in SQLite through `rusqlite`.
///
/// The source uses the same `orbiv_migrations` schema and value encoding as
/// `orbiv-migrator-source-sqlx`, so the two SQLite implementations can read
/// each other's migration records.
#[derive(Clone)]
pub struct OrbivMigratorSourceRusqlite {
    component: String,
    connection: SharedConnection,
}

impl OrbivMigratorSourceRusqlite {
    /// Creates a rusqlite-backed migrator source.
    ///
    /// This validates the component name but does not access SQLite. The
    /// metadata table is created when [`MigratorSource::install`] is called.
    pub fn new(component: impl Into<String>, connection: SharedConnection) -> OrbivResult<Self> {
        let component = component.into();
        if component.chars().count() > 127 {
            return Err(OrbivError::bad_argument(
                "SQLite migrator source component cannot exceed 127 characters.",
            ));
        }

        Ok(Self {
            component,
            connection,
        })
    }

    fn connection(&self) -> OrbivResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|error| {
            OrbivError::internal(format!("Failed to lock the rusqlite connection: {error}"))
        })
    }
}

#[async_trait::async_trait]
impl MigratorSource for OrbivMigratorSourceRusqlite {
    fn component(&self) -> String {
        self.component.clone()
    }

    async fn install(&self) -> OrbivResult<()> {
        self.connection()?
            .execute(
                indoc::indoc! { r#"
                    CREATE TABLE IF NOT EXISTS orbiv_migrations (
                        component VARCHAR(127) NOT NULL,
                        version BIGINT NOT NULL,
                        name VARCHAR(255) NOT NULL,
                        description TEXT NOT NULL,
                        applied_at BIGINT NOT NULL,
                        execution_time BIGINT NOT NULL,
                        success BOOLEAN NOT NULL,
                        failed_reason TEXT NULL,
                        PRIMARY KEY (component, version)
                    )
                "# },
                [],
            )
            .map_err(|error| {
                OrbivError::internal("Failed to create the SQLite migration records table.")
                    .source(error)
            })?;
        Ok(())
    }

    async fn list_records(&self) -> OrbivResult<Vec<MigrationRecord>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(indoc::indoc! { r#"
                SELECT version, name, description, applied_at, execution_time, success, failed_reason
                FROM orbiv_migrations
                WHERE component = ?1
                ORDER BY version ASC
            "# })
            .map_err(|error| {
                OrbivError::internal("Failed to prepare the SQLite migration records query.")
                    .source(error)
            })?;
        let mut rows = statement.query(params![&self.component]).map_err(|error| {
            OrbivError::internal("Failed to list migration records from SQLite.").source(error)
        })?;
        let mut records = Vec::new();

        while let Some(row) = rows.next().map_err(|error| {
            OrbivError::internal("Failed to read a SQLite migration record.").source(error)
        })? {
            let version: i64 = row.get("version").map_err(|error| {
                OrbivError::internal("Failed to read a SQLite migration version.").source(error)
            })?;
            let version = u64::try_from(version).map_err(|error| {
                OrbivError::internal("SQLite migration record contains a negative version.")
                    .source(error)
            })?;
            let name = row.get("name").map_err(|error| {
                OrbivError::internal("Failed to read a SQLite migration name.").source(error)
            })?;
            let description = row.get("description").map_err(|error| {
                OrbivError::internal("Failed to read a SQLite migration description.").source(error)
            })?;
            let applied_at: i64 = row.get("applied_at").map_err(|error| {
                OrbivError::internal("Failed to read a SQLite migration applied_at.").source(error)
            })?;
            let applied_at = jiff::Timestamp::from_millisecond(applied_at).map_err(|error| {
                OrbivError::internal(
                    "SQLite migration record contains an out-of-range applied_at value.",
                )
                .source(error)
            })?;
            let execution_time = row.get("execution_time").map_err(|error| {
                OrbivError::internal("Failed to read a SQLite migration execution_time.")
                    .source(error)
            })?;
            let success = row.get("success").map_err(|error| {
                OrbivError::internal("Failed to read a SQLite migration success flag.")
                    .source(error)
            })?;
            let failed_reason = row.get("failed_reason").map_err(|error| {
                OrbivError::internal("Failed to read a SQLite migration failed_reason.")
                    .source(error)
            })?;

            records.push(MigrationRecord {
                version,
                name,
                description,
                applied_at,
                execution_time: jiff::SignedDuration::from_millis(execution_time),
                success,
                failed_reason,
            });
        }

        Ok(records)
    }

    async fn add_record(&self, record: MigrationRecord) -> OrbivResult<()> {
        if record.name.chars().count() > 127 {
            return Err(OrbivError::bad_argument(
                "SQLite migration name cannot exceed 127 characters.",
            ));
        }
        let version = i64::try_from(record.version).map_err(|error| {
            OrbivError::bad_argument("Migration version exceeds the SQLite INTEGER range.")
                .source(error)
        })?;
        let applied_at = record.applied_at.as_millisecond();
        let execution_time = i64::try_from(record.execution_time.as_millis()).map_err(|error| {
            OrbivError::bad_argument(
                "Migration execution time exceeds the SQLite INTEGER millisecond range.",
            )
            .source(error)
        })?;
        let connection = self.connection()?;
        let result = connection.execute(
            indoc::indoc! { r#"
                INSERT INTO orbiv_migrations (
                    component,
                    version,
                    name,
                    description,
                    applied_at,
                    execution_time,
                    success,
                    failed_reason
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "# },
            params![
                &self.component,
                version,
                &record.name,
                &record.description,
                applied_at,
                execution_time,
                record.success,
                record.failed_reason.as_deref(),
            ],
        );

        match result {
            Ok(_) => Ok(()),
            Err(error)
                if matches!(
                    &error,
                    rusqlite::Error::SqliteFailure(error, _)
                        if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
                            || error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                ) =>
            {
                Err(OrbivError::bad_argument(format!(
                    "Migration version {} already exists.",
                    record.version
                ))
                .source(error))
            }
            Err(error) => Err(
                OrbivError::internal("Failed to add a migration record to SQLite.").source(error),
            ),
        }
    }

    async fn remove_record(&self, version: u64) -> OrbivResult<()> {
        let sql_version = i64::try_from(version).map_err(|error| {
            OrbivError::bad_argument("Migration version exceeds the SQLite INTEGER range.")
                .source(error)
        })?;
        let removed = self
            .connection()?
            .execute(
                indoc::indoc! { r#"
                    DELETE FROM orbiv_migrations
                    WHERE component = ?1 AND version = ?2
                "# },
                params![&self.component, sql_version],
            )
            .map_err(|error| {
                OrbivError::internal("Failed to remove a migration record from SQLite.")
                    .source(error)
            })?;

        if removed == 0 {
            return Err(OrbivError::bad_argument(format!(
                "Migration version {version} does not exist."
            )));
        }

        Ok(())
    }
}
