use std::cell::RefCell;
use std::collections::HashMap;
use cts_lib::{
    tools::{
        localkey::{
            refcell::{with, with_mut}
        },
    },
    types::cm::tc::{PositionId, storage_logs::{LogStorageInit, StorageLogTrait, position_log::PositionLog}},
};
use ic_cdk::{
    update,
    query,
    init,
    pre_upgrade,
    post_upgrade
};
use canister_tools::{self, MemoryId};
use candid::Principal;
use cm_storage_lib::{
    self,
    StorageData,
    OldStorageData,
    STORAGE_DATA,
    STORAGE_DATA_MEMORY_ID,
    FlushQuest,
    FlushSuccess,
    FlushError,    
};



type UserPositionsKey = Principal;
type UserPositionsVecValue = PositionId;
type UserPositions = HashMap<UserPositionsKey, Vec<UserPositionsVecValue>>;




const USER_POSITIONS_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local!{
    static USER_POSITIONS: RefCell<UserPositions> = RefCell::new(UserPositions::new());
}





#[init]
fn init(q: LogStorageInit) {
    canister_tools::init(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID);
    canister_tools::init(&USER_POSITIONS, USER_POSITIONS_MEMORY_ID);
        
    with_mut(&STORAGE_DATA, |data| {
        data.set_log_size(q.log_size);
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    canister_tools::post_upgrade(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID, None::<fn(OldStorageData) -> StorageData>);
    canister_tools::post_upgrade(&USER_POSITIONS, USER_POSITIONS_MEMORY_ID, None::<fn(UserPositions) -> UserPositions>);
}


#[update]
pub fn flush(q: FlushQuest) -> Result<FlushSuccess, FlushError> {
    with_mut(&USER_POSITIONS, |user_positions| {
        cm_storage_lib::flush(
            q, 
            user_positions,
            PositionLog::log_id_of_the_log_serialization,
            PositionLog::index_keys_of_the_log_serialization,
        )    
    })
}

#[query(manual_reply = true)]
pub fn map_logs_rchunks(k: UserPositionsKey, opt_start_before_id: Option<u128>, chunk_size: u32) {
    with(&USER_POSITIONS, |user_positions| {
        cm_storage_lib::map_logs_rchunks(&k, opt_start_before_id, chunk_size, user_positions);
    });
}







ic_cdk::export_candid!();