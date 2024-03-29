use bitvec::prelude::*;
use bonsai_trie::databases::HashMapDb;
use bonsai_trie::id::{BasicId, BasicIdBuilder};
use bonsai_trie::{BonsaiStorage, BonsaiStorageConfig};
use mp_felt::Felt252Wrapper;
use mp_hashers::pedersen::PedersenHasher;
use mp_hashers::HasherT;
use mp_transactions::compute_hash::ComputeTransactionHash;
use starknet_api::transaction::Transaction;
use starknet_ff::FieldElement;
use starknet_types_core::felt::Felt;
use starknet_types_core::hash::Pedersen;
use tokio::task::{spawn_blocking, JoinSet};

use crate::utils::constant::bonsai_identifier;

/// Compute the combined hash of the transaction hash and the signature.
///
/// Since the transaction hash doesn't take the signature values as its input
/// computing the transaction commitent uses a hash value that combines
/// the transaction hash with the array of signature values.
///
/// # Arguments
///
/// * `transaction` - The transaction to compute the hash of.
///
/// # Returns
///
/// The transaction hash with signature.
pub fn calculate_transaction_hash_with_signature<H: HasherT>(
    transaction: &Transaction,
    chain_id: Felt252Wrapper,
    block_number: u64,
) -> FieldElement
where
    H: HasherT,
{
    let include_signature = block_number >= 61394;

    let signature_hash = if matches!(transaction, Transaction::Invoke(_)) || include_signature {
        // Include signatures for Invoke transactions or for all transactions
        // starting from block 61394
        H::compute_hash_on_elements(
            &transaction.signature().iter().map(|elt| FieldElement::from(*elt)).collect::<Vec<FieldElement>>(),
        )
    } else {
        // Before block 61394, and for non-Invoke transactions, signatures are not included
        H::compute_hash_on_elements(&[])
    };

    H::hash_elements(
        FieldElement::from(transaction.compute_hash::<H>(chain_id, false, Some(block_number))),
        signature_hash,
    )
}

/// Calculate the transaction commitment in memory using HashMapDb (which is more efficient for this
/// usecase).
///
/// # Arguments
///
/// * `transactions` - The transactions of the block
/// * `chain_id` - The current chain id
/// * `block_number` - The current block number
///
/// # Returns
///
/// The transaction commitment as `Felt252Wrapper`.
pub async fn memory_transaction_commitment(
    transactions: &[Transaction],
    chain_id: Felt252Wrapper,
    block_number: u64,
) -> Result<Felt252Wrapper, String> {
    // TODO @cchudant refacto/optimise this function
    let config = BonsaiStorageConfig::default();
    let bonsai_db = HashMapDb::<BasicId>::default();
    let mut bonsai_storage =
        BonsaiStorage::<_, _, Pedersen>::new(bonsai_db, config).expect("Failed to create bonsai storage");
    let identifier = bonsai_identifier::TRANSACTION;

    // transaction hashes are computed in parallel
    let mut task_set = JoinSet::new();
    transactions.iter().cloned().enumerate().for_each(|(i, tx)| {
        task_set.spawn(async move {
            (i, calculate_transaction_hash_with_signature::<PedersenHasher>(&tx, chain_id, block_number))
        });
    });

    // once transaction hashes have finished computing, they are inserted into the local Bonsai db
    while let Some(res) = task_set.join_next().await {
        let (i, tx_hash) = res.map_err(|e| format!("Failed to retrieve transaction hash: {e}"))?;
        let key = BitVec::from_vec(i.to_be_bytes().to_vec());
        let value = Felt::from(Felt252Wrapper::from(tx_hash));
        bonsai_storage.insert(identifier, key.as_bitslice(), &value).expect("Failed to insert into bonsai storage");
    }

    let mut id_builder = BasicIdBuilder::new();
    let id = id_builder.new_id();

    // run in a blocking-safe thread to avoid starving the thread pool
    let root_hash = spawn_blocking(move || {
        bonsai_storage.commit(id).expect("Failed to commit to bonsai storage");
        bonsai_storage.root_hash(identifier).expect("Failed to get root hash")
    })
    .await
    .map_err(|e| format!("Failed to computed transaction root hash: {e}"))?;

    Ok(Felt252Wrapper::from(root_hash))
}
