use crate::{
    controller::PipelineController,
    import::BlockImporter,
    p2p::{P2pError, P2pPipelineArguments, P2pPipelineController, P2pPipelineSteps},
};
use futures::TryStreamExt;
use mc_db::{stream::BlockStreamConfig, MadaraBackend};
use mc_p2p::{P2pCommands, PeerId};
use mp_block::Header;
use mp_state_update::StateDiff;
use std::{ops::Range, sync::Arc};

pub type StateDiffsSync = PipelineController<P2pPipelineController<StateDiffsSyncSteps>>;
pub fn state_diffs_pipeline(
    P2pPipelineArguments { backend, peer_set, p2p_commands, importer }: P2pPipelineArguments,
    parallelization: usize,
    batch_size: usize,
) -> StateDiffsSync {
    PipelineController::new(
        P2pPipelineController::new(peer_set, StateDiffsSyncSteps { backend, p2p_commands, importer }),
        parallelization,
        batch_size,
    )
}
pub struct StateDiffsSyncSteps {
    backend: Arc<MadaraBackend>,
    p2p_commands: P2pCommands,
    importer: Arc<BlockImporter>,
}

impl P2pPipelineSteps for StateDiffsSyncSteps {
    type InputItem = Header;
    type SequentialStepInput = Vec<StateDiff>;
    type Output = Vec<StateDiff>;

    async fn p2p_parallel_step(
        self: Arc<Self>,
        peer_id: PeerId,
        block_range: Range<u64>,
        input: Vec<Self::InputItem>,
    ) -> Result<Self::SequentialStepInput, P2pError> {
        tracing::debug!("p2p state_diffs parallel step: {block_range:?}, peer_id: {peer_id}");
        let strm = self
            .p2p_commands
            .clone()
            .make_state_diffs_stream(
                peer_id,
                BlockStreamConfig::default().with_block_range(block_range.clone()),
                input.iter().map(|header| header.state_diff_length.unwrap_or_default() as _).collect::<Vec<_>>(),
            )
            .await;
        tokio::pin!(strm);

        let mut state_diffs = vec![];
        for (block_n, header) in block_range.zip(input) {
            let state_diff = strm.try_next().await?.ok_or(P2pError::peer_error("Expected to receive item"))?;
            state_diffs.push(state_diff.clone());
            self.importer.verify_and_save_state_diff(block_n, state_diff, header).await?;
        }

        Ok(state_diffs)
    }

    async fn p2p_sequential_step(
        self: Arc<Self>,
        peer_id: PeerId,
        block_range: Range<u64>,
        input: Self::SequentialStepInput,
    ) -> Result<Self::Output, P2pError> {
        tracing::debug!("p2p state_diffs sequential step: {block_range:?}, peer_id: {peer_id}");
        if let Some(block_n) = block_range.last() {
            self.backend.head_status().state_diffs.set(Some(block_n));
        }
        Ok(input)
    }

    fn starting_block_n(&self) -> Option<u64> {
        self.backend.head_status().state_diffs.get()
    }
}
