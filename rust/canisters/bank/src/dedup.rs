use crate::{MAX_LEN_OF_THE_DEDUP_MAP, TX_WINDOW_NANOS, PERMITTED_DRIFT_NANOS};
use cts_lib::{
    tools::{time_nanos_u64, structural_hash},
    icrc::{Icrc1TransferQuest, Icrc1TransferError, BlockId},
};
use std::collections::HashMap;
use candid::Principal;



pub type DedupMap = HashMap<
    (Principal/*caller*/, [u8; 32]/*structural-hash*/), 
    (BlockId/*block-index*/, u64/*created-at-time-of-the-request used for pruning*/)
>;


pub fn check_for_dup(dedup_map: &mut DedupMap, caller: Principal, q: &Icrc1TransferQuest) -> Result<(), Icrc1TransferError> {
    if let Some(created_at_time) = q.created_at_time {
        prune_dedup_map(dedup_map);
        let time_nanos_u64: u64 = time_nanos_u64();
        if created_at_time < time_nanos_u64 - TX_WINDOW_NANOS - PERMITTED_DRIFT_NANOS {
            return Err(Icrc1TransferError::TooOld);
        }
        if created_at_time > time_nanos_u64 + PERMITTED_DRIFT_NANOS {
            return Err(Icrc1TransferError::CreatedInFuture{ ledger_time: time_nanos_u64 });
        }
        if let Some((i, _)) = dedup_map.get(&(caller, icrc1_transfer_quest_structural_hash(q))) {
            return Err(Icrc1TransferError::Duplicate{ duplicate_of: (*i).into() });
        }
        if dedup_map.len() >= MAX_LEN_OF_THE_DEDUP_MAP {
            return Err(Icrc1TransferError::TemporarilyUnavailable);
        }
    }
    Ok(())
}


fn prune_dedup_map(dedup_map: &mut DedupMap) {
    let time_nanos_u64 = time_nanos_u64();
    dedup_map.retain(|_, (_, created_at_time)| {
        *created_at_time >= time_nanos_u64 - TX_WINDOW_NANOS - PERMITTED_DRIFT_NANOS
    });
}

pub fn icrc1_transfer_quest_structural_hash(q: &Icrc1TransferQuest) -> [u8; 32] {
    use serde_bytes::Bytes;
    
    #[derive(serde::Serialize)]
    struct IcrcIdForTheStructureHash<'a> {
        owner: &'a Bytes,
        subaccount: Option<&'a Bytes>
    }
    #[derive(serde::Serialize)]
    struct Icrc1TransferQuestForTheStructureHash<'a> {
        to: IcrcIdForTheStructureHash<'a>,
        fee: Option<u128>,
        memo: Option<&'a Bytes>,
        from_subaccount: Option<&'a Bytes>,
        created_at_time: Option<u64>,
        amount: u128,
    }
    
    structural_hash(
        &Icrc1TransferQuestForTheStructureHash {
            to: IcrcIdForTheStructureHash{
                owner: Bytes::new(q.to.owner.as_slice()),
                subaccount: q.to.subaccount.as_ref().map(|b| Bytes::new(&b[..])),
            },
            fee: q.fee,
            memo: q.memo.as_ref().map(|b| Bytes::new(&b[..])),
            from_subaccount: q.from_subaccount.as_ref().map(|b| Bytes::new(&b[..])),
            created_at_time: q.created_at_time,
            amount: q.amount,
        }
    ).unwrap() // unwrap ok bc this function is only used within icrc1_transfer which is synchronous and is within a single message execution
}
