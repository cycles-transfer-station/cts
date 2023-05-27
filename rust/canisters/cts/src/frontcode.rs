use std::collections::HashMap;

use cts_lib::{
    ic_cdk::{
        export::candid::{CandidType, Deserialize, Func, Nat},
        api::{data_certificate, set_certified_data}
    },
    ic_certified_map::{self, RbTree, HashTree, AsHashTree},
    tools::{
        localkey::refcell::{with},
    },
};

use serde::Serialize;
use serde_bytes::ByteBuf;

use crate::CTS_DATA;



const LABEL_ASSETS: &[u8; 11] = b"http_assets";

#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct File {
    pub headers: Vec<(String, String)>,
    pub content_chunks: Vec<ByteBuf>
}
pub type Files = HashMap<String, File>;
pub type FilesHashes = RbTree<String, ic_certified_map::Hash>;


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
    pub headers: Vec<(&'a str, &'a str)>,
    pub body: &'a ByteBuf,
    pub streaming_strategy: Option<StreamStrategy<'a>>,
}

#[derive(Clone, Debug, CandidType)]
pub enum StreamStrategy<'a> {
    Callback { callback: Func, token: StreamCallbackToken<'a>},
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamCallbackToken<'a> {
    pub key: &'a str,
    pub content_encoding: &'a str,
    pub index: Nat,
    // We don't care about the sha, we just want to be backward compatible.
    pub sha256: Option<[u8; 32]>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamCallbackTokenBackwards {
    pub key: String,
    pub content_encoding: String,
    pub index: Nat,
    // We don't care about the sha, we just want to be backward compatible.
    pub sha256: Option<[u8; 32]>,
}

#[derive(Clone, Debug, CandidType)]
pub struct StreamCallbackHttpResponse<'a> {
    pub body: &'a ByteBuf,
    pub token: Option<StreamCallbackToken<'a>>,
}






pub fn set_root_hash(tree: &FilesHashes) {
    let root_hash = ic_certified_map::labeled_hash(LABEL_ASSETS, &tree.root_hash());
    set_certified_data(&root_hash[..]);
}


pub fn make_file_certificate_header(file_name: &str) -> (String, String) {
    let certificate: Vec<u8> = data_certificate().unwrap_or(vec![]);
    with(&CTS_DATA, |cts_data| {
        let witness: HashTree = cts_data.frontcode_files_hashes.witness(file_name.as_bytes());
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


pub fn create_opt_stream_callback_token<'a>(file_name: &'a str, file: &'a File, chunk_i: usize) -> Option<StreamCallbackToken<'a>> {
    if file.content_chunks.len() > chunk_i + 1 {
        Some(StreamCallbackToken{
            key: file_name,
            content_encoding: file.headers.iter().find(|header| { header.0.eq_ignore_ascii_case("Content-Encoding") }).map(|header| { &*(header.1) }).unwrap_or(""),
            index: Nat::from(chunk_i + 1),
            sha256: {
                with(&CTS_DATA, |cts_data| {
                    cts_data.frontcode_files_hashes.get(file_name.as_bytes())
                    .map(|hash| { hash.clone() })
                })  
            }
        })
    } else {
        None
    }
}
