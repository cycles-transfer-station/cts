use std::collections::HashMap;
use sha2::Digest;
use cts_lib::{
    tools::{
        localkey::refcell::{with},
        sha256
    },
    types::http_request::*,
};
use ic_certified_map::{self, RbTree};

use serde::Serialize;
use serde_bytes::ByteBuf;

use candid::{CandidType, Deserialize, Nat};

use crate::CTS_DATA;


#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct File {
    pub headers: Vec<(String, String)>,
    pub content_chunks: Vec<ByteBuf>
}
impl File {
    pub fn sha256_hash(&self) -> [u8; 32] {
        let mut hasher = sha2::Sha256::new();
        for chunk in self.content_chunks.iter() {
            hasher.update(chunk);    
        }
        hasher.finalize().into()
    }
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


pub fn hash_of_files(files: &Files) -> [u8; 32] {    
    let mut fields_hashes = Vec::<[u8; 64]>::new();
    for (filename, file) in files.iter() {
        fields_hashes.push(
            [
                sha256(&filename.as_bytes()), 
                file.sha256_hash()
            ]
            .concat().try_into().unwrap()
        );
    }
    fields_hashes.sort();
    let mut hasher = sha2::Sha256::new();
    for field_hash in fields_hashes.into_iter() {
        hasher.update(field_hash);    
    }
    let final_hash: [u8; 32] = hasher.finalize().into();
    final_hash    
}


#[test]
fn test_batch_hash() {
    let files = Files::from_iter([
        (
            "hi".to_string(), 
            File{headers: vec![], content_chunks: vec![
                ByteBuf::from(vec![0,1,2,3,4]),
            ]}
        ),
        (
            "abc".to_string(),
            File{
                headers: vec![],
                content_chunks: vec![
                    ByteBuf::from(vec![5,4,3,2,1]),
                    ByteBuf::from(vec![5,4,3,2,1]),
                ]
            }
        )
    ]);
    
    let batch_hash = hash_of_files(&files);
    println!("batch_hash: {:?}", batch_hash);
    
    assert_eq!(
        batch_hash,        
        [219, 253, 218, 23, 132, 249, 50, 93, 170, 82, 202, 17, 243, 251, 73, 76, 47, 206, 49, 222, 141, 190, 252, 117, 226, 21, 20, 91, 35, 106, 131, 48],        
    );
}