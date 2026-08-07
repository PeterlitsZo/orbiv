use orbiv::{MigrationRecord, MigratorSource, OrbivError, OrbivResult};
use redis::AsyncCommands;

/// Options used to connect an [`OrbivMigratorSourceRedis`] to Redis.
#[derive(Debug, Clone)]
pub struct OrbivMigratorSourceRedisOptions {
    /// A Redis connection URL, for example `redis://127.0.0.1/`.
    pub url: String,
}

/// Stores Orbiv migration records in a Redis hash.
#[derive(Clone)]
pub struct OrbivMigratorSourceRedis {
    component: String,
    client: redis::Client,
}

impl OrbivMigratorSourceRedis {
    /// Creates a Redis-backed migrator source.
    ///
    /// This validates the connection URL but does not connect to Redis. A
    /// connection is established when a record operation is first executed.
    pub fn new(
        component: impl Into<String>,
        opts: OrbivMigratorSourceRedisOptions,
    ) -> OrbivResult<Self> {
        let client = redis::Client::open(opts.url).map_err(|error| {
            OrbivError::bad_argument("Invalid Redis connection URL.").source(error)
        })?;

        Ok(Self {
            component: component.into(),
            client,
        })
    }

    fn migrations_key(&self) -> String {
        format!("orbiv:{{{}}}:migrations", self.component)
    }

    async fn connection(&self) -> OrbivResult<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| OrbivError::internal("Failed to connect to Redis.").source(error))
    }
}

#[async_trait::async_trait]
impl MigratorSource for OrbivMigratorSourceRedis {
    fn component(&self) -> String {
        self.component.clone()
    }

    async fn install(&self) -> OrbivResult<()> {
        Ok(())
    }

    async fn list_records(&self) -> OrbivResult<Vec<MigrationRecord>> {
        let mut connection = self.connection().await?;
        let values: Vec<String> =
            connection
                .hvals(self.migrations_key())
                .await
                .map_err(|error| {
                    OrbivError::internal("Failed to list migration records from Redis.")
                        .source(error)
                })?;

        let mut records = values
            .into_iter()
            .map(|value| json_to_record(&value))
            .collect::<OrbivResult<Vec<_>>>()?;

        records.sort_by_key(|record| record.version);
        Ok(records)
    }

    async fn add_record(&self, record: MigrationRecord) -> OrbivResult<()> {
        let value = record_to_json(&record)?;
        let mut connection = self.connection().await?;
        let inserted: bool = connection
            .hset_nx(self.migrations_key(), record.version, value)
            .await
            .map_err(|error| {
                OrbivError::internal("Failed to add a migration record to Redis.").source(error)
            })?;

        if !inserted {
            return Err(OrbivError::bad_argument(format!(
                "Migration version {} already exists.",
                record.version
            )));
        }

        Ok(())
    }

    async fn remove_record(&self, version: u64) -> OrbivResult<()> {
        let mut connection = self.connection().await?;
        let removed: usize = connection
            .hdel(self.migrations_key(), version)
            .await
            .map_err(|error| {
                OrbivError::internal("Failed to remove a migration record from Redis.")
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct InnerMigrationRecord {
    version: u64,
    name: String,
    description: String,
    applied_at: jiff::Timestamp,
    execution_time: jiff::SignedDuration,
    success: bool,
    failed_reason: Option<String>,
}

fn record_to_json(record: &MigrationRecord) -> OrbivResult<String> {
    serde_json::to_string(&InnerMigrationRecord {
        version: record.version,
        name: record.name.clone(),
        description: record.description.clone(),
        applied_at: record.applied_at,
        execution_time: record.execution_time,
        success: record.success,
        failed_reason: record.failed_reason.clone(),
    })
    .map_err(|error| OrbivError::internal("Failed to serialize a migration record.").source(error))
}

fn json_to_record(json: &str) -> OrbivResult<MigrationRecord> {
    let inner: InnerMigrationRecord = serde_json::from_str(json).map_err(|error| {
        OrbivError::internal("Failed to parse a migration record from Redis.").source(error)
    })?;
    Ok(MigrationRecord {
        version: inner.version,
        name: inner.name,
        description: inner.description,
        applied_at: inner.applied_at,
        execution_time: inner.execution_time,
        success: inner.success,
        failed_reason: inner.failed_reason,
    })
}
