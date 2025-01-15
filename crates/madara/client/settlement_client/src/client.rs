use crate::gas_price::L1BlockMetrics;
use crate::messaging::CommonMessagingEventData;
use crate::state_update::StateUpdate;
use async_trait::async_trait;
use futures::Stream;
use mc_db::l1_db::LastSyncedEventBlock;
use mc_db::MadaraBackend;
use mp_utils::service::ServiceContext;
use starknet_types_core::felt::Felt;
use std::sync::Arc;

pub enum ClientType {
    ETH,
    STARKNET,
}

#[async_trait]
pub trait ClientTrait: Send + Sync {
    // Configuration type used for initialization
    type Config;

    // Get client type
    fn get_client_type(&self) -> ClientType;

    // Create a new instance of the client
    async fn new(config: Self::Config) -> anyhow::Result<Self>
    where
        Self: Sized;

    // Get the latest block number
    async fn get_latest_block_number(&self) -> anyhow::Result<u64>;

    // Get the block number of the last occurrence of the state update event
    async fn get_last_event_block_number(&self) -> anyhow::Result<u64>;

    // Get the last verified block number
    async fn get_last_verified_block_number(&self) -> anyhow::Result<u64>;

    // Get the last state root
    // - change this to Felt in implementation
    // - write tests for conversion to Felt from <native-type>
    async fn get_last_verified_state_root(&self) -> anyhow::Result<Felt>;

    // Get the last verified block hash
    async fn get_last_verified_block_hash(&self) -> anyhow::Result<Felt>;

    // Get initial state from client
    async fn get_initial_state(&self) -> anyhow::Result<StateUpdate>;

    // Listen for update state events
    async fn listen_for_update_state_events(
        &self,
        backend: Arc<MadaraBackend>,
        ctx: ServiceContext,
        l1_block_metrics: Arc<L1BlockMetrics>,
    ) -> anyhow::Result<()>;

    // get gas prices
    async fn get_gas_prices(&self) -> anyhow::Result<(u128, u128)>;

    // Get message hash from event
    fn get_messaging_hash(&self, event: &CommonMessagingEventData) -> anyhow::Result<Vec<u8>>;

    /// Get cancellation status of an L1 to L2 message
    ///
    /// This function query the core contract to know if a L1->L2 message has been cancelled
    /// # Arguments
    ///
    /// - msg_hash : Hash of L1 to L2 message
    ///
    /// # Return
    ///
    /// - A felt representing a timestamp :
    ///     - 0 if the message has not been cancelled
    ///     - timestamp of the cancellation if it has been cancelled
    /// - An Error if the call fail
    async fn get_l1_to_l2_message_cancellations(&self, msg_hash: Vec<u8>) -> anyhow::Result<Felt>;

    // ============================================================
    // Stream Implementations :
    // ============================================================
    type StreamType: Stream<Item = Option<anyhow::Result<CommonMessagingEventData>>>;
    async fn get_event_stream(&self, last_synced_event_block: LastSyncedEventBlock)
        -> anyhow::Result<Self::StreamType>;
}
