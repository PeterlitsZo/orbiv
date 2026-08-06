use crate::{Migration, MigrationRecord, MigratorSource, OrbitError, OrbitResult};

pub struct Migrator<H, S> {
    handler: H,
    source: S,
    migrations: Vec<Box<dyn Migration<H>>>,
}

impl<H, S> Migrator<H, S>
where
    H: Send + Sync + 'static,
    S: MigratorSource,
{
    pub fn builder() -> MigratorBuilder<H, S> {
        MigratorBuilder::new()
    }

    pub async fn up(&self) -> OrbitResult<()> {
        let records = self.source.list_records().await?;
        let has_failed = records.iter().any(|r| !r.success);
        if has_failed {
            return Err(OrbitError::has_failed_migration(
                "Cannot apply migrations because there are failed previous migrations.",
            ));
        }

        self.validate_migrations()?;
        let records = self.validate_records(records)?;

        for migration in self.migrations.iter().skip(records.len()) {
            // Apply the migration.
            let applied_at = jiff::Timestamp::now();
            let apply_result = migration.up(&self.handler).await;
            let execution_time = jiff::Timestamp::now().duration_since(applied_at);

            // Handle the result of the migration.
            match apply_result {
                Ok(_) => {
                    // Record the migration as applied.
                    self.source
                        .add_record(MigrationRecord {
                            version: migration.version(),
                            name: migration.name().to_string(),
                            description: migration.description().to_string(),
                            applied_at,
                            execution_time,
                            success: true,
                            failed_reason: None,
                        })
                        .await
                        .map_err(|e| e.context("add a record after apply successfully"))?;
                }
                Err(e) => {
                    let e = e.context("apply migration failed");

                    // Record the migration as failed.
                    self.source
                        .add_record(MigrationRecord {
                            version: migration.version(),
                            name: migration.name().to_string(),
                            description: migration.description().to_string(),
                            applied_at,
                            execution_time,
                            success: false,
                            failed_reason: Some(e.to_string()),
                        })
                        .await
                        .map_err(|e| e.context("add a record after apply failed"))?;

                    return Err(e);
                }
            }
        }

        Ok(())
    }

    fn validate_migrations(&self) -> OrbitResult<()> {
        for (index, migration) in self.migrations.iter().enumerate() {
            let expected_version = index as u64 + 1;
            let actual_version = migration.version();
            if actual_version != expected_version {
                return Err(OrbitError::invalid_migration(format!(
                    "Invalid local migration sequence: expected version {expected_version}, found version {actual_version} ({}).",
                    migration.name(),
                )));
            }
        }

        Ok(())
    }

    fn validate_records(
        &self,
        mut records: Vec<MigrationRecord>,
    ) -> OrbitResult<Vec<MigrationRecord>> {
        records.sort_unstable_by_key(|record| record.version);

        for (index, record) in records.iter().enumerate() {
            let expected_version = index as u64 + 1;
            if record.version != expected_version {
                return Err(OrbitError::invalid_migration(format!(
                    "Invalid migration history: expected recorded version {expected_version}, found version {} ({}).",
                    record.version, record.name,
                )));
            }

            let migration = self.migrations.get(index).ok_or_else(|| {
                OrbitError::invalid_migration(format!(
                    "Invalid migration history: recorded version {} ({}) does not exist in the local migration list.",
                    record.version, record.name,
                ))
            })?;

            if record.name != migration.name() {
                return Err(OrbitError::invalid_migration(format!(
                    "Invalid migration history for version {}: expected name {:?}, found {:?}.",
                    record.version,
                    migration.name(),
                    record.name,
                )));
            }
        }

        Ok(records)
    }
}

pub struct MigratorBuilder<H, S> {
    handler: Option<H>,
    source: Option<S>,
    migrations: Option<Vec<Box<dyn Migration<H>>>>,
}

impl<H, S> Default for MigratorBuilder<H, S> {
    fn default() -> Self {
        Self {
            handler: None,
            source: None,
            migrations: None,
        }
    }
}

impl<H, S> MigratorBuilder<H, S>
where
    H: Send + Sync + 'static,
    S: MigratorSource,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handler(mut self, handler: H) -> Self {
        self.handler = Some(handler);
        self
    }

    pub fn source(mut self, source: S) -> Self {
        self.source = Some(source);
        self
    }

    pub fn migrations(mut self, migrations: Vec<Box<dyn Migration<H>>>) -> Self {
        self.migrations = Some(migrations);
        self
    }

    pub fn build(self) -> OrbitResult<Migrator<H, S>> {
        let handler = self
            .handler
            .ok_or_else(|| OrbitError::bad_argument("Missing handler."))?;
        let source = self
            .source
            .ok_or_else(|| OrbitError::bad_argument("Missing source."))?;
        let migrations = self
            .migrations
            .ok_or_else(|| OrbitError::bad_argument("Missing migrations."))?;

        Ok(Migrator {
            handler,
            source,
            migrations,
        })
    }
}
