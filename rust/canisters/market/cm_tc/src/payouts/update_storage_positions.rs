// do not borrow or borrow_mut the CM_DATA.


use crate::{
    POSITIONS_STORAGE_DATA,
};
use cts_lib::{
    types::{
        CallError,
        cm::tc::{PositionId},
    },
    tools::{
        localkey::refcell::{with,with_mut},
        call_error_as_u32_and_string,
    },
};
use ic_cdk::api::call::call;
use candid::Principal;



pub type DoUpdateStoragePositionResult = Result<(), CallError>;

pub async fn do_update_storage_position(position_id: PositionId, log_serialization_b: Vec<u8> /* const generics[u8; PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]*/) -> DoUpdateStoragePositionResult {
    // find if position is on a storage-canister or still in the flush_buffer.
    let mut call_storage_canister_id: Option<Principal> = None;
    
    with(&POSITIONS_STORAGE_DATA, |positions_storage_data| {
        for storage_canister in positions_storage_data.storage_canisters.iter().rev() {
            if position_id < storage_canister.first_log_id + storage_canister.length as u128 
            && position_id >= storage_canister.first_log_id {
                // it is on the storage canister
                call_storage_canister_id = Some(storage_canister.canister_id);
            } 
        }
    });
    
    match call_storage_canister_id {
        None => {
            with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| {
                let storage_buffer_first_log_id: PositionId = {
                    match positions_storage_data.storage_canisters.last() {
                        None => 0,
                        Some(ref sc) => sc.first_log_id + sc.length as u128
                    }
                };
                let start_i: usize = (position_id - storage_buffer_first_log_id) as usize * log_serialization_b.len();
                positions_storage_data.storage_buffer[start_i..start_i + log_serialization_b.len()].copy_from_slice(&log_serialization_b); 
            });
            Ok(())
        },
        Some(call_storage_canister_id) => {
            // call storage canister
            match call(
                call_storage_canister_id,
                "cm_update_log",
                (position_id, log_serialization_b),
            ).await {
                Ok(()) => Ok(()),
                Err(call_error) => Err(call_error_as_u32_and_string(call_error)),
            }
        }
    }
    
}


