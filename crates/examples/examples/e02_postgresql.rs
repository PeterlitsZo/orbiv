use orbiv::{Migration, Migrator, MigratorSteps, OrbivError, OrbivResult};
use orbiv_migrator_source_sqlx::{OrbivMigratorSourceSqlx, OrbivMigratorSourceSqlxOptions};

#[derive(Clone)]
struct PostgresHandler {
    pool: sqlx::Pool<sqlx::Postgres>,
}

impl PostgresHandler {
    async fn new(pool: sqlx::Pool<sqlx::Postgres>) -> anyhow::Result<Self> {
        Ok(Self { pool })
    }
}

struct MigrationV001;

#[async_trait::async_trait]
impl Migration<PostgresHandler> for MigrationV001 {
    fn version(&self) -> u64 {
        1
    }

    fn name(&self) -> &str {
        "add foobar"
    }

    fn description(&self) -> &str {
        "Adds a 'foobar' table."
    }

    async fn up(&self, handler: &PostgresHandler) -> OrbivResult<()> {
        sqlx::query("CREATE TABLE foobar (id SERIAL PRIMARY KEY, value TEXT)")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to create foobar table").source(e))?;
        sqlx::query("INSERT INTO foobar (value) VALUES ('foobar')")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to insert into foobar table").source(e))?;
        Ok(())
    }

    async fn down(&self, handler: &PostgresHandler) -> OrbivResult<()> {
        sqlx::query("DROP TABLE foobar")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to create foobar table").source(e))?;
        Ok(())
    }
}

struct MigrationV002;

#[async_trait::async_trait]
impl Migration<PostgresHandler> for MigrationV002 {
    fn version(&self) -> u64 {
        2
    }

    fn name(&self) -> &str {
        "add barfoo"
    }

    fn description(&self) -> &str {
        "Adds a 'barfoo' table."
    }

    async fn up(&self, handler: &PostgresHandler) -> OrbivResult<()> {
        sqlx::query("CREATE TABLE barfoo (id SERIAL PRIMARY KEY, value TEXT)")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to create barfoo table").source(e))?;
        sqlx::query("INSERT INTO barfoo (value) VALUES ('barfoo')")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to insert into barfoo table").source(e))?;
        Ok(())
    }

    async fn down(&self, handler: &PostgresHandler) -> OrbivResult<()> {
        sqlx::query("DROP TABLE barfoo")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to create barfoo table").source(e))?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // You can use this command to run a PostgreSQL server locally:
    //
    // ```bash
    // docker run --name some-postgres -p 5432:5432 -e POSTGRES_PASSWORD=mysecretpassword -d postgres
    // ```
    let pool =
        sqlx::Pool::connect("postgres://postgres:mysecretpassword@localhost:5432/postgres").await?;
    let handler = PostgresHandler::new(pool.clone()).await?;

    // Build the migrator source and migrations.
    let source = OrbivMigratorSourceSqlx::new(
        "default",
        OrbivMigratorSourceSqlxOptions { pool: pool.clone() },
    )
    .unwrap();
    let migrations: Vec<Box<dyn Migration<PostgresHandler>>> =
        vec![Box::new(MigrationV001), Box::new(MigrationV002)];

    // Build the migrator and apply the migrations.
    let migrator = Migrator::builder()
        .handler(handler.clone())
        .source(source.clone())
        .migrations(migrations)
        .build()
        .unwrap();
    migrator.up(MigratorSteps::All).await.unwrap();

    // Verify the migrations were applied.
    let rows = sqlx::query("SELECT * FROM orbiv_migrations")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);

    // Verify the migrations were applied correctly.
    let rows = sqlx::query("SELECT * FROM foobar")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let rows = sqlx::query("SELECT * FROM barfoo")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);

    // Rollback the migrations.
    migrator.down(MigratorSteps::All).await.unwrap();

    // Verify the migrations were rolled back correctly.
    let rows = sqlx::query("SELECT * FROM orbiv_migrations")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 0);
    let rows_result = sqlx::query("SELECT * FROM foobar").fetch_all(&pool).await;
    assert!(rows_result.is_err());
    let rows_result = sqlx::query("SELECT * FROM barfoo").fetch_all(&pool).await;
    assert!(rows_result.is_err());

    println!("Done.");

    Ok(())
}
