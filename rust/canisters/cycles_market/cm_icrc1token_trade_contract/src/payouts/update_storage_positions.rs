use crate::types::{PositionLog};
use cts_lib::types::cycles_market::icrc1token_trade_contract::{PositionId};



pub type DoUpdateStoragePositionResult = bool;

pub async fn do_update_storage_position(position_id: PositionId, log_serialization_b: [u8; PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]) -> DoUpdateStoragePositionResult {
    todo!();
    
    
}


