use anyhow::Result;
use parking_lot::RwLock;
use hashbrown::HashMap;

use crate::storage::StorageManager;
use crate::index::IndexHandler;
use crate::structures::IndexDeclaration;


/// A manager for a collection of indexes.
///
/// This will store index definitions as persistent json files and will
/// load any existing indexes at creation time.
pub struct SearchEngine {
    storage: StorageManager,
    indexes: RwLock<HashMap<String, IndexHandler>>,
}

impl SearchEngine {
    /// Creates a new search engine loading the existing index metadata
    /// from the given directory.
    pub async fn create(dir: &str) -> Result<Self> {
        let storage = StorageManager::with_directory(dir.to_string()).await?;
        let loaded_indexes = storage.load_all().await?;

        // load previously defined indexes on restart.
        let mut indexes = HashMap::with_capacity(loaded_indexes.len());
        for loader in loaded_indexes {
            let name = loader.name.clone();
            let index = IndexHandler::build_loaded(loader)?;

            indexes.insert(name, index);
        }

        Ok(Self {
            storage,
            indexes: RwLock::new(indexes),
        })
    }

    /// Adds a declared index to the search engine.
    ///
    /// This will set it in the index storage and then build the index handlers.
    pub async fn add_index(&self, index: &IndexDeclaration) -> Result<()> {
        self.storage.store_index_meta(index).await?;

        let loaded = index.into_schema();
        let index = IndexHandler::build_loaded(loaded)?;
    }
}

