use crate::{OrbivError, OrbivResult};

#[async_trait::async_trait]
pub trait Migration<H>: Send + Sync
where
    H: Send + Sync + 'static,
{
    fn version(&self) -> u64;

    fn name(&self) -> &str;

    fn description(&self) -> &str;

    async fn up(&self, handler: &H) -> OrbivResult<()>;

    async fn down(&self, _handler: &H) -> OrbivResult<()> {
        Err(OrbivError::unimplemented("not implemented"))
    }
}
