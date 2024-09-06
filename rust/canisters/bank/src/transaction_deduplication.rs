use crate::{MAX_LEN_OF_A_DEDUP_MAP};
use cts_lib::tools::time_nanos_u64;



pub type DedupMap = HashMap<
    (Principal/*caller*/, [u8; 32]/*structural-hash*/), 
    (u128/*block-index*/, u64/*created-at-time-of-the-request used for pruning*/)
>;


pub fn check_for_dup(dedup_map: &mut DedupMap, caller: Principal, q: &Icrc1TransferQuest) -> Result<(), Icrc1TransferError> {
    if let Some(created_at_time) = q.created_at_time() {
        prune_dedup_map(dedup_map);
        let time_nanos_u64 = time_nanos_u64();
        if created_at_time < time_nanos_u64 - TX_WINDOW_NANOS - PERMITTED_DRIFT_NANOS {
            return Err(Icrc1TransferError::TooOld);
        }
        if created_at_time > time_nanos_u64 + PERMITTED_DRIFT_NANOS {
            return Err(Icrc1TransferError::CreatedInFuture{ ledger_time: time_nanos_u64 });
        }
        if let Some((i, _)) = dedup_map.get(&(caller, icrc1_transfer_quest_structural_hash(q))) {
            return Err(Icrc1TransferError::Duplicate{ duplicate_of: i });
        }
        if dedup_map.len() >= MAX_LEN_OF_A_DEDUP_MAP {
            return Err(Icrc1TransferError::TemporarilyUnavailable);
        }
    }
    Ok(())
}


fn prune_dedup_map(dedup_map: &mut DedupMap) {
    dedup_map.retain(|(_, (_, created_at_time))| {
        created_at_time >= time_nanos_u64 - TX_WINDOW_NANOS - PERMITTED_DRIFT_NANOS
    });
}

pub fn icrc1_transfer_quest_structural_hash(q: &Icrc1TransferQuest) -> [u8; 32] {
    todo!()
}
