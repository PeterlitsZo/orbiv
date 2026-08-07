# Orbiv

The Orbiv crates provides a framework for managing database (or anything)
migrations.

- `orbiv`: The core crate providing the core migration logic.
- `orbiv-migrator-source-redis`: A Redis-based source for Orbiv with `redis`
  crate.
- `orbiv-migrator-source-sqlx`: A SQL-based source for Orbiv with `sqlx`
  crate.

## Tutorial

Use `cargo` to add dependencies to your project:

```
cargo add orbiv
cargo add orbiv-migrator-source-sqlx
```

First at all, you should define a handler to access the database or anything
you want to migrate.

Here we define a `PostgresHandler` to help we access PostgreSQL. It just
wraps a `sqlx` connection pool and we can use `sqlx` to execute SQL queries.

```rust
#[derive(Clone)]
struct PostgresHandler {
    pub pool: sqlx::Pool<sqlx::Postgres>,
}

impl PostgresHandler {
    async fn new(pool: sqlx::Pool<sqlx::Postgres>) -> anyhow::Result<Self> {
        Ok(Self { pool })
    }
}
```

Then we need to define some `Migration` objects. Here is the first `Migration`
we will define:

```rust
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
```

It is very simple:

- version: The version number of the migration, starting from 1, incrementing by
  1 for each migration.
- name: The name of the migration, used to identify it. Do not change the name
  of the migration struct after it has been applied.
- description: A description of the migration, used to provide context for the
  migration.
- up: The migration logic to apply the migration.
- down: The migration logic to revert the migration.

Then we can give the handler, source and migrations list here:

````rust
use orbiv_migrator_source_sqlx::{OrbivMigratorSourceSqlx, OrbivMigratorSourceSqlxOptions};

// You can use this command to run a PostgreSQL server locally:
//
// ```bash
// docker run --name some-postgres -p 5432:5432 -e POSTGRES_PASSWORD=mysecretpassword -d postgres
// ```
let pool = sqlx::Pool::connect("postgres://postgres:mysecretpassword@localhost:5432/postgres").await?;
let handler = PostgresHandler::new(pool.clone())
        .await?;

// Build the migrator source and migrations.
let source = OrbivMigratorSourceSqlx::new(
    "default",
    OrbivMigratorSourceSqlxOptions { pool: pool.clone() },
).unwrap();
let migrations: Vec<Box<dyn Migration<PostgresHandler>>> =
    vec![Box::new(MigrationV001), Box::new(MigrationV002)];
````

The handler is used to let migration access the database (or anything else). The
source is used to help the migrator to store the metadata about the migrations.
The migrations list is a list of all the migrations to apply.

Here we use the `OrbivMigratorSourceSqlx` -- a SQLx-based implementation of the
migrator source. You can use it if you want to store the migration metadata in a
SQL database. We also define the `OrbivMigratorSourceRedis`, if you want to
store the migration metadata in Redis.

Then it is very easy to build a migrator and let it run:

```rust
// Build the migrator and apply the migrations.
let migrator = Migrator::builder()
    .handler(handler.clone())
    .source(source.clone())
    .migrations(migrations)
    .build()
    .unwrap();
migrator.up(MigratorSteps::All).await.unwrap();
```
