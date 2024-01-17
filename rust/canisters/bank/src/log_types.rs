use std::borrow::Cow;
use serde::Serialize;
use candid::{CandidType, Deserialize};
use cts_lib::types::Cycles;
use serde_bytes::ByteBuf;
use crate::CountId;
use ic_stable_structures::{Storable, storable::Bound};


#[derive(CandidType, Serialize, Deserialize)]
pub struct Log {
    pub ts: u64,
    pub fee: Option<Cycles>, // if the user does not specify the fee in the request
    pub tx: LogTX,
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct LogTX {
    pub op: Operation,
    pub fee: Option<Cycles>, // if the user specifies the fee in the request
    pub amt: Cycles,
    pub memo: Option<ByteBuf>,
    pub ts: Option<u64>, // if the user specifies the created_at_time field in the request.
}

#[derive(CandidType, Serialize, Deserialize)]
pub enum Operation {
    Burn{ from: CountId },
    Mint{ to: CountId },
    Xfer{ from: CountId, to: CountId } 
}
    
impl Storable for Log {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(rmp_serde::to_vec(self).unwrap())
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        rmp_serde::from_slice(&bytes).unwrap()
    }
    const BOUND: Bound = {
        Bound::Bounded{
            max_size: 270,
            is_fixed_size: false
        }
    };
}
    