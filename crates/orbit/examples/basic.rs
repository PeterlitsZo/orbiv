use std::sync::{Arc, Mutex};

use orbit::{MigrateResult, Migration, Migrator};

type MemoryHandler = Arc<Mutex<Vec<String>>>;

struct MigrationV001;

#[async_trait::async_trait]
impl Migration<MemoryHandler> for MigrationV001 {
    async fn up(&self, handler: &MemoryHandler) -> MigrateResult<()> {
        handler.lock().unwrap().push("foobar".to_string());
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let migrations = vec![Box::new(MigrationV001) as Box<dyn Migration<MemoryHandler>>];
    let handler = MemoryHandler::default();
    let migrator = Migrator::builder()
        .handler(handler.clone())
        .migrations(migrations)
        .build()
        .unwrap();
    migrator.up().await.unwrap();

    assert_eq!(handler.lock().unwrap().len(), 1);
    assert_eq!(handler.lock().unwrap()[0], "foobar");
}
