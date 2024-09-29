use serde::Serialize;
use serde_bytes::ByteBuf;


const LABEL_LAST_BLOCK_INDEX: &[u8; 16] = b"last_block_index";
const LABEL_LAST_BLOCK_HASH: &[u8; 15] = b"last_block_hash";

use ic_cdk::api::set_certified_data;
use ic_certified_map::{HashTree, fork_hash, labeled_hash, leaf_hash, fork, labeled};


pub fn set_root_hash(last_block_index: u64, last_block_hash: [u8; 32]) {    
    let last_block_index_leb128 = {
        let mut v = Vec::new();
        let _ = leb128::write::unsigned(&mut v, last_block_index); // ignore result. worse case it doesn't certify the tip.
        v
    };
    let root_hash = fork_hash(
        &labeled_hash(LABEL_LAST_BLOCK_INDEX, &leaf_hash(&last_block_index_leb128[..])),
        &labeled_hash(LABEL_LAST_BLOCK_HASH, &leaf_hash(&last_block_hash[..])),
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
        labeled(LABEL_LAST_BLOCK_INDEX, HashTree::Leaf((&last_block_index_leb128[..]).into())),
        labeled(LABEL_LAST_BLOCK_HASH, HashTree::Leaf((&last_block_hash[..]).into())),
    );
    let mut serializer = serde_cbor::ser::Serializer::new(Vec::new());
    serializer.self_describe().unwrap();
    tree.serialize(&mut serializer).unwrap();
    ByteBuf::from(serializer.into_inner())    
}
