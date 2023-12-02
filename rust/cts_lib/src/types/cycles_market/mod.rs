pub mod cm_main;
pub mod tc;


use candid::{CandidType, Deserialize};


#[derive(CandidType, Deserialize)]
pub enum LogStorageType {
    Trades,
    Positions,
}


#[allow(non_upper_case_globals)]
pub const TC_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB: usize = 500; // multiple of 10
    


#[derive(CandidType, Deserialize, Clone)]
pub struct ViewStorageLogsQuest<LogIndexKey> {
    pub opt_start_before_id: Option<u128>,
    pub index_key: Option<LogIndexKey>
}