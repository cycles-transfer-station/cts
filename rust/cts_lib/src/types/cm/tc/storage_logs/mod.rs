use super::*;
use crate::consts::MiB;

pub mod trade_log; 
pub mod position_log;


#[cfg(not(debug_assertions))]
pub const FLUSH_STORAGE_BUFFER_AT_SIZE: usize = 5 * MiB;

#[cfg(debug_assertions)]
pub const FLUSH_STORAGE_BUFFER_AT_SIZE: usize = 1 * KiB;

pub const MAX_STORAGE_BUFFER_SIZE: usize = FLUSH_STORAGE_BUFFER_AT_SIZE + 1*MiB;

#[cfg(not(debug_assertions))]
pub const FLUSH_STORAGE_BUFFER_CHUNK_SIZE_BEFORE_MODULO: usize = 1*MiB+512*KiB; 

#[cfg(debug_assertions)]
pub const FLUSH_STORAGE_BUFFER_CHUNK_SIZE_BEFORE_MODULO: usize = 512; 



pub trait StorageLogTrait {
    const STABLE_MEMORY_SERIALIZE_SIZE: usize;
    const STABLE_MEMORY_VERSION: u16;
    fn stable_memory_serialize(&self) -> Vec<u8>;// Self::STABLE_MEMORY_SERIALIZE_SIZE]; const generics not stable yet
    fn stable_memory_serialize_backwards(log_b: &[u8]) -> Self;
    fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128;
    type LogIndexKey: CandidType + for<'a> Deserialize<'a> + PartialEq + Eq;
    fn index_keys_of_the_log_serialization(log_b: &[u8]) -> Vec<Self::LogIndexKey>;
}

#[derive(CandidType, Deserialize, Clone)]
pub struct ViewStorageLogsQuest<LogIndexKey> {
    pub opt_start_before_id: Option<u128>,
    pub index_key: Option<LogIndexKey>
}

#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct LogStorageInit {
    pub log_size: u32,
}

