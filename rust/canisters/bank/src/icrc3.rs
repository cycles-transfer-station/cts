use serde_bytes::Bytes;
use cts_lib::{
    types::bank::new_log_types::{Log, Operation, MintKind},
    icrc::IcrcId,
};


mod value;
pub use value::{Icrc3Value, Icrc3Map};





pub fn icrc3_value_of_a_block_log<'a>(log: &'a Log) -> Icrc3Value<'a> {
    let mut tx = Icrc3Map::from_iter([
        ("amt", Icrc3Value::Nat(log.tx.amt)),
    ]);
    if let Some(fee) = log.tx.fee {
        tx.insert("fee", Icrc3Value::Nat(fee));
    }
    if let Some(ref memo) = log.tx.memo {
        tx.insert("memo", Icrc3Value::Blob(Bytes::new(&memo[..])));
    }
    if let Some(created_at_time) = log.tx.ts {
        tx.insert("ts", Icrc3Value::Nat(created_at_time.into()));
    }
    match log.tx.op {
        Operation::Mint{ ref to, ref kind } => {
            tx.insert("to", icrc3_value_of_an_icrc_id(&to));
            match kind {
                MintKind::CyclesIn{ ref from_canister } => {
                    tx.insert("kind", Icrc3Value::Text("cycin"));
                    tx.insert("can", Icrc3Value::Blob(Bytes::new(from_canister.as_slice())));
                },
                MintKind::CMC{ ref caller, icp_block_height } => {
                    tx.insert("kind", Icrc3Value::Text("cmc"));
                    tx.insert("callr", Icrc3Value::Blob(Bytes::new(caller.as_slice())));
                    tx.insert("icpb", Icrc3Value::Nat((*icp_block_height).into()));
                }
            }
        }
        Operation::Burn{ ref from, ref for_canister } => {
            tx.insert("from", icrc3_value_of_an_icrc_id(&from));
            tx.insert("can", Icrc3Value::Blob(Bytes::new(for_canister.as_slice())));
        }
        Operation::Xfer{ ref from, ref to } => {
            tx.insert("from", icrc3_value_of_an_icrc_id(&from));
            tx.insert("to", icrc3_value_of_an_icrc_id(&to));            
        }
    }
    
    let mut map = Icrc3Map::from_iter([
        ("btype", Icrc3Value::Text(log.tx.op.icrc3_btype())),
        ("ts", Icrc3Value::Nat(log.ts.into())),
        ("tx", Icrc3Value::Map(tx)),
    ]);
    if let Some(ref phash) = log.phash {
        map.insert("phash", Icrc3Value::Blob(Bytes::new(&phash[..])));
    }
    if let Some(fee) = log.fee {
        map.insert("fee", Icrc3Value::Nat(fee));
    }
    
    Icrc3Value::Map(map)
}


fn icrc3_value_of_an_icrc_id<'a>(icrc_id: &'a IcrcId) -> Icrc3Value<'a> {
    let mut v = vec![Icrc3Value::Blob(Bytes::new(icrc_id.owner.as_slice()))];
    if let Some(ref subaccount) = icrc_id.subaccount {
        if *subaccount != [0u8; 32] {
            v.push(Icrc3Value::Blob(Bytes::new(&subaccount[..])));
        }
    }
    Icrc3Value::Array(v)
}




#[test]
fn test_icrc3_block_hash() {
    use candid::Principal;
    use cts_lib::types::bank::new_log_types::LogTX;
    
    let block = Log{
        phash: Some(serde_bytes::ByteArray::new([0; 32])),
        ts: 123456,
        fee: Some(132454321384),
        tx: LogTX{
            op: Operation::Xfer{
                to: IcrcId{ owner: Principal::from_slice(&[0,1,2,3,4]), subaccount: None },
                from: IcrcId{ owner: Principal::from_slice(&[5,6,7,8,9]), subaccount: None },
            },
            fee: None,
            amt: 26842139832158,
            ts: None,
            memo: Some(serde_bytes::ByteBuf::from(vec![1,2,3]))
        }
    };
    
    let icrc3_value = icrc3_value_of_a_block_log(&block);
    
    println!("{:?}", hex::encode(icrc3_value.hash()));
    
}
