use candid::Principal;


pub const STABLE_MEMORY_SERIALIZE_SIZE: usize = 5;

pub fn index_keys_of_the_log_serialization(b: &[u8]) -> Vec<Principal> {
    vec![ Principal::from_slice(&b[17..(17 + b[16] as usize)]) ]
} 

pub fn log_id_of_the_log_serialization(b: &[u8]) -> u128 {
    u128::from_be_bytes(b[0..16].try_into().unwrap())
} 
