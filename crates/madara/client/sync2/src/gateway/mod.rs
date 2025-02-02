use crate::{
    apply_state::ApplyStateSync,
    import::BlockImporter,
    metrics::SyncMetrics,
    pipeline::{ApplyOutcome, PipelineController, PipelineSteps},
    sync::{ForwardPipeline, Probe, SyncController, SyncControllerConfig},
    util::AbortOnDrop,
};
use anyhow::Context;
use classes::ClassesSync;
use mc_db::MadaraBackend;
use mc_gateway_client::GatewayProvider;
use mp_block::{BlockHeaderWithSignatures, BlockId, BlockTag, Header, TransactionWithReceipt};
use mp_chain_config::{StarknetVersion, StarknetVersionError};
use mp_gateway::state_update::{ProviderStateUpdateWithBlock, ProviderStateUpdateWithBlockPendingMaybe};
use mp_receipt::EventWithTransactionHash;
use mp_state_update::StateDiff;
use starknet_core::types::Felt;
use std::{iter, ops::Range, sync::Arc};

mod classes;

struct GatewayBlock {
    block_hash: Felt,
    header: Header,
    state_diff: StateDiff,
    transactions: Vec<TransactionWithReceipt>,
    events: Vec<EventWithTransactionHash>,
}

#[derive(Debug, thiserror::Error)]
enum FromGatewayError {
    #[error("Transaction count is not equal to receipt count")]
    TransactionCountNotEqualToReceiptCount,
    #[error("Invalid starknet version: {0:#}")]
    StarknetVersion(#[from] StarknetVersionError),
    #[error("Unable to determine Starknet version for block {0:#x}")]
    FromMainnetStarknetVersion(Felt),
}

impl TryFrom<ProviderStateUpdateWithBlock> for GatewayBlock {
    type Error = FromGatewayError;
    fn try_from(value: ProviderStateUpdateWithBlock) -> Result<Self, Self::Error> {
        if value.block.transactions.len() != value.block.transaction_receipts.len() {
            return Err(FromGatewayError::TransactionCountNotEqualToReceiptCount);
        }
        let state_diff = mp_state_update::StateDiff::from(value.state_update.state_diff);
        Ok(GatewayBlock {
            block_hash: value.block.block_hash,
            header: Header {
                parent_block_hash: value.block.parent_block_hash,
                sequencer_address: value.block.sequencer_address.unwrap_or_default(),
                block_timestamp: mp_block::header::BlockTimestamp(value.block.timestamp),
                protocol_version: value
                    .block
                    .starknet_version
                    .as_deref()
                    .map(|version| Ok(version.parse()?))
                    .unwrap_or_else(|| {
                        StarknetVersion::try_from_mainnet_block_number(value.block.block_number)
                            .ok_or(FromGatewayError::FromMainnetStarknetVersion(value.block.block_hash))
                    })?,
                l1_gas_price: mp_block::header::GasPrices {
                    eth_l1_gas_price: value.block.l1_gas_price.price_in_wei,
                    strk_l1_gas_price: value.block.l1_gas_price.price_in_fri,
                    eth_l1_data_gas_price: value.block.l1_data_gas_price.price_in_wei,
                    strk_l1_data_gas_price: value.block.l1_data_gas_price.price_in_fri,
                },
                l1_da_mode: value.block.l1_da_mode,
                block_number: value.block.block_number,
                global_state_root: value.block.state_root,
                transaction_count: value.block.transactions.len() as u64,
                transaction_commitment: value.block.transaction_commitment,
                event_count: value.block.transaction_receipts.iter().map(|r| r.events.len() as u64).sum(),
                event_commitment: value.block.event_commitment,
                state_diff_length: Some(state_diff.len() as u64),
                state_diff_commitment: value.block.state_diff_commitment,
                receipt_commitment: value.block.receipt_commitment,
            },
            events: value
                .block
                .transaction_receipts
                .iter()
                .flat_map(|receipt| {
                    receipt
                        .events
                        .iter()
                        .cloned()
                        .map(|event| EventWithTransactionHash { transaction_hash: receipt.transaction_hash, event })
                })
                .collect(),
            transactions: value
                .block
                .transactions
                .into_iter()
                .zip(value.block.transaction_receipts)
                .map(|(transaction, receipt)| TransactionWithReceipt {
                    receipt: receipt.into_mp(&transaction),
                    transaction: transaction.into(),
                })
                .collect(),
            state_diff,
        })
    }
}

pub type GatewayBlockSync = PipelineController<GatewaySyncSteps>;
pub fn block_with_state_update_pipeline(
    backend: Arc<MadaraBackend>,
    importer: Arc<BlockImporter>,
    client: Arc<GatewayProvider>,
    parallelization: usize,
    batch_size: usize,
) -> GatewayBlockSync {
    PipelineController::new(GatewaySyncSteps { backend, importer, client }, parallelization, batch_size)
}

// TODO: check that the headers follow each other
pub struct GatewaySyncSteps {
    backend: Arc<MadaraBackend>,
    importer: Arc<BlockImporter>,
    client: Arc<GatewayProvider>,
}
impl PipelineSteps for GatewaySyncSteps {
    type InputItem = ();
    type SequentialStepInput = Vec<StateDiff>;
    type Output = Vec<StateDiff>;

    async fn parallel_step(
        self: Arc<Self>,
        block_range: Range<u64>,
        _input: Vec<Self::InputItem>,
    ) -> anyhow::Result<Self::SequentialStepInput> {
        AbortOnDrop::spawn(async move {
            let mut out = vec![];
            tracing::debug!("Gateway sync parallel step {:?}", block_range);
            for block_n in block_range {
                let block = self
                    .client
                    .get_state_update_with_block(BlockId::Number(block_n))
                    .await
                    .with_context(|| format!("Getting state update with block_n={block_n}"))?;

                let ProviderStateUpdateWithBlockPendingMaybe::NonPending(block) = block else {
                    anyhow::bail!("Asked for a block_n, got a pending one")
                };

                let gateway_block: GatewayBlock = block.try_into().context("Parsing gateway block")?;

                let state_diff = self
                    .importer
                    .run_in_rayon_pool(move |importer| {
                        let mut signed_header = BlockHeaderWithSignatures {
                            header: gateway_block.header,
                            block_hash: gateway_block.block_hash,
                            consensus_signatures: vec![],
                        };

                        // Fill in the header with the commitments missing in pre-v0.13.2 headers from the gateway.
                        let allow_pre_v0_13_2 = true;

                        let state_diff_commitment = importer.verify_state_diff(
                            block_n,
                            &gateway_block.state_diff,
                            &signed_header.header,
                            allow_pre_v0_13_2,
                        )?;
                        let (transaction_commitment, receipt_commitment) = importer.verify_transactions(
                            block_n,
                            &gateway_block.transactions,
                            &signed_header.header,
                            allow_pre_v0_13_2,
                        )?;
                        let event_commitment = importer.verify_events(
                            block_n,
                            &gateway_block.events,
                            &signed_header.header,
                            allow_pre_v0_13_2,
                        )?;
                        signed_header.header = Header {
                            state_diff_commitment: Some(state_diff_commitment),
                            transaction_commitment,
                            event_commitment,
                            receipt_commitment: Some(receipt_commitment),
                            ..signed_header.header
                        };
                        importer.verify_header(block_n, &signed_header)?;

                        importer.save_header(block_n, signed_header)?;
                        importer.save_state_diff(block_n, gateway_block.state_diff.clone())?;
                        importer.save_transactions(block_n, gateway_block.transactions)?;
                        importer.save_events(block_n, gateway_block.events)?;

                        anyhow::Ok(gateway_block.state_diff)
                    })
                    .await?;
                out.push(state_diff);
            }
            Ok(out)
        })
        .await
    }
    async fn sequential_step(
        self: Arc<Self>,
        block_range: Range<u64>,
        input: Self::SequentialStepInput,
    ) -> anyhow::Result<ApplyOutcome<Self::Output>> {
        tracing::debug!("Gateway sync sequential step: {block_range:?}");
        if let Some(block_n) = block_range.last() {
            self.backend.head_status().headers.set(Some(block_n));
            self.backend.head_status().state_diffs.set(Some(block_n));
            self.backend.head_status().transactions.set(Some(block_n));
            self.backend.head_status().events.set(Some(block_n));
            self.backend.save_head_status_to_db()?;
        }
        Ok(ApplyOutcome::Success(input))
    }

    fn starting_block_n(&self) -> Option<u64> {
        self.backend.head_status().latest_full_block_n()
    }
}

pub struct ForwardSyncConfig {
    pub block_parallelization: usize,
    pub block_batch_size: usize,
    pub classes_parallelization: usize,
    pub classes_batch_size: usize,
    pub apply_state_parallelization: usize,
    pub apply_state_batch_size: usize,
    pub disable_tries: bool,
}

impl Default for ForwardSyncConfig {
    fn default() -> Self {
        Self {
            block_parallelization: 100,
            block_batch_size: 1,
            classes_parallelization: 200,
            classes_batch_size: 1,
            apply_state_parallelization: 3,
            apply_state_batch_size: 5,
            disable_tries: false,
        }
    }
}

impl ForwardSyncConfig {
    pub fn disable_tries(self, val: bool) -> Self {
        Self { disable_tries: val, ..self }
    }
}

pub type GatewaySync = SyncController<GatewayForwardSync, GatewayLatestProbe>;
pub fn forward_sync(
    backend: Arc<MadaraBackend>,
    importer: Arc<BlockImporter>,
    client: Arc<GatewayProvider>,
    controller_config: SyncControllerConfig,
    config: ForwardSyncConfig,
) -> GatewaySync {
    let probe = GatewayLatestProbe::new(client.clone());
    SyncController::new(
        GatewayForwardSync::new(backend, importer, client, config),
        Some(probe.into()),
        controller_config,
    )
}

pub struct GatewayForwardSync {
    blocks_pipeline: GatewayBlockSync,
    classes_pipeline: ClassesSync,
    apply_state_pipeline: ApplyStateSync,
    backend: Arc<MadaraBackend>,
}

impl GatewayForwardSync {
    pub fn new(
        backend: Arc<MadaraBackend>,
        importer: Arc<BlockImporter>,
        client: Arc<GatewayProvider>,
        config: ForwardSyncConfig,
    ) -> Self {
        let blocks_pipeline = block_with_state_update_pipeline(
            backend.clone(),
            importer.clone(),
            client.clone(),
            config.block_parallelization,
            config.block_batch_size,
        );
        let classes_pipeline = classes::classes_pipeline(
            backend.clone(),
            importer.clone(),
            client.clone(),
            config.classes_parallelization,
            config.classes_batch_size,
        );
        let apply_state_pipeline = super::apply_state::apply_state_pipeline(
            backend.clone(),
            importer.clone(),
            config.apply_state_parallelization,
            config.apply_state_batch_size,
            config.disable_tries,
        );
        Self { blocks_pipeline, classes_pipeline, apply_state_pipeline, backend }
    }
}

impl ForwardPipeline for GatewayForwardSync {
    async fn run(&mut self, target_height: u64, metrics: &mut SyncMetrics) -> anyhow::Result<()> {
        tracing::debug!("Run pipeline to height={target_height:?}");
        loop {
            while self.blocks_pipeline.can_schedule_more() && self.blocks_pipeline.next_input_block_n() <= target_height
            {
                let next_input_block_n = self.blocks_pipeline.next_input_block_n();
                self.blocks_pipeline.push(next_input_block_n..next_input_block_n + 1, iter::once(()));
            }

            let next_full_block = self.backend.head_status().next_full_block();

            tokio::select! {
                Some(res) = self.apply_state_pipeline.next() => {
                    res?;
                }
                Some(res) = self.classes_pipeline.next() => {
                    res?;
                }
                Some(res) = self.blocks_pipeline.next(), if self.classes_pipeline.can_schedule_more() && self.apply_state_pipeline.can_schedule_more() => {
                    let (range, state_diffs) = res?;
                    self.classes_pipeline.push(range.clone(), state_diffs.iter().map(|s| s.all_declared_classes()));
                    self.apply_state_pipeline.push(range, state_diffs);
                }
                // all pipelines are empty, we're done :)
                else => break Ok(())
            }

            let new_next_full_block = self.backend.head_status().next_full_block();
            for block_n in next_full_block..new_next_full_block {
                // Notify of a new full block here.
                metrics.update(block_n, &self.backend).context("Updating metrics")?;
            }
        }
    }

    fn next_input_block_n(&self) -> u64 {
        self.blocks_pipeline.next_input_block_n()
    }

    fn is_empty(&self) -> bool {
        self.blocks_pipeline.is_empty() && self.classes_pipeline.is_empty() && self.apply_state_pipeline.is_empty()
    }

    fn show_status(&self) {
        tracing::info!(
            "📥 Blocks: {} | Classes: {} | State: {}",
            self.blocks_pipeline.status(),
            self.classes_pipeline.status(),
            self.apply_state_pipeline.status(),
        );
    }

    fn latest_block(&self) -> Option<u64> {
        self.backend.head_status().latest_full_block_n()
    }
}

pub struct GatewayLatestProbe {
    client: Arc<GatewayProvider>,
}

impl GatewayLatestProbe {
    pub fn new(client: Arc<GatewayProvider>) -> Self {
        Self { client }
    }
}

impl Probe for GatewayLatestProbe {
    async fn forward_probe(self: Arc<Self>, _next_block_n: u64) -> anyhow::Result<Option<u64>> {
        let header = self.client.get_header(BlockId::Tag(BlockTag::Latest)).await.context("Getting latest header")?;
        Ok(Some(header.block_number))
    }
}
