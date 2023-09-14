use candid::Principal;
use crate::tools::thirty_bytes_as_principal;

pub const STABLE_MEMORY_SERIALIZE_SIZE: usize = 163;

pub fn index_keys_of_the_log_serialization(b: &[u8]) -> Vec<Principal> {
    vec![ thirty_bytes_as_principal(&b[18..48].try_into().unwrap()) ]
} 

pub fn log_id_of_the_log_serialization(b: &[u8]) -> u128 {
    u128::from_be_bytes(b[2..18].try_into().unwrap())
} 
