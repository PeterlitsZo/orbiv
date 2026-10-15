use crate::{OrbivError, OrbivResult};

/// A migration using the shared handler `H`.
///
/// Use [`macro@crate::migration`] to supply metadata and async-trait support.
#[async_trait::async_trait]
pub trait Migration<H>: Send + Sync
where
    H: Send + Sync + 'static,
{
    /// Stable version; migrations must be ordered consecutively from `1`.
    fn version(&self) -> u64;

    /// Stable name checked against migration history.
    fn name(&self) -> &str;

    /// Description stored in migration history.
    fn description(&self) -> &str;

    /// Applies the migration; recorded as successful only on `Ok(())`.
    async fn up(&self, handler: &H) -> OrbivResult<()>;

    /// Reverts the migration. Returns `Unimplemented` by default.
    async fn down(&self, _handler: &H) -> OrbivResult<()> {
        Err(OrbivError::unimplemented("not implemented"))
    }
}
