use crate::{MAX_LEN_OF_THE_DEDUP_MAP, TX_WINDOW_NANOS, PERMITTED_DRIFT_NANOS};
use cts_lib::{
    tools::time_nanos_u64,
    icrc::{Icrc1TransferError, BlockId},
};
use std::collections::HashMap;
use candid::Principal;



pub type DedupMap = HashMap<
    (Principal/*caller*/, [u8; 32]/*structural-hash*/), 
    (BlockId/*block-index*/, u64/*created-at-time-of-the-request used for pruning*/)
>;


pub fn check_for_dup(dedup_map: &mut DedupMap, caller: Principal, created_at_time: u64, q_structural_hash: [u8; 32]) -> Result<(), Icrc1TransferError> {
    prune_dedup_map(dedup_map);
    let time_nanos_u64: u64 = time_nanos_u64();
    if created_at_time < time_nanos_u64 - TX_WINDOW_NANOS - PERMITTED_DRIFT_NANOS {
        return Err(Icrc1TransferError::TooOld);
    }
    if created_at_time > time_nanos_u64 + PERMITTED_DRIFT_NANOS {
        return Err(Icrc1TransferError::CreatedInFuture{ ledger_time: time_nanos_u64 });
    }
    if let Some((i, _)) = dedup_map.get(&(caller, q_structural_hash)) {
        return Err(Icrc1TransferError::Duplicate{ duplicate_of: (*i).into() });
    }
    if dedup_map.len() >= MAX_LEN_OF_THE_DEDUP_MAP {
        return Err(Icrc1TransferError::TemporarilyUnavailable);
    }
    Ok(())
}


fn prune_dedup_map(dedup_map: &mut DedupMap) {
    let time_nanos_u64 = time_nanos_u64();
    dedup_map.retain(|_, (_, created_at_time)| {
        *created_at_time >= time_nanos_u64 - TX_WINDOW_NANOS - PERMITTED_DRIFT_NANOS
    });
}