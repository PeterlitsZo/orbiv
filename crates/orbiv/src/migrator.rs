use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Migration, MigrationRecord, MigratorSource, OrbivError, OrbivResult};

/// Validates and applies a sequence of migrations using a persistent source.
///
/// A migrator owns the application handler, the migration-record source, and
/// the ordered local migration list. Before applying, reverting, or reporting
/// a version, it validates that local migrations and persisted history agree.
pub struct Migrator<H, S> {
    handler: H,
    source: S,
    migrations: Vec<Box<dyn Migration<H>>>,
    installed: AtomicBool,
}

/// Selects how many migrations an [`Migrator::up`] or [`Migrator::down`] call
/// should process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigratorSteps {
    /// Process every available migration.
    All,
    /// Process at most the given number of migrations.
    Number(u64),
}

impl MigratorSteps {
    fn limit(&self) -> usize {
        match self {
            Self::All => usize::MAX,
            Self::Number(number) => usize::try_from(*number).unwrap_or(usize::MAX),
        }
    }
}

impl<H, S> Migrator<H, S>
where
    H: Send + Sync + 'static,
    S: MigratorSource,
{
    /// Creates a builder for a migrator.
    ///
    /// A handler, source, and migration list must all be provided before
    /// calling [`MigratorBuilder::build`].
    pub fn builder() -> MigratorBuilder<H, S> {
        MigratorBuilder::new()
    }

    /// Applies up to `steps` pending migrations in ascending version order.
    ///
    /// The operation stops if persisted history contains a failed migration or
    /// does not match the local migration list. A failed `up` call is recorded
    /// in the source before its error is returned.
    pub async fn up(&self, steps: MigratorSteps) -> OrbivResult<()> {
        self.ensure_installed()
            .await
            .map_err(|e| e.context("install for up"))?;

        let records = self.source.list_records().await?;
        let has_failed = records.iter().any(|r| !r.success);
        if has_failed {
            return Err(OrbivError::has_failed_migration(
                "Cannot apply migrations because there are failed previous migrations.",
            ));
        }

        self.validate_migrations()?;
        let records = self.validate_records(records)?;

        for migration in self
            .migrations
            .iter()
            .skip(records.len())
            .take(steps.limit())
        {
            // Apply the migration.
            let applied_at = jiff::Timestamp::now();
            let apply_result = migration.up(&self.handler).await;
            let execution_time = jiff::Timestamp::now().duration_since(applied_at);

            // Handle the result of the migration.
            let mut record_to_add = MigrationRecord {
                version: migration.version(),
                name: migration.name().to_string(),
                description: migration.description().to_string(),
                applied_at,
                execution_time,
                success: true,
                failed_reason: None,
            };
            match apply_result {
                Ok(_) => {
                    // Record the migration as applied.
                    self.source
                        .add_record(record_to_add)
                        .await
                        .map_err(|e| e.context("add a record after apply successfully"))?;
                }
                Err(e) => {
                    let e = e.context("apply migration failed");

                    // Record the migration as failed.
                    record_to_add.success = false;
                    record_to_add.failed_reason = Some(e.to_string());
                    self.source
                        .add_record(record_to_add)
                        .await
                        .map_err(|e| e.context("add a record after apply failed"))?;

                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Reverts up to `steps` applied migrations in descending version order.
    ///
    /// The operation stops if persisted history contains a failed migration or
    /// does not match the local migration list. Each record is removed only
    /// after its migration is reverted successfully.
    pub async fn down(&self, steps: MigratorSteps) -> OrbivResult<()> {
        self.ensure_installed()
            .await
            .map_err(|e| e.context("install for down"))?;

        let records = self.source.list_records().await?;
        let has_failed = records.iter().any(|r| !r.success);
        if has_failed {
            return Err(OrbivError::has_failed_migration(
                "Cannot revert migrations because there are failed previous migrations.",
            ));
        }

        self.validate_migrations()?;
        let records = self.validate_records(records)?;

        for migration in self
            .migrations
            .iter()
            .take(records.len())
            .rev()
            .take(steps.limit())
        {
            // Revert the migration.
            migration
                .down(&self.handler)
                .await
                .map_err(|e| e.context("revert migration failed"))?;

            // Remove the record after reverting successfully.
            self.source
                .remove_record(migration.version())
                .await
                .map_err(|e| e.context("remove a record after revert successfully"))?;
        }

        Ok(())
    }

    /// Returns the current schema version recorded by the migration source.
    ///
    /// `0` means that no migrations have been applied. This method reads the
    /// source on every call and validates the returned history against the local
    /// migration list. It returns an error instead of a version when history
    /// contains a failed migration or is otherwise inconsistent.
    pub async fn current_version(&self) -> OrbivResult<u64> {
        self.ensure_installed()
            .await
            .map_err(|e| e.context("install for current version"))?;

        let records = self.source.list_records().await?;
        let has_failed = records.iter().any(|record| !record.success);
        if has_failed {
            return Err(OrbivError::has_failed_migration(
                "Cannot determine the current schema version because there are failed previous migrations.",
            ));
        }

        self.validate_migrations()?;
        let records = self.validate_records(records)?;

        Ok(records.last().map_or(0, |record| record.version))
    }

    async fn ensure_installed(&self) -> OrbivResult<()> {
        if self.installed.load(Ordering::Acquire) {
            return Ok(());
        }

        self.source.install().await?;
        self.installed.store(true, Ordering::Release);
        Ok(())
    }

    fn validate_migrations(&self) -> OrbivResult<()> {
        for (index, migration) in self.migrations.iter().enumerate() {
            let expected_version = index as u64 + 1;
            let actual_version = migration.version();
            if actual_version != expected_version {
                return Err(OrbivError::invalid_migration(format!(
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
    ) -> OrbivResult<Vec<MigrationRecord>> {
        records.sort_unstable_by_key(|record| record.version);

        for (index, record) in records.iter().enumerate() {
            let expected_version = index as u64 + 1;
            if record.version != expected_version {
                return Err(OrbivError::invalid_migration(format!(
                    "Invalid migration history: expected recorded version {expected_version}, found version {} ({}).",
                    record.version, record.name,
                )));
            }

            let migration = self.migrations.get(index).ok_or_else(|| {
                OrbivError::invalid_migration(format!(
                    "Invalid migration history: recorded version {} ({}) does not exist in the local migration list.",
                    record.version, record.name,
                ))
            })?;

            if record.name != migration.name() {
                return Err(OrbivError::invalid_migration(format!(
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

/// Builds a [`Migrator`].
///
/// The handler, source, and migration list are all required.
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
    /// Creates an empty migrator builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the application handler passed to each migration.
    pub fn handler(mut self, handler: H) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Sets the persistent migration-record source.
    pub fn source(mut self, source: S) -> Self {
        self.source = Some(source);
        self
    }

    /// Sets the ordered migration list.
    pub fn migrations(mut self, migrations: Vec<Box<dyn Migration<H>>>) -> Self {
        self.migrations = Some(migrations);
        self
    }

    /// Builds the migrator.
    ///
    /// Returns an error when the handler, source, or migration list has not
    /// been provided.
    pub fn build(self) -> OrbivResult<Migrator<H, S>> {
        let handler = self
            .handler
            .ok_or_else(|| OrbivError::bad_argument("Missing handler."))?;
        let source = self
            .source
            .ok_or_else(|| OrbivError::bad_argument("Missing source."))?;
        let migrations = self
            .migrations
            .ok_or_else(|| OrbivError::bad_argument("Missing migrations."))?;

        Ok(Migrator {
            handler,
            source,
            migrations,
            installed: AtomicBool::new(false),
        })
    }
}
