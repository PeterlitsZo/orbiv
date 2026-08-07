use crate::OrbitResult;

/// Represents a source for migration records, such as a database or something
/// else.
#[async_trait::async_trait]
pub trait MigratorSource: Send + Sync {
    /// The component name associated with this migrator source.
    ///
    /// It can be the service name or any other identifier for the component.
    /// For example, if three services use the same database to store migration
    /// records, let each service use a different component name can avoid
    /// conflicts when multiple services are running simultaneously.
    fn component(&self) -> String {
        "default".to_string()
    }

    /// Install something before other methods are called.
    ///
    /// For example, if the migrator source is a database, this might create the
    /// necessary tables.
    async fn install(&self) -> OrbitResult<()>;

    /// Lists all migration records from the source.
    ///
    /// The records MUST be returned in ascending order by version.
    async fn list_records(&self) -> OrbitResult<Vec<MigrationRecord>>;

    /// Adds a new migration record to the source.
    ///
    /// If the version already exists, it MUST raise an error.
    async fn add_record(&self, record: MigrationRecord) -> OrbitResult<()>;

    /// Remove a migration record from the source by its version.
    ///
    /// If the version does not exist, it MUST raise an error.
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
