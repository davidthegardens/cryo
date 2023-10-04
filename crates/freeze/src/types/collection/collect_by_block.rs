use super::collect_generic::fetch_partition;
use crate::{
    ChunkDim, CollectError, ToDataFrames, Datatype, Params, Partition, Schemas, Source, Table,
};
use polars::prelude::*;
use std::collections::HashMap;
use tokio::sync::mpsc;

type Result<T> = ::core::result::Result<T, CollectError>;

/// defines how to collect dataset by block
#[async_trait::async_trait]
pub trait CollectByBlock: 'static + Send + Default + ToDataFrames {
    /// type of block data responses
    type Response: Send;

    /// parameters for requesting data by block
    fn block_parameters() -> Vec<ChunkDim> {
        vec![ChunkDim::BlockNumber]
    }

    /// fetch dataset data by block
    async fn extract(request: Params, source: Source, schemas: Schemas) -> Result<Self::Response>;

    /// transform block data response into column data
    fn transform(response: Self::Response, columns: &mut Self, schemas: &Schemas);

    /// collect data into DataFrame
    async fn collect_by_block(
        partition: Partition,
        source: Source,
        schemas: &HashMap<Datatype, Table>,
    ) -> Result<HashMap<Datatype, DataFrame>> {
        let (sender, receiver) = mpsc::channel(1);
        let chain_id = source.chain_id;
        fetch_partition(
            Self::extract,
            partition,
            source,
            schemas.clone(),
            Self::block_parameters(),
            sender,
        )
        .await?;
        Self::block_data_to_dfs(receiver, schemas, chain_id).await
    }

    /// convert block-derived data to dataframe
    async fn block_data_to_dfs(
        mut receiver: mpsc::Receiver<Result<Self::Response>>,
        schemas: &HashMap<Datatype, Table>,
        chain_id: u64,
    ) -> Result<HashMap<Datatype, DataFrame>> {
        let mut columns = Self::default();
        while let Some(message) = receiver.recv().await {
            match message {
                Ok(message) => Self::transform(message, &mut columns, schemas),
                Err(e) => return Err(e),
            }
        }
        columns.create_dfs(schemas, chain_id)
    }
}