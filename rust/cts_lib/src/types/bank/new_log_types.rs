use std::borrow::Cow;
use serde::{Serialize, Deserialize};
use candid::{CandidType, Principal};
use crate::types::Cycles;
use serde_bytes::{ByteBuf, ByteArray};
use crate::icrc::IcrcId;
use ic_stable_structures::{Storable, storable::Bound};

// POSTCARD-SERIALIZATION
// Warning! postcard correctness counts on the specific sequence of the fields in the structs from top to bottom staying the same for both forwards and backwards.

// CandidType is temp while we still using the get_logs_backwards method.


#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Log {
    pub phash: Option<ByteArray<32>>, // none if first block // check that existing blocks will be able to deserialize a new optional field. // or maybe in the post-upgrade, change these values to a new serialization format with phash, and serde_bytes for the subaccounts.
    pub ts: u64,
    pub fee: Option<Cycles>, // if the user does not specify the fee in the request
    pub tx: LogTX,
}

#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct LogTX {
    pub op: Operation,
    pub fee: Option<Cycles>, // if the user specifies the fee in the request
    pub amt: Cycles,
    pub memo: Option<ByteBuf>,
    pub ts: Option<u64>, // if the user specifies the created_at_time field in the request.
}

#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Operation {
    Mint{ to: IcrcId, kind: MintKind },
    Burn{ from: IcrcId, for_canister: Principal },
    Xfer{ from: IcrcId, to: IcrcId } 
}

impl Operation {
    pub fn icrc3_btype(&self) -> &'static str {
        match self {
            Self::Mint{ .. } => "1mint",
            Self::Burn{ .. } => "1burn", 
            Self::Xfer{ .. } => "1xfer",
        }
    }
}

#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum MintKind {
    CyclesIn{ from_canister: Principal },
    CMC{ caller: Principal, icp_block_height: u64 }    
}
    

const LOG_STORABLE_MAX_SIZE: u32 = 309; // 277 is max then plus 32 for good measure.

impl Storable for Log {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(postcard::to_stdvec(self).unwrap())
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        postcard::from_bytes(&bytes).unwrap()
    }
    const BOUND: Bound = {
        Bound::Bounded{
            max_size: LOG_STORABLE_MAX_SIZE,
            is_fixed_size: false
        }
    };
}





#[test]
fn test_bank_log_serialization_size() {
    let full_size_principal = Principal::from_slice(&[u8::MAX; 29][..]);
    let full_size_icrc_id = IcrcId{ owner: full_size_principal, subaccount: Some(ByteArray::new([u8::MAX; 32])) };
    
    let mut log = Log{
        phash: Some(ByteArray::new([u8::MAX; 32])),
        ts: u64::MAX,
        fee: Some(u128::MAX),
        tx: LogTX{
            op: Operation::Xfer{
                to: full_size_icrc_id,
                from: full_size_icrc_id,
            },
            fee: Some(u128::MAX),
            amt: u128::MAX,
            ts: Some(u64::MAX),
            memo: Some(ByteBuf::from(vec![u8::MAX; 32]))
        }
    };
    let ser = log.to_bytes();
    println!("{}", ser.len());
    assert!(ser.len() <= LOG_STORABLE_MAX_SIZE as usize);
    
    log.tx.op = Operation::Mint{
        to: full_size_icrc_id,
        kind: MintKind::CyclesIn{ from_canister: full_size_principal },
    };
    let ser = log.to_bytes();
    println!("{}", ser.len());
    assert!(ser.len() <= LOG_STORABLE_MAX_SIZE as usize);

    log.tx.op = Operation::Mint{
        to: full_size_icrc_id,
        kind: MintKind::CMC{ caller: full_size_principal, icp_block_height: u64::MAX },
    };
    let ser = log.to_bytes();
    println!("{}", ser.len());
    assert!(ser.len() <= LOG_STORABLE_MAX_SIZE as usize);
    
    log.tx.op = Operation::Burn{
        from: full_size_icrc_id,
        for_canister: full_size_principal,
    };
    let ser = log.to_bytes();
    println!("{}", ser.len());
    assert!(ser.len() <= LOG_STORABLE_MAX_SIZE as usize);
    
}
