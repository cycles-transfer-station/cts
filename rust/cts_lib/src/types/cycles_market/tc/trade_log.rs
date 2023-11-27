use crate::{
    icrc::Tokens,
    types::{
        cycles_market::{
            tc::{
                CyclesPerToken,
                PositionKind,
            }
        }        
    },
    ic_cdk::trap,
};
use super::{PositionId};




pub const STABLE_MEMORY_SERIALIZE_SIZE: usize = 225; 

pub fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128 {
    u128::from_be_bytes(log_b[18..34].try_into().unwrap())
}

pub fn tokens_quantity_of_the_log_serialization(log_b: &[u8]) -> Tokens {
    u128::from_be_bytes(log_b[94..110].try_into().unwrap())        
}
pub fn rate_of_the_log_serialization(log_b: &[u8]) -> CyclesPerToken {
    u128::from_be_bytes(log_b[126..142].try_into().unwrap())        
}
pub fn timestamp_nanos_of_the_log_serialization(log_b: &[u8]) -> u128 {
    u128::from_be_bytes(log_b[143..159].try_into().unwrap())        
}
pub fn position_kind_of_the_log_serialization(log_b: &[u8]) -> PositionKind {
    match log_b[142] {
        0 => PositionKind::Cycles,
        1 => PositionKind::Token,
        x => trap(&format!("unknown position kind log serialization {x}"))
    }
}


pub fn index_keys_of_the_log_serialization(b: &[u8]) -> Vec<PositionId> {
    vec![ 
        u128::from_be_bytes(b[2..18].try_into().unwrap()),
        u128::from_be_bytes(b[191..207].try_into().unwrap())  
    ]
} 


