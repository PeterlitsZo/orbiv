use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use orbiv::{Migration, MigrationRecord, Migrator, MigratorSource, MigratorSteps, OrbivResult};

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

    async fn up(&self, handler: &MemoryHandler) -> OrbivResult<()> {
        handler.lock().unwrap().push("foobar".to_string());
        Ok(())
    }

    async fn down(&self, handler: &MemoryHandler) -> OrbivResult<()> {
        handler.lock().unwrap().pop();
        Ok(())
    }
}

struct MigrationV002;

#[async_trait::async_trait]
impl Migration<MemoryHandler> for MigrationV002 {
    fn version(&self) -> u64 {
        2
    }

    fn name(&self) -> &str {
        "add barfoo"
    }

    fn description(&self) -> &str {
        "Adds a 'barfoo' string to the memory handler."
    }

    async fn up(&self, handler: &MemoryHandler) -> OrbivResult<()> {
        handler.lock().unwrap().push("barfoo".to_string());
        Ok(())
    }

    async fn down(&self, handler: &MemoryHandler) -> OrbivResult<()> {
        handler.lock().unwrap().pop();
        Ok(())
    }
}

#[derive(Clone, Default)]
struct MemoryMigratorSource {
    records: Arc<Mutex<Vec<MigrationRecord>>>,
    install_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl MigratorSource for MemoryMigratorSource {
    async fn install(&self) -> OrbivResult<()> {
        self.install_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn list_records(&self) -> OrbivResult<Vec<MigrationRecord>> {
        let records = self.records.lock().unwrap();
        Ok(records.clone())
    }

    async fn add_record(&self, record: MigrationRecord) -> OrbivResult<()> {
        self.records.lock().unwrap().push(record);
        Ok(())
    }

    async fn remove_record(&self, version: u64) -> OrbivResult<()> {
        let mut records = self.records.lock().unwrap();
        records.retain(|r| r.version != version);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    // Initialize the handler to do migrations, the source to store migration
    // records, and the migrations list.
    let handler = MemoryHandler::default();
    let source = MemoryMigratorSource::default();
    let migrations: Vec<Box<dyn Migration<MemoryHandler>>> =
        vec![Box::new(MigrationV001), Box::new(MigrationV002)];

    // Build the migrator and apply the migrations.
    let migrator = Migrator::builder()
        .handler(handler.clone())
        .source(source.clone())
        .migrations(migrations)
        .build()
        .unwrap();

    // Reading the initial version installs the source once. Later operations
    // reuse that successful installation.
    assert_eq!(migrator.current_version().await.unwrap(), 0);
    assert_eq!(source.install_count.load(Ordering::Relaxed), 1);

    migrator.up(MigratorSteps::All).await.unwrap();
    assert_eq!(migrator.current_version().await.unwrap(), 2);
    assert_eq!(source.install_count.load(Ordering::Relaxed), 1);

    // The migrations were applied successfully.
    assert_eq!(handler.lock().unwrap().len(), 2);
    assert_eq!(handler.lock().unwrap()[0], "foobar");
    assert_eq!(handler.lock().unwrap()[1], "barfoo");

    // The migration record was added to the source.
    let records = source.list_records().await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].version, 1);
    assert_eq!(records[0].name, "add foobar");
    assert_eq!(records[1].version, 2);
    assert_eq!(records[1].name, "add barfoo");

    // Revert the migrations.
    migrator.down(MigratorSteps::Number(1)).await.unwrap();
    assert_eq!(migrator.current_version().await.unwrap(), 1);
    assert_eq!(source.install_count.load(Ordering::Relaxed), 1);
    assert_eq!(handler.lock().unwrap().len(), 1);
    assert_eq!(handler.lock().unwrap()[0], "foobar");
    let records = source.list_records().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].version, 1);
    assert_eq!(records[0].name, "add foobar");

    println!("Done.");
}
