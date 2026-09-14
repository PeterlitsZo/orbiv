use std::sync::{Arc, Mutex, MutexGuard};

use orbiv::{
    Migration, MigrationRecord, Migrator, MigratorSource, MigratorSteps, OrbivError,
    OrbivErrorKind, OrbivResult,
};
use orbiv_migrator_source_rusqlite::{OrbivMigratorSourceRusqlite, SharedConnection};
use rusqlite::Connection;

#[derive(Clone)]
struct SqliteHandler {
    connection: SharedConnection,
}

impl SqliteHandler {
    fn connection(&self) -> OrbivResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|error| {
            OrbivError::internal(format!("Failed to lock the SQLite connection: {error}"))
        })
    }
}

struct MigrationV001;

#[async_trait::async_trait]
impl Migration<SqliteHandler> for MigrationV001 {
    fn version(&self) -> u64 {
        1
    }

    fn name(&self) -> &str {
        "add foobar"
    }

    fn description(&self) -> &str {
        "Adds a 'foobar' table."
    }

    async fn up(&self, handler: &SqliteHandler) -> OrbivResult<()> {
        handler
            .connection()?
            .execute_batch(
                "CREATE TABLE foobar (id INTEGER PRIMARY KEY, value TEXT NOT NULL);\
                 INSERT INTO foobar (value) VALUES ('foobar');",
            )
            .map_err(|error| {
                OrbivError::internal("Failed to create and populate the foobar table.")
                    .source(error)
            })?;
        Ok(())
    }

    async fn down(&self, handler: &SqliteHandler) -> OrbivResult<()> {
        handler
            .connection()?
            .execute("DROP TABLE foobar", [])
            .map_err(|error| {
                OrbivError::internal("Failed to drop the foobar table.").source(error)
            })?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory()?));
    let handler = SqliteHandler {
        connection: connection.clone(),
    };
    let source = OrbivMigratorSourceRusqlite::new("default", connection.clone())?;
    let migrations: Vec<Box<dyn Migration<SqliteHandler>>> = vec![Box::new(MigrationV001)];

    let migrator = Migrator::builder()
        .handler(handler.clone())
        .source(source.clone())
        .migrations(migrations)
        .build()?;

    migrator.up(MigratorSteps::All).await?;

    let records = source.list_records().await?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].version, 1);
    assert_eq!(records[0].name, "add foobar");
    {
        let connection = handler.connection()?;
        let row_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM foobar WHERE value = 'foobar'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(row_count, 1);
    }

    migrator.down(MigratorSteps::All).await?;
    assert!(source.list_records().await?.is_empty());
    {
        let connection = handler.connection()?;
        assert!(connection.prepare("SELECT * FROM foobar").is_err());
    }

    let record = MigrationRecord {
        version: 1,
        name: "duplicate-check".to_string(),
        description: "Checks duplicate record handling.".to_string(),
        applied_at: jiff::Timestamp::now(),
        execution_time: jiff::SignedDuration::from_millis(1),
        success: true,
        failed_reason: None,
    };
    source.add_record(record.clone()).await?;
    let error = source.add_record(record).await.unwrap_err();
    assert_eq!(error.kind(), &OrbivErrorKind::BadArgument);

    source.remove_record(1).await?;
    let error = source.remove_record(1).await.unwrap_err();
    assert_eq!(error.kind(), &OrbivErrorKind::BadArgument);

    println!("Done.");
    Ok(())
}
