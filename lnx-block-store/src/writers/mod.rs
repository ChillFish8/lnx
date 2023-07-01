use std::io;
use std::path::Path;
use async_trait::async_trait;
use crate::metastore_wrapper::StorageMetastore;


/// The writer responsible for safely persisting doc blocks to disk
/// doubling as a WAL for the tantivy index as well.
pub struct BlockStoreWriter {
    segment_writer: AutoWriter,
    metastore: StorageMetastore,
    writer_id: u64,
}

impl BlockStoreWriter {
    /// Creates a new file or opens an existing file for
    /// the doc store writer.
    pub async fn open(
        file_path: &Path,
        metastore: StorageMetastore,
        writer_id: u64,
    ) -> io::Result<Self> {
        let segment_writer = AutoWriter::open(file_path).await?;

        Ok(Self {
            segment_writer,
            metastore,
            writer_id,
        })
    }

    #[inline]
    /// Writes a block of data into the doc store.
    ///
    /// Returns the position the bytes were written at.
    pub async fn write_all(&mut self, bytes: &[u8]) -> io::Result<u64> {
        self.segment_writer.write_all(bytes).await
    }

    #[inline]
    /// Ensures any pending data written to the doc store is safely persisted to disk.
    /// 
    /// This internally advances the checkpoint cursor that will be stored
    /// when a index is committed.
    pub async fn flush(&mut self) -> io::Result<()> {
        self.segment_writer.flush().await
    }

    #[inline]
    /// Stores the new commit checkpoint in the metastore.
    pub fn set_commit_checkpoint(&mut self, pos: u64) -> anyhow::Result<()> {
        self.metastore.set_checkpoint(self.writer_id, pos)
    }
}

#[async_trait]
pub trait SegmentWriter: Sized {
    /// Opens a segment for writing.
    ///
    /// If the file already exists the cursor should start writing from the end of the file.
    async fn open(path: &Path) -> io::Result<Self>;

    /// Writes a chunk of data to the store.
    async fn write_all(&mut self, bytes: &[u8]) -> io::Result<u64>;

    /// Ensures all data written to the store is safely persisted on disk.
    ///
    /// Returns the current position of the writer which is safely persisted.
    async fn flush(&mut self) -> io::Result<()>;
}


pub enum AutoWriter {

}

#[async_trait]
impl SegmentWriter for AutoWriter {
    async fn open(path: &Path) -> io::Result<Self> {
        todo!()
    }

    async fn write_all(&mut self, bytes: &[u8]) -> io::Result<u64> {
        todo!()
    }

    async fn flush(&mut self) -> io::Result<()> {
        todo!()
    }
}