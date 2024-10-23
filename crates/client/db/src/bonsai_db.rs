use crate::error::DbError;
use crate::rocksdb_snapshot::SnapshotWithDBArc;
use crate::snapshots::Snapshots;
use crate::{Column, DatabaseExt, WriteBatchWithTransaction, DB};
use bonsai_trie::id::BasicId;
use bonsai_trie::{BonsaiDatabase, BonsaiPersistentDatabase, BonsaiStorage, ByteVec, DatabaseKey};
use rocksdb::{Direction, IteratorMode, WriteOptions};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

pub type GlobalTrie<H> = BonsaiStorage<BasicId, BonsaiDb, H>;

#[derive(Clone, Debug)]
pub(crate) struct DatabaseKeyMapping {
    pub(crate) flat: Column,
    pub(crate) trie: Column,
    pub(crate) log: Column,
}

impl DatabaseKeyMapping {
    pub(crate) fn map(&self, key: &DatabaseKey) -> Column {
        match key {
            DatabaseKey::Trie(_) => self.trie,
            DatabaseKey::Flat(_) => self.flat,
            DatabaseKey::TrieLog(_) => self.log,
        }
    }
}

pub struct BonsaiDb {
    db: Arc<DB>,
    /// Mapping from `DatabaseKey` => rocksdb column name
    column_mapping: DatabaseKeyMapping,
    snapshots: Arc<Snapshots>,
    write_opt: WriteOptions,
}

impl BonsaiDb {
    pub(crate) fn new(db: Arc<DB>, snapshots: Arc<Snapshots>, column_mapping: DatabaseKeyMapping) -> Self {
        let mut write_opt = WriteOptions::default();
        write_opt.disable_wal(true);
        Self { db, column_mapping, write_opt, snapshots }
    }
}

impl fmt::Debug for BonsaiDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<database>")
    }
}

impl BonsaiDatabase for BonsaiDb {
    type Batch = WriteBatchWithTransaction;
    type DatabaseError = DbError;

    fn create_batch(&self) -> Self::Batch {
        Self::Batch::default()
    }

    fn get(&self, key: &DatabaseKey) -> Result<Option<ByteVec>, Self::DatabaseError> {
        log::trace!("Getting from RocksDB: {:?}", key);
        let handle = self.db.get_column(self.column_mapping.map(key));
        Ok(self.db.get_cf(&handle, key.as_slice())?.map(Into::into))
    }

    fn get_by_prefix(&self, prefix: &DatabaseKey) -> Result<Vec<(ByteVec, ByteVec)>, Self::DatabaseError> {
        log::trace!("Getting from RocksDB: {:?}", prefix);
        let handle = self.db.get_column(self.column_mapping.map(prefix));
        let iter = self.db.iterator_cf(&handle, IteratorMode::From(prefix.as_slice(), Direction::Forward));
        Ok(iter
            .map_while(|kv| {
                if let Ok((key, value)) = kv {
                    if key.starts_with(prefix.as_slice()) {
                        // nb: to_vec on a Box<[u8]> is a noop conversion
                        Some((key.to_vec().into(), value.to_vec().into()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect())
    }

    fn contains(&self, key: &DatabaseKey) -> Result<bool, Self::DatabaseError> {
        log::trace!("Checking if RocksDB contains: {:?}", key);
        let handle = self.db.get_column(self.column_mapping.map(key));
        Ok(self.db.get_cf(&handle, key.as_slice()).map(|value| value.is_some())?)
    }

    fn insert(
        &mut self,
        key: &DatabaseKey,
        value: &[u8],
        batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        log::trace!("Inserting into RocksDB: {:?} {:?}", key, value);
        let handle = self.db.get_column(self.column_mapping.map(key));

        // NB: we don't need old value as the trie log is not used :)
        // this actually speeds up things quite a lot

        log::debug!("Insert!: {:?}", key.as_slice());

        let old_value = self.db.get_cf(&handle, key.as_slice())?;
        if let Some(batch) = batch {
            batch.put_cf(&handle, key.as_slice(), value);
        } else {
            self.db.put_cf_opt(&handle, key.as_slice(), value, &self.write_opt)?;
        }
        Ok(old_value.map(Into::into))
    }

    fn remove(
        &mut self,
        key: &DatabaseKey,
        batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        log::trace!("Removing from RocksDB: {:?}", key);
        let handle = self.db.get_column(self.column_mapping.map(key));
        let old_value = self.db.get_cf(&handle, key.as_slice())?;
        if let Some(batch) = batch {
            batch.delete_cf(&handle, key.as_slice());
        } else {
            self.db.delete_cf_opt(&handle, key.as_slice(), &self.write_opt)?;
        }
        Ok(old_value.map(Into::into))
    }

    fn remove_by_prefix(&mut self, prefix: &DatabaseKey) -> Result<(), Self::DatabaseError> {
        let handle = self.db.get_column(self.column_mapping.map(prefix));
        let iter = self.db.iterator_cf(&handle, IteratorMode::From(prefix.as_slice(), Direction::Forward));
        let mut batch = self.create_batch();
        for kv in iter {
            if let Ok((key, _)) = kv {
                if key.starts_with(prefix.as_slice()) {
                    batch.delete_cf(&handle, &key);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        drop(handle);
        self.write_batch(batch)?;
        Ok(())
    }

    fn write_batch(&mut self, batch: Self::Batch) -> Result<(), Self::DatabaseError> {
        Ok(self.db.write_opt(batch, &self.write_opt)?)
    }
}

fn to_changed_key(k: &DatabaseKey) -> (usize, ByteVec) {
    (
        match k {
            DatabaseKey::Trie(_) => 0,
            DatabaseKey::Flat(_) => 1,
            DatabaseKey::TrieLog(_) => 2,
        },
        k.as_slice().into(),
    )
}

pub struct BonsaiTransaction {
    snapshot: Arc<SnapshotWithDBArc<DB>>,
    changed: BTreeMap<(usize, ByteVec), Option<ByteVec>>,
    db: Arc<DB>,
    column_mapping: DatabaseKeyMapping,
}

impl fmt::Debug for BonsaiTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<tx>")
    }
}

impl BonsaiDatabase for BonsaiTransaction {
    type Batch = WriteBatchWithTransaction;
    type DatabaseError = DbError;

    fn create_batch(&self) -> Self::Batch {
        Default::default()
    }

    fn get(&self, key: &DatabaseKey) -> Result<Option<ByteVec>, Self::DatabaseError> {
        log::trace!("Getting from RocksDB: {:?}", key);
        if let Some(val) = self.changed.get(&to_changed_key(key)) {
            return Ok(val.clone());
        }
        let handle = self.db.get_column(self.column_mapping.map(key));
        Ok(self.db.get_cf(&handle, key.as_slice())?.map(Into::into))
    }

    fn get_by_prefix(&self, _prefix: &DatabaseKey) -> Result<Vec<(ByteVec, ByteVec)>, Self::DatabaseError> {
        unreachable!()
        // log::trace!("Getting from RocksDB: {:?}", prefix);
        // let handle = self.db.get_column(self.column_mapping.map(prefix));
        // let iter = self.snapshot.iterator_cf(&handle, IteratorMode::From(prefix.as_slice(), Direction::Forward));
        // Ok(iter
        //     .map_while(|kv| {
        //         if let Ok((key, value)) = kv {
        //             if key.starts_with(prefix.as_slice()) {
        //                 Some((key.to_vec().into(), value.to_vec().into()))
        //             } else {
        //                 None
        //             }
        //         } else {
        //             None
        //         }
        //     })
        //     .collect())
    }

    fn contains(&self, key: &DatabaseKey) -> Result<bool, Self::DatabaseError> {
        log::trace!("Checking if RocksDB contains: {:?}", key);
        let handle = self.db.get_column(self.column_mapping.map(key));
        Ok(self.snapshot.get_cf(&handle, key.as_slice())?.is_some())
    }

    fn insert(
        &mut self,
        key: &DatabaseKey,
        value: &[u8],
        _batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        self.changed.insert(to_changed_key(key), Some(value.into()));
        Ok(None)
    }

    fn remove(
        &mut self,
        key: &DatabaseKey,
        _batch: Option<&mut Self::Batch>,
    ) -> Result<Option<ByteVec>, Self::DatabaseError> {
        self.changed.insert(to_changed_key(key), None);
        Ok(None)
    }

    fn remove_by_prefix(&mut self, _prefix: &DatabaseKey) -> Result<(), Self::DatabaseError> {
        unreachable!()
    }

    fn write_batch(&mut self, _batch: Self::Batch) -> Result<(), Self::DatabaseError> {
        // self.txn.
        // unreachable!()
        Ok(())
        // Ok(self.txn.rebuild_from_writebatch(&batch)?)
    }
}

impl BonsaiPersistentDatabase<BasicId> for BonsaiDb {
    type Transaction<'a> = BonsaiTransaction where Self: 'a;
    type DatabaseError = DbError;

    fn snapshot(&mut self, id: BasicId) {
        log::debug!("Generating RocksDB snapshot");
        self.snapshots.create_new(id);
    }

    fn transaction(&self, id: BasicId) -> Option<(BasicId, Self::Transaction<'_>)> {
        log::trace!("Generating RocksDB transaction");
        if let Some((id, snapshot)) = self.snapshots.get_closest(id) {
            Some((
                id,
                BonsaiTransaction {
                    db: Arc::clone(&self.db),
                    snapshot,
                    column_mapping: self.column_mapping.clone(),
                    changed: Default::default(),
                },
            ))
        } else {
            None
        }
    }

    fn merge<'a>(&mut self, _transaction: Self::Transaction<'a>) -> Result<(), Self::DatabaseError>
    where
        Self: 'a,
    {
        // transaction.txn.commit()?;
        unreachable!()
    }
}
