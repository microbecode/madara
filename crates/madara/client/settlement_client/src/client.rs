use crate::error::SettlementClientError;
use crate::gas_price::L1BlockMetrics;
use crate::messaging::CommonMessagingEventData;
use crate::state_update::StateUpdate;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::Stream;
use mc_db::l1_db::LastSyncedEventBlock;
use mc_db::MadaraBackend;
use mockall::automock;
use mp_utils::service::ServiceContext;
use starknet_types_core::felt::Felt;
use std::sync::Arc;

pub enum ClientType {
    ETH,
    STARKNET,
}

#[derive(Debug, Default, PartialEq)]
pub struct DummyConfig;
pub type DummyStream = BoxStream<'static, Option<Result<CommonMessagingEventData, SettlementClientError>>>;

#[automock(
    type Config = DummyConfig;
    type StreamType = DummyStream;
)]
#[async_trait]
pub trait SettlementClientTrait: Send + Sync {
    // Configuration type used for initialization
    type Config;

    // Get client type
    fn get_client_type(&self) -> ClientType;

    // Get the latest block number
    async fn get_latest_block_number(&self) -> Result<u64, SettlementClientError>;

    // Get the block number of the last occurrence of the state update event
    async fn get_last_event_block_number(&self) -> Result<u64, SettlementClientError>;

    // Get the last verified block number
    async fn get_last_verified_block_number(&self) -> Result<u64, SettlementClientError>;

    // Get the last state root
    // - change this to Felt in implementation
    // - write tests for conversion to Felt from <native-type>
    async fn get_last_verified_state_root(&self) -> Result<Felt, SettlementClientError>;

    // Get the last verified block hash
    async fn get_last_verified_block_hash(&self) -> Result<Felt, SettlementClientError>;

    // Get initial state from client
    async fn get_initial_state(&self) -> Result<StateUpdate, SettlementClientError>;

    // Listen for update state events
    async fn listen_for_update_state_events(
        &self,
        backend: Arc<MadaraBackend>,
        ctx: ServiceContext,
        l1_block_metrics: Arc<L1BlockMetrics>,
    ) -> Result<(), SettlementClientError>;

    // get gas prices
    async fn get_gas_prices(&self) -> Result<(u128, u128), SettlementClientError>;

    // Get message hash from event
    fn get_messaging_hash(&self, event: &CommonMessagingEventData) -> Result<Vec<u8>, SettlementClientError>;

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
    async fn get_l1_to_l2_message_cancellations(&self, msg_hash: Vec<u8>) -> Result<Felt, SettlementClientError>;

    // ============================================================
    // Stream Implementations :
    // ============================================================

    /// The type of Stream that will be returned by get_messaging_stream
    /// - Stream: Represents an asynchronous sequence of values
    /// - Item: Each element in the stream is wrapped in Option to handle potential gaps
    /// - Result<T, SettlementClientError>: Each item is further wrapped in Result for error handling
    /// - CommonMessagingEventData: The actual message data structure being streamed
    type StreamType: Stream<Item = Option<Result<CommonMessagingEventData, SettlementClientError>>> + Send;

    /// Retrieves a stream of messaging events starting from the last synced block
    ///
    /// # Arguments
    /// * `last_synced_event_block` - Contains information about the last block that was
    ///    successfully processed, used as starting point for the new stream
    ///
    /// # Returns
    /// * `Result<Self::StreamType, SettlementClientError>` - Returns the stream if successful, or an error
    ///    if stream creation fails
    async fn get_messaging_stream(
        &self,
        last_synced_event_block: LastSyncedEventBlock,
    ) -> Result<Self::StreamType, SettlementClientError>;
}
