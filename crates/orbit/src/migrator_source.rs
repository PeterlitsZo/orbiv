use crate::OrbitResult;

#[async_trait::async_trait]
pub trait MigratorSource: Send + Sync {
    /// Lists all migration records from the source.
    async fn list_records(&self) -> OrbitResult<Vec<MigrationRecord>>;

    /// Adds a new migration record to the source.
    async fn add_record(&self, record: MigrationRecord) -> OrbitResult<()>;

    /// Remove a migration record from the source by its version.
    async fn remove_record(&self, version: u64) -> OrbitResult<()>;
}

#[derive(Debug, Clone)]
pub struct MigrationRecord {
    pub version: u64,
    pub name: String,
    pub description: String,
    pub applied_at: jiff::Timestamp,
    pub execution_time: jiff::SignedDuration,
    pub success: bool,
    pub failed_reason: Option<String>,
}
