use crate::NewLogs;
use ic_cdk::api::set_certified_data;
use ic_certified_map::{HashTree, fork_hash, labeled_hash, leaf_hash, fork, labeled};
use serde::Serialize;
use serde_bytes::ByteBuf;
use cts_lib::types::bank::icrc3::icrc3_value_of_a_block_log;


const LABEL_LAST_BLOCK_INDEX: &[u8; 16] = b"last_block_index";
const LABEL_LAST_BLOCK_HASH: &[u8; 15] = b"last_block_hash";


pub fn set_root_hash(logs: &NewLogs) {    
    if logs.len() != 0 {
        let last_block_index = logs.len() - 1;
        let last_block_hash = icrc3_value_of_a_block_log(&logs.get(last_block_index).unwrap()).hash(); // unwrap ok bc we check the length of the logs first.
        let last_block_index_leb128 = {
            let mut v = Vec::new();
            let _ = leb128::write::unsigned(&mut v, last_block_index); // ignore result for now, since this function is called in the callback after the cycles-out deposit-cycles call, we don't want to error here. Either way there's no common reason for it to fail. 
            v
        };
        let root_hash = fork_hash(
            &labeled_hash(LABEL_LAST_BLOCK_INDEX, &leaf_hash(&last_block_index_leb128[..])),
            &labeled_hash(LABEL_LAST_BLOCK_HASH, &leaf_hash(&last_block_hash[..])),
        );
        set_certified_data(&root_hash[..]);
    }
}

pub fn make_data_certificate_hash_tree(logs: &NewLogs) -> Option<ByteBuf> { // cbor hashtree. none if logs.len == 0 
    if logs.len() == 0 {
        return None;
    }
    let last_block_index = logs.len() - 1;
    let last_block_hash = icrc3_value_of_a_block_log(&logs.get(last_block_index).unwrap()).hash();
    let last_block_index_leb128 = {
        let mut v = Vec::new();
        leb128::write::unsigned(&mut v, last_block_index).unwrap(); // unwrap ok bc this is only called in icrc3_get_tip_certificate which is a query call.
        v
    };
    let tree: HashTree = fork(
        labeled(LABEL_LAST_BLOCK_INDEX, HashTree::Leaf((&last_block_index_leb128[..]).into())),
        labeled(LABEL_LAST_BLOCK_HASH, HashTree::Leaf((&last_block_hash[..]).into())),
    );
    let mut serializer = serde_cbor::ser::Serializer::new(Vec::new());
    serializer.self_describe().unwrap();
    tree.serialize(&mut serializer).unwrap();
    Some(ByteBuf::from(serializer.into_inner()))    
}
