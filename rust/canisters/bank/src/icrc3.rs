use serde::Serialize;
use candid::{CandidType, Principal, Deserialize};
use serde_bytes::{ByteBuf, Bytes};
use cts_lib::{
    types::bank::new_log_types::{Log, Operation, MintKind},
    icrc::{IcrcId, icrc3::{Icrc3Value, Icrc3Map}},
};



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


// certification

const LABEL_LATEST_BLOCK_INDEX: &[u8; 16] = b"last_block_index";
const LABEL_LATEST_BLOCK_HASH: &[u8; 15] = b"last_block_hash";

use ic_cdk::api::set_certified_data;
use ic_certified_map::{HashTree, fork_hash, labeled_hash, leaf_hash, fork, labeled};


pub fn set_root_hash(last_block_index: u64, last_block_hash: [u8; 32]) {    
    let last_block_index_leb128 = {
        let mut v = Vec::new();
        let _ = leb128::write::unsigned(&mut v, last_block_index); // ignore result. worse case it doesn't certify the tip.
        v
    };
    let root_hash = fork_hash(
        &labeled_hash(LABEL_LATEST_BLOCK_INDEX, &leaf_hash(&last_block_index_leb128[..])),
        &labeled_hash(LABEL_LATEST_BLOCK_HASH, &leaf_hash(&last_block_hash[..])),
    );
    set_certified_data(&root_hash[..]);
}

pub fn make_data_certificate_hash_tree(last_block_index: u64, last_block_hash: [u8; 32]) -> ByteBuf { // cbor hashtree
    let last_block_index_leb128 = {
        let mut v = Vec::new();
        let _ = leb128::write::unsigned(&mut v, last_block_index); // ignore result. worse case it doesn't certify the tip.
        v
    };
    let tree: HashTree = fork(
        labeled(LABEL_LATEST_BLOCK_INDEX, HashTree::Leaf((&last_block_index_leb128[..]).into())),
        labeled(LABEL_LATEST_BLOCK_HASH, HashTree::Leaf((&last_block_hash[..]).into())),
    );
    let mut serializer = serde_cbor::ser::Serializer::new(Vec::new());
    serializer.self_describe().unwrap();
    tree.serialize(&mut serializer).unwrap();
    ByteBuf::from(serializer.into_inner())    
}


// --- TYPES ---

#[derive(CandidType, Deserialize, Copy, Clone)]
pub struct StartAndLength{
    pub start: u128,
    pub length: u128,
}

#[derive(CandidType)]
pub struct IdAndBlock<'a>{
    pub id: u128,
    pub block: Icrc3Value<'a>,
}

#[derive(CandidType)]
pub struct GetBlocksArgsAndCallback {
    pub args : GetBlocksArgs,
    pub callback : Icrc3Callback,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "candid::types::reference::Func")]
pub struct Icrc3Callback {
    pub canister_id: Principal,
    pub method: String,
}
impl PartialOrd for Icrc3Callback {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Icrc3Callback {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.canister_id.cmp(&other.canister_id) {
            std::cmp::Ordering::Equal => self.method.cmp(&other.method),
            c => c,
        }
    }
}
impl Icrc3Callback {
    pub fn new(canister_id: Principal, method: impl Into<String>) -> Self {
        Self {
            canister_id,
            method: method.into(),
        }
    }
}
impl Clone for Icrc3Callback {
    fn clone(&self) -> Self {
        Self {
            canister_id: self.canister_id,
            method: self.method.clone(),
        }
    }
}
impl From<Icrc3Callback> for candid::types::reference::Func {
    fn from(archive_fn: Icrc3Callback) -> Self {
        let p: &Principal = &Principal::try_from(archive_fn.canister_id.as_ref())
            .expect("could not deserialize principal");
        Self {
            principal: *p,
            method: archive_fn.method,
        }
    }
}
impl TryFrom<candid::types::reference::Func> for Icrc3Callback {
    type Error = String;
    fn try_from(func: candid::types::reference::Func) -> Result<Self, Self::Error> {
        let canister_id = Principal::try_from(func.principal.as_slice())
            .map_err(|e| format!("principal is not a canister id: {}", e))?;
        Ok(Self {
            canister_id,
            method: func.method,
        })
    }
}
impl CandidType for Icrc3Callback {
    fn _ty() -> candid::types::Type {
        candid::func!((GetBlocksArgs) -> (GetBlocksResult) query)
    }
    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        candid::types::reference::Func::from(self.clone()).idl_serialize(serializer)
    }
}

pub type GetBlocksArgs = Vec<StartAndLength>;

#[derive(CandidType)]
pub struct GetBlocksResult<'a> {
    // Total number of blocks in the block log
    pub log_length : u128,

    // Blocks found locally to the Ledger
    pub blocks : Vec<IdAndBlock<'a>>,

    // List of callbacks to fetch the blocks that are not local
    // to the Ledger, i.e. archived blocks
    pub archived_blocks : Vec<GetBlocksArgsAndCallback>,
}
impl GetBlocksResult<'static> {
    pub fn placeholder() -> Self {
        Self{
            log_length: 0,
            blocks: vec![],
            archived_blocks: vec![],
        }
    }
}

#[derive(CandidType, Deserialize)]
pub struct SupportBlockType {
    pub block_type: &'static str,
    pub url: &'static str,
}

#[derive(CandidType, Deserialize)]
pub struct GetArchivesArgs {
    // The last archive seen by the client.
    // The Ledger will return archives coming
    // after this one if set, otherwise it
    // will return the first archives.
    pub from : Option<Principal>,
}

#[derive(CandidType, Deserialize)]
pub struct ArchiveData{
    // The id of the archive
    pub canister_id : Principal,

    // The first block in the archive
    pub start : u128,

    // The last block in the archive
    pub end : u128,
}

pub type GetArchivesResult = Vec<ArchiveData>;

#[derive(CandidType, Deserialize)]
pub struct Icrc3DataCertificate {
    // Signature of the root of the hash_tree
    pub certificate : ByteBuf,
    // CBOR encoded hash_tree
    pub hash_tree : ByteBuf,
}
