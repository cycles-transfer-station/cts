pub mod cm_main;
pub mod tc;
pub mod icrc45;
pub mod get_position;

use candid::{CandidType, Deserialize};


#[derive(CandidType, Deserialize)]
pub enum LogStorageType {
    Trades,
    Positions,
}

#[allow(non_upper_case_globals)]
pub const TC_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB: usize = 500; // multiple of 10
    

