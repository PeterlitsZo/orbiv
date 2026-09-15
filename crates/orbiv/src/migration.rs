use crate::{OrbivError, OrbivResult};

/// Defines one reversible step in a migration sequence.
///
/// `H` is the application-specific handler shared with each migration. A
/// [`crate::Migrator`] applies migrations in the order in which they are
/// provided and requires their versions to form a contiguous sequence starting
/// at `1`.
#[async_trait::async_trait]
pub trait Migration<H>: Send + Sync
where
    H: Send + Sync + 'static,
{
    /// Returns this migration's version.
    ///
    /// Versions must start at `1`, increase by one in the order supplied to the
    /// migrator, and remain unchanged after the migration has been applied.
    fn version(&self) -> u64;

    /// Returns the stable name used to identify this migration in history.
    ///
    /// Changing the name after the migration has been applied causes history
    /// validation to fail.
    fn name(&self) -> &str;

    /// Returns a human-readable description stored in the migration record.
    fn description(&self) -> &str;

    /// Applies this migration with the shared application handler.
    ///
    /// The migration is recorded as successful only when this method returns
    /// `Ok(())`.
    async fn up(&self, handler: &H) -> OrbivResult<()>;

    /// Reverts the changes made by [`Migration::up`].
    ///
    /// The default implementation returns an `Unimplemented` error.
    async fn down(&self, _handler: &H) -> OrbivResult<()> {
        Err(OrbivError::unimplemented("not implemented"))
    }
}
