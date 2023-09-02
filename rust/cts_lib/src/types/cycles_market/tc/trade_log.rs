use crate::{
    icrc::Tokens,
    types::{
        cycles_market::{
            tc::{
                CyclesPerToken
            }
        }        
    }
};
use super::PositionId;




pub const STABLE_MEMORY_SERIALIZE_SIZE: usize = 157; 

pub fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128 {
    u128::from_be_bytes(log_b[16..32].try_into().unwrap())
}

pub fn tokens_quantity_of_the_log_serialization(log_b: &[u8]) -> Tokens {
    u128::from_be_bytes(log_b[92..108].try_into().unwrap())        
}
pub fn rate_of_the_log_serialization(log_b: &[u8]) -> CyclesPerToken {
    u128::from_be_bytes(log_b[124..140].try_into().unwrap())        
}
pub fn timestamp_nanos_of_the_log_serialization(log_b: &[u8]) -> u128 {
    u128::from_be_bytes(log_b[141..157].try_into().unwrap())        
}


pub fn index_key_of_the_log_serialization(b: &[u8]) -> PositionId {
    u128::from_be_bytes(b[0..16].try_into().unwrap())
} 


