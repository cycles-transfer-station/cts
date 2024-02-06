use std::collections::HashMap;

use cts_lib::{
    tools::{
        localkey::refcell::{with},
    },
    types::http_request::*,
};
use ic_certified_map::{self, RbTree};

use serde::Serialize;
use serde_bytes::ByteBuf;

use candid::{CandidType, Deserialize, Nat};

use crate::CTS_DATA;


#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct File {
    pub headers: Vec<(String, String)>,
    pub content_chunks: Vec<ByteBuf>
}
pub type Files = HashMap<String, File>;
pub type FilesHashes = RbTree<String, ic_certified_map::Hash>;




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
