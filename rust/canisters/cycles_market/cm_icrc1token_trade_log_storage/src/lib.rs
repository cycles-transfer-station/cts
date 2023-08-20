use std::cell::RefCell;
use std::collections::HashMap;
use cts_lib::{
    tools::{
        localkey::{
            refcell::{with, with_mut}
        },
    },
    ic_cdk::{
        self,
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    },
    stable_memory_tools::{
        self,
        MemoryId,
    },
    types::cycles_market::icrc1token_trade_contract::{PositionId, PurchaseId, icrc1token_trade_log_storage::*},
};


use cm_storage_lib::{
    self,
    StorageData,
    OldStorageData,
    STORAGE_DATA,
    STORAGE_DATA_MEMORY_ID    
};


// ----------------------------------





type PositionsPurchasesKey = PositionId;
type PositionsPurchasesVecValue = PurchaseId;
type PositionsPurchases = HashMap<PositionsPurchasesKey, Vec<PositionsPurchasesVecValue>>;


fn index_key_of_the_log_serialization(b: &[u8]) -> PositionsPurchasesKey {
    u128::from_be_bytes(b[0..16].try_into().unwrap())
} 

fn log_id_of_the_log_serialization(b: &[u8]) -> u128 {
    u128::from_be_bytes(b[16..32].try_into().unwrap())
} 



const POSITIONS_PURCHASES_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local!{
    static POSITIONS_PURCHASES: RefCell<PositionsPurchases> = RefCell::new(PositionsPurchases::new());
}





#[init]
fn init(q: LogStorageInit) {
    stable_memory_tools::init(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID);
    stable_memory_tools::init(&POSITIONS_PURCHASES, POSITIONS_PURCHASES_MEMORY_ID);
        
    with_mut(&STORAGE_DATA, |data| {
        data.set_log_size(q.log_size);
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::post_upgrade(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID, None::<fn(OldStorageData) -> StorageData>);
    stable_memory_tools::post_upgrade(&POSITIONS_PURCHASES, POSITIONS_PURCHASES_MEMORY_ID, None::<fn(PositionsPurchases) -> PositionsPurchases>);
}


#[update]
pub fn flush(q: FlushQuest) -> Result<FlushSuccess, FlushError> {
    with_mut(&POSITIONS_PURCHASES, |positions_purchases| {
        cm_storage_lib::flush(
            q, 
            positions_purchases,
            log_id_of_the_log_serialization,
            index_key_of_the_log_serialization,
        )    
    })
}

#[query(manual_reply = true)]
pub fn map_logs_rchunks(k: PositionsPurchasesKey, opt_start_before_id: Option<u128>, chunk_size: u32) {
    with(&POSITIONS_PURCHASES, |positions_purchases| {
        cm_storage_lib::map_logs_rchunks(&k, opt_start_before_id, chunk_size, positions_purchases);
    });
}

















