use std::sync::{Arc, Mutex};

use orbit::{Migration, MigrationRecord, Migrator, MigratorSource, OrbitResult};

type MemoryHandler = Arc<Mutex<Vec<String>>>;

struct MigrationV001;

#[async_trait::async_trait]
impl Migration<MemoryHandler> for MigrationV001 {
    fn version(&self) -> u64 {
        1
    }

    fn name(&self) -> &str {
        "add foobar"
    }

    fn description(&self) -> &str {
        "Adds a 'foobar' string to the memory handler."
    }

    async fn up(&self, handler: &MemoryHandler) -> OrbitResult<()> {
        handler.lock().unwrap().push("foobar".to_string());
        Ok(())
    }
}

#[derive(Clone, Default)]
struct MemoryMigratorSource {
    records: Arc<Mutex<Vec<MigrationRecord>>>,
}

#[async_trait::async_trait]
impl MigratorSource for MemoryMigratorSource {
    async fn list_records(&self) -> OrbitResult<Vec<MigrationRecord>> {
        let records = self.records.lock().unwrap();
        Ok(records.clone())
    }

    async fn add_record(&self, record: MigrationRecord) -> OrbitResult<()> {
        self.records.lock().unwrap().push(record);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    // Initialize the handler to do migrations, the source to store migration
    // records, and the migrations list.
    let handler = MemoryHandler::default();
    let source = MemoryMigratorSource::default();
    let migrations = vec![Box::new(MigrationV001) as Box<dyn Migration<MemoryHandler>>];

    // Build the migrator and apply the migrations.
    let migrator = Migrator::builder()
        .handler(handler.clone())
        .source(source.clone())
        .migrations(migrations)
        .build()
        .unwrap();
    migrator.up().await.unwrap();

    // The migrations were applied successfully.
    assert_eq!(handler.lock().unwrap().len(), 1);
    assert_eq!(handler.lock().unwrap()[0], "foobar");

    // The migration record was added to the source.
    let records = source.list_records().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].version, 1);
    assert_eq!(records[0].name, "add foobar");
}
