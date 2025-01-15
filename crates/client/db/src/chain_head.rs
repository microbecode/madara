use crate::DatabaseExt;
use crate::{Column, MadaraBackend, MadaraStorageError};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
#[serde(transparent)]
pub struct BlockNStatus(AtomicU64);

impl BlockNStatus {
    pub fn get(&self) -> Option<u64> {
        self.0.load(SeqCst).checked_sub(1)
    }
    pub fn set(&self, block_n: Option<u64>) {
        self.0.store(block_n.map(|block_n| block_n + 1).unwrap_or(0), SeqCst)
    }
}

impl Clone for BlockNStatus {
    fn clone(&self) -> Self {
        Self(self.0.load(SeqCst).into())
    }
}

/// Counter of the latest block currently in the database.
/// We have multiple counters because the sync pipeline is split in sub-pipelines.
#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct ChainHead {
    pub headers: BlockNStatus,
    pub state_diffs: BlockNStatus,
    pub classes: BlockNStatus,
    pub transactions: BlockNStatus,
    pub events: BlockNStatus,
    pub l1_head: BlockNStatus,
    pub global_trie: BlockNStatus,
}

impl ChainHead {
    pub fn latest_full_block_n(&self) -> Option<u64> {
        self.headers
            .get()
            .max(self.state_diffs.get())
            .max(self.classes.get())
            .max(self.transactions.get())
            .max(self.events.get())
            .max(self.global_trie.get())
    }
}

const ROW_HEAD_STATUS: &[u8] = b"head_status";

impl MadaraBackend {
    pub fn head_status(&self) -> &ChainHead {
        &self.head_status
    }
    pub fn load_head_status_from_db(&mut self) -> Result<(), MadaraStorageError> {
        let col = self.db.get_column(Column::BlockStorageMeta);
        self.head_status = Default::default();
        if let Some(res) = self.db.get_pinned_cf(&col, ROW_HEAD_STATUS)? {
            self.head_status = bincode::deserialize(res.as_ref())?;
        }
        Ok(())
    }
}
