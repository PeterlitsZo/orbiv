use crate::{OrbitError, OrbitResult};

#[async_trait::async_trait]
pub trait Migration<H>: Send + Sync
where
    H: Send + Sync + 'static,
{
    fn version(&self) -> u64;

    fn name(&self) -> &str;

    fn description(&self) -> &str;

    async fn up(&self, handler: &H) -> OrbitResult<()>;

    async fn down(&self, _handler: &H) -> OrbitResult<()> {
        Err(OrbitError::unimplemented("not implemented"))
    }
}
