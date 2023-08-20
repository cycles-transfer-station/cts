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
        export::Principal,
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    },
    stable_memory_tools::{
        self,
    },
    types::cycles_market::icrc1token_trade_contract::{PositionId, icrc1token_trade_log_storage::*},
};

use ic_stable_structures::{
    memory_manager::MemoryId,
};

use cm_storage_lib::{
    self,
    StorageData,
    OldStorageData,
    STORAGE_DATA,
    STORAGE_DATA_MEMORY_ID
    
};



type UserPositionsKey = Principal;
type UserPositionsVecValue = PositionId;
type UserPositions = HashMap<UserPositionsKey, Vec<UserPositionsVecValue>>;


fn index_key_of_the_log_serialization(b: &[u8]) -> UserPositionsKey {
    Principal::from_slice(&b[17..(17 + b[16] as usize)])
} 

fn log_id_of_the_log_serialization(b: &[u8]) -> u128 {
    u128::from_be_bytes(b[0..16].try_into().unwrap())
} 



const USER_POSITIONS_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local!{
    static USER_POSITIONS: RefCell<UserPositions> = RefCell::new(UserPositions::new());
}





#[init]
fn init(q: LogStorageInit) {
    stable_memory_tools::init(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID);
    stable_memory_tools::init(&USER_POSITIONS, USER_POSITIONS_MEMORY_ID);
        
    with_mut(&STORAGE_DATA, |data| {
        data.log_size = q.log_size;
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::post_upgrade(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID, None::<fn(OldStorageData) -> StorageData>);
    stable_memory_tools::post_upgrade(&USER_POSITIONS, USER_POSITIONS_MEMORY_ID, None::<fn(UserPositions) -> UserPositions>);
}


#[update]
pub fn flush(q: FlushQuest) -> Result<FlushSuccess, FlushError> {
    with_mut(&USER_POSITIONS, |user_positions| {
        cm_storage_lib::flush(
            q, 
            user_positions,
            log_id_of_the_log_serialization,
            index_key_of_the_log_serialization,
        )    
    })
}

#[query(manual_reply = true)]
pub fn map_logs_rchunks(k: Principal, opt_start_before_id: Option<u128>, chunk_size: u32) {
    with(&USER_POSITIONS, |user_positions| {
        cm_storage_lib::map_logs_rchunks(&k, opt_start_before_id, chunk_size, user_positions);
    });
}


