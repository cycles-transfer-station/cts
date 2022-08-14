use std::collections::HashMap;
use std::borrow::{BorrowMut, Borrow};

use cts_lib::{
    ic_cdk_macros::{update, query},
    ic_cdk::{
        export::candid::{CandidType, Deserialize, Func},
        api::{data_certificate, set_certified_data}
    },
    ic_certified_map::{self, RbTree, HashTree, AsHashTree},
    tools::{
        sha256,
        localkey::refcell::{with, with_mut},
    },
};

use serde::Serialize;

use crate::FRONTCODE_FILES_HASHES;


const LABEL_ASSETS: &[u8; 11] = b"http_assets";

#[derive(CandidType, Deserialize, Clone)]
pub struct File {
    pub content_type: String,
    pub content_encoding: String,
    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>
}
pub type Files = HashMap<String, File>;
pub type FilesHashes = RbTree<String, ic_certified_map::Hash>;



#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Token {}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum StreamingStrategy {
    Callback { callback: Func, token: Token},
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackHttpResponse {
    pub body: Vec<u8>,
    pub token: Option<Token>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    #[serde(with = "serde_bytes")]
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, CandidType)]
pub struct HttpResponse<'a> {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: &'a Vec<u8>,
    pub streaming_strategy: Option<StreamingStrategy>,
}








pub fn set_root_hash(tree: &FilesHashes) {
    let root_hash = ic_certified_map::labeled_hash(LABEL_ASSETS, &tree.root_hash());
    set_certified_data(&root_hash[..]);
}


pub fn make_file_certificate_header(file_name: &str) -> (String, String) {
    let certificate: Vec<u8> = data_certificate().unwrap();
    // let file_hashes: FileHashes = get_file_hashes();
    with(&FRONTCODE_FILES_HASHES, |ffhs| {
        let witness: HashTree = ffhs.witness(file_name.as_bytes());
        let tree: HashTree = ic_certified_map::labeled(LABEL_ASSETS, witness);
        let mut serializer = serde_cbor::ser::Serializer::new(vec![]);
        serializer.self_describe().unwrap();
        tree.serialize(&mut serializer).unwrap();
        (
            "IC-Certificate".to_string(),
            format!("certificate=:{}:, tree=:{}:",
                base64::encode(&certificate),
                base64::encode(&serializer.into_inner())
            )
        )
    })
}



