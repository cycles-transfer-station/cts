pub mod cm_main;
pub mod tc;


use candid::{CandidType, Deserialize};


#[derive(CandidType, Deserialize)]
pub enum LogStorageType {
    Trades,
    Positions,
}



