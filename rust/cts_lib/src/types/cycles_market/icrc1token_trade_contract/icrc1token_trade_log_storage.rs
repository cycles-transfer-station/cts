use crate::ic_cdk::export::candid::{CandidType, Deserialize};
use serde_bytes::Bytes;

#[derive(CandidType, Deserialize)]
pub struct Icrc1TokenTradeLogStorageInit {
    pub log_size: u32,
}

#[derive(CandidType)]
pub struct FlushQuestForward<'a> {
    pub bytes: &'a Bytes
}

#[derive(CandidType, Deserialize)]
pub struct FlushQuest {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>
}

#[derive(CandidType, Deserialize)]
pub struct FlushSuccess {}

#[derive(CandidType, Deserialize)]
pub enum FlushError {
    StorageIsFull,
}

#[derive(CandidType, Deserialize)]
pub struct SeeTradeLogsQuest {
    pub start_id: u128,
    pub length: u128,
}

#[derive(CandidType, Deserialize)]
pub struct StorageLogs {
    #[serde(with = "serde_bytes")]
    pub logs: Vec<u8>
}

