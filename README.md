# Orbiv

Orbiv manages database (or anything else) migrations.

- `orbiv`: The core crate providing the core migration logic.
- `orbiv-migrator-source-redis`: A Redis-based source for Orbiv with `redis`
  crate.
- `orbiv-migrator-source-rusqlite`: A lightweight SQLite source for Orbiv with
  `rusqlite` and no SQLx or async runtime dependency.
- `orbiv-migrator-source-sqlx`: A SQL-based source for Orbiv with `sqlx`
  crate.

## Tutorial

Use `cargo` to add dependencies to your project:

```bash
cargo add orbiv
cargo add orbiv-migrator-source-sqlx
```

Define a handler to access the database:

```rust
#[derive(Clone)]
struct PostgresHandler {
    pub pool: sqlx::Pool<sqlx::Postgres>,
}
```

Define a migration with its metadata and apply/rollback logic:

```rust,ignore
use orbiv::{Migration, Migrator, MigratorSteps, OrbivError, OrbivResult};

struct MigrationV001;

#[orbiv::migration(
    version = 1,
    name = "add foobar",
    description = "Adds a 'foobar' table.",
)]
impl Migration<PostgresHandler> for MigrationV001 {
    async fn up(&self, handler: &PostgresHandler) -> OrbivResult<()> {
        sqlx::query("CREATE TABLE foobar (id SERIAL PRIMARY KEY, value TEXT)")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to create foobar table").source(e))?;
        Ok(())
    }

    async fn down(&self, handler: &PostgresHandler) -> OrbivResult<()> {
        sqlx::query("DROP TABLE foobar")
            .execute(&handler.pool)
            .await
            .map_err(|e| OrbivError::internal("failed to drop foobar table").source(e))?;
        Ok(())
    }
}
```

Versions start at 1 and increase by one. Keep versions and names stable after
applying migrations. `description` defaults to `""`; omitting `down` returns an
`Unimplemented` error. The macro supplies the metadata methods and
`async_trait`, so neither needs to be written separately.

Create a source to store migration history and list the migrations in order:

````rust,ignore
use orbiv_migrator_source_sqlx::{OrbivMigratorSourceSqlx, OrbivMigratorSourceSqlxOptions};

// You can use this command to run a PostgreSQL server locally:
//
// ```bash
// docker run --name some-postgres -p 5432:5432 -e POSTGRES_PASSWORD=mysecretpassword -d postgres
// ```
let pool = sqlx::Pool::connect("postgres://postgres:mysecretpassword@localhost:5432/postgres").await?;
let handler = PostgresHandler { pool: pool.clone() };

// Build the migrator source and migrations.
let source = OrbivMigratorSourceSqlx::new(
    "default",
    OrbivMigratorSourceSqlxOptions { pool: pool.clone() },
).unwrap();
let migrations: Vec<Box<dyn Migration<PostgresHandler>>> =
    vec![Box::new(MigrationV001)];
````

Build the migrator and apply the migrations:

```rust,ignore
// Build the migrator and apply the migrations.
let migrator = Migrator::builder()
    .handler(handler.clone())
    .source(source.clone())
    .migrations(migrations)
    .build()
    .unwrap();

// No migrations have been applied yet. This also prepares the migrator source;
// callers do not need to call MigratorSource::install themselves.
assert_eq!(migrator.current_version().await.unwrap(), 0);

migrator.up(MigratorSteps::All).await.unwrap();
assert_eq!(migrator.current_version().await.unwrap(), 1);
```
