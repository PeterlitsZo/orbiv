use indoc::indoc;
use orbiv::{MigrationRecord, MigratorSource, OrbivError, OrbivResult};
use sqlx::{Database, Pool, Row};

const CREATE_MIGRATIONS_TABLE_SQL: &str = indoc! {r#"
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
"#};

pub struct OrbivMigratorSourceSqlxOptions<DB>
where
    DB: Database,
{
    pub pool: Pool<DB>,
}

pub struct OrbivMigratorSourceSqlx<DB>
where
    DB: Database,
{
    component: String,
    pool: Pool<DB>,
}

impl<DB> Clone for OrbivMigratorSourceSqlx<DB>
where
    DB: Database,
{
    fn clone(&self) -> Self {
        Self {
            component: self.component.clone(),
            pool: self.pool.clone(),
        }
    }
}

impl<DB> OrbivMigratorSourceSqlx<DB>
where
    DB: Database,
{
    pub fn new(
        component: impl Into<String>,
        opts: OrbivMigratorSourceSqlxOptions<DB>,
    ) -> OrbivResult<Self> {
        let component = component.into();
        if component.chars().count() > 127 {
            return Err(OrbivError::bad_argument(
                "SQL migrator source component cannot exceed 127 characters.",
            ));
        }

        Ok(Self {
            component,
            pool: opts.pool,
        })
    }

    fn version_to_i64(version: u64) -> OrbivResult<i64> {
        i64::try_from(version).map_err(|error| {
            OrbivError::bad_argument("Migration version exceeds the SQL BIGINT range.")
                .source(error)
        })
    }

    fn version_from_i64(version: i64) -> OrbivResult<u64> {
        u64::try_from(version).map_err(|error| {
            OrbivError::internal("SQL migration record contains a negative version.").source(error)
        })
    }

    fn validate_name(name: &str) -> OrbivResult<()> {
        if name.chars().count() > 127 {
            return Err(OrbivError::bad_argument(
                "SQL migration name cannot exceed 127 characters.",
            ));
        }
        Ok(())
    }

    fn timestamp_from_millis(value: i64) -> OrbivResult<jiff::Timestamp> {
        jiff::Timestamp::from_millisecond(value).map_err(|error| {
            OrbivError::internal("SQL migration record contains an out-of-range applied_at value.")
                .source(error)
        })
    }

    fn duration_to_millis(duration: jiff::SignedDuration) -> OrbivResult<i64> {
        i64::try_from(duration.as_millis()).map_err(|error| {
            OrbivError::bad_argument(
                "Migration execution time exceeds the SQL BIGINT millisecond range.",
            )
            .source(error)
        })
    }
}

macro_rules! impl_migrator_source {
    ($database:ty, $list_sql:expr, $add_sql:expr, $remove_sql:expr) => {
        #[async_trait::async_trait]
        impl MigratorSource for OrbivMigratorSourceSqlx<$database> {
            fn component(&self) -> String {
                self.component.clone()
            }

            async fn install(&self) -> OrbivResult<()> {
                sqlx::query(CREATE_MIGRATIONS_TABLE_SQL)
                    .execute(&self.pool)
                    .await
                    .map_err(|error| {
                        OrbivError::internal("Failed to create the SQL migration records table.")
                            .source(error)
                    })?;
                Ok(())
            }

            async fn list_records(&self) -> OrbivResult<Vec<MigrationRecord>> {
                let rows = sqlx::query($list_sql)
                    .bind(&self.component)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|error| {
                        OrbivError::internal(
                            "Failed to list migration records from the SQL database.",
                        )
                        .source(error)
                    })?;

                rows.into_iter()
                    .map(|row| {
                        let version: i64 = row.try_get("version").map_err(|error| {
                            OrbivError::internal("Failed to read a SQL migration version.")
                                .source(error)
                        })?;
                        let name: String = row.try_get("name").map_err(|error| {
                            OrbivError::internal("Failed to read a SQL migration name.")
                                .source(error)
                        })?;
                        let description: String = row.try_get("description").map_err(|error| {
                            OrbivError::internal("Failed to read a SQL migration description.")
                                .source(error)
                        })?;
                        let applied_at: i64 = row.try_get("applied_at").map_err(|error| {
                            OrbivError::internal("Failed to read a SQL migration applied_at.")
                                .source(error)
                        })?;
                        let execution_time: i64 =
                            row.try_get("execution_time").map_err(|error| {
                                OrbivError::internal(
                                    "Failed to read a SQL migration execution_time.",
                                )
                                .source(error)
                            })?;
                        let success: bool = row.try_get("success").map_err(|error| {
                            OrbivError::internal("Failed to read a SQL migration success flag.")
                                .source(error)
                        })?;
                        let failed_reason: Option<String> =
                            row.try_get("failed_reason").map_err(|error| {
                                OrbivError::internal(
                                    "Failed to read a SQL migration failed_reason.",
                                )
                                .source(error)
                            })?;

                        Ok(MigrationRecord {
                            version: Self::version_from_i64(version)?,
                            name,
                            description,
                            applied_at: Self::timestamp_from_millis(applied_at)?,
                            execution_time: jiff::SignedDuration::from_millis(execution_time),
                            success,
                            failed_reason,
                        })
                    })
                    .collect()
            }

            async fn add_record(&self, record: MigrationRecord) -> OrbivResult<()> {
                Self::validate_name(&record.name)?;
                let version = Self::version_to_i64(record.version)?;
                let applied_at = record.applied_at.as_millisecond();
                let execution_time = Self::duration_to_millis(record.execution_time)?;
                let result = sqlx::query($add_sql)
                    .bind(&self.component)
                    .bind(version)
                    .bind(record.name)
                    .bind(record.description)
                    .bind(applied_at)
                    .bind(execution_time)
                    .bind(record.success)
                    .bind(record.failed_reason)
                    .execute(&self.pool)
                    .await;

                match result {
                    Ok(_) => Ok(()),
                    Err(error)
                        if error
                            .as_database_error()
                            .is_some_and(|error| error.is_unique_violation()) =>
                    {
                        Err(OrbivError::bad_argument(format!(
                            "Migration version {} already exists.",
                            record.version
                        ))
                        .source(error))
                    }
                    Err(error) => Err(OrbivError::internal(
                        "Failed to add a migration record to the SQL database.",
                    )
                    .source(error)),
                }
            }

            async fn remove_record(&self, version: u64) -> OrbivResult<()> {
                let sql_version = Self::version_to_i64(version)?;
                let result = sqlx::query($remove_sql)
                    .bind(&self.component)
                    .bind(sql_version)
                    .execute(&self.pool)
                    .await
                    .map_err(|error| {
                        OrbivError::internal(
                            "Failed to remove a migration record from the SQL database.",
                        )
                        .source(error)
                    })?;

                if result.rows_affected() == 0 {
                    return Err(OrbivError::bad_argument(format!(
                        "Migration version {version} does not exist."
                    )));
                }

                Ok(())
            }
        }
    };
}

impl_migrator_source!(
    sqlx::MySql,
    indoc! { r#"
        SELECT version, name, description, applied_at, execution_time, success, failed_reason
        FROM orbiv_migrations
        WHERE component = ?
        ORDER BY version ASC
    "# },
    indoc! { r#"
        INSERT INTO orbiv_migrations (component, version, name, description, applied_at, execution_time, success, failed_reason)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    "# },
    indoc! { r#"
        DELETE FROM orbiv_migrations
        WHERE component = ? AND version = ?
    "# }
);

impl_migrator_source!(
    sqlx::Postgres,
    indoc! { r#"
        SELECT version, name, description, applied_at, execution_time, success, failed_reason
        FROM orbiv_migrations
        WHERE component = $1
        ORDER BY version ASC
    "# },
    indoc! { r#"
        INSERT INTO orbiv_migrations (component, version, name, description, applied_at, execution_time, success, failed_reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    "# },
    indoc! { r#"
        DELETE FROM orbiv_migrations
        WHERE component = $1 AND version = $2
    "# }
);

impl_migrator_source!(
    sqlx::Sqlite,
    indoc! { r#"
        SELECT version, name, description, applied_at, execution_time, success, failed_reason
        FROM orbiv_migrations
        WHERE component = ?
        ORDER BY version ASC
    "# },
    indoc! { r#"
        INSERT INTO orbiv_migrations (component, version, name, description, applied_at, execution_time, success, failed_reason)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    "# },
    indoc! { r#"
        DELETE FROM orbiv_migrations
        WHERE component = ? AND version = ?
    "# }
);
