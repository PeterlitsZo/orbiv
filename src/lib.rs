pub use error::{MigrateError, MigrateErrorKind, MigrateResult};

mod error;

#[async_trait::async_trait]
pub trait Migration<H>: Send + Sync
where
    H: Send + Sync + 'static,
{
    async fn up(&self, handler: &H) -> MigrateResult<()>;

    async fn down(&self, _handler: &H) -> MigrateResult<()> {
        Err(MigrateError::unimplemented("not implemented"))
    }
}

pub struct Migrator<H> {
    handler: H,
    migrations: Vec<Box<dyn Migration<H>>>,
}

impl<H> Migrator<H>
where
    H: Send + Sync + 'static,
{
    pub fn builder() -> MigratorBuilder<H> {
        MigratorBuilder::new()
    }

    pub async fn up(&self) -> MigrateResult<()> {
        for migration in &self.migrations {
            migration.up(&self.handler).await?;
        }
        Ok(())
    }
}

pub struct MigratorBuilder<H> {
    handler: Option<H>,
    migrations: Option<Vec<Box<dyn Migration<H>>>>,
}

impl<H> Default for MigratorBuilder<H>
where
    H: Send + Sync + 'static,
{
    fn default() -> Self {
        Self {
            handler: None,
            migrations: None,
        }
    }
}

impl<H> MigratorBuilder<H>
where
    H: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handler(mut self, handler: H) -> Self {
        self.handler = Some(handler);
        self
    }

    pub fn migrations(mut self, migrations: Vec<Box<dyn Migration<H>>>) -> Self {
        self.migrations = Some(migrations);
        self
    }

    pub fn build(self) -> MigrateResult<Migrator<H>> {
        let handler = self.handler.ok_or_else(MigrateError::missing_handler)?;
        let migrations = self
            .migrations
            .ok_or_else(MigrateError::missing_migrations)?;

        Ok(Migrator {
            handler,
            migrations,
        })
    }
}
