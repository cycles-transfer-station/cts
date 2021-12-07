use std::collections::HashMap;

use ic_cdk_macros::{update, query};
use ic_cdk::{
    export::candid::{CandidType, Deserialize, Func},
    api::{data_certificate, set_certified_data}
};
use ic_certified_map::{HashTree, AsHashTree};

use serde::Serialize;

use crate::tools::{
    sha256
};

use crate::stable::{
    FileHashes,
    put_file_hashes,
    get_file_hashes
};


const LABEL_ASSETS: &[u8] = b"http_assets";







#[derive(Clone, Debug, CandidType, Deserialize)]
struct Token {}

#[derive(Clone, Debug, CandidType, Deserialize)]
enum StreamingStrategy {
    Callback { callback: Func, token: Token},
}

#[derive(Clone, Debug, CandidType, Deserialize)]
struct StreamingCallbackHttpResponse {
    body: Vec<u8>,
    token: Option<Token>,
}


#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}


#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpResponse {
    status_code: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    streaming_strategy: Option<StreamingStrategy>,
}






pub fn set_root_hash(tree: &FileHashes) {
    let root_hash = ic_certified_map::labeled_hash(LABEL_ASSETS, &tree.root_hash());
    set_certified_data(&root_hash[..]); // [..] 
}


#[update]
pub fn upload_frontcode_files_chunks(file_path: String, file_bytes: Vec<u8>) -> () {
    let mut file_hashes: FileHashes = get_file_hashes();
    file_hashes.insert(file_path, sha256(&file_bytes));
    put_file_hashes(&file_hashes);
    set_root_hash(&file_hashes);
}


fn make_file_certificate_header<'a>(file_name: &str, asset_hashes: &'a FileHashes) -> (String, String) {
    let certificate: Vec<u8> = data_certificate().unwrap();
    let witness: HashTree<'a> = asset_hashes.witness(file_name.as_bytes());
    let tree: HashTree<'a> = ic_certified_map::labeled(LABEL_ASSETS, witness);
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
}


#[query]
pub fn http_request(quest: HttpRequest) -> HttpResponse {

    // let mut certificate_header: (String, String) = ("test".to_string(), "test".to_string());
    // let url_parts: Vec<&str> = quest.url.split('?').collect();
    // match url_parts[0] {
    //     file_name => {
 
            
            
    //     }
    // }
    let certificate_header: (String, String) = make_file_certificate_header(&quest.url, &get_file_hashes());
    
    HttpResponse {
        status_code: 200u16,
        headers: vec![
            certificate_header, 
            ("Content-Type".to_string(), "text/plain; charset=utf-8".to_string()),
            ("content-encoding".to_string(), "".to_string())
        ],
        body: "hello".as_bytes().to_vec(),
        streaming_strategy: None
    }
}

#[query]
pub fn public_get_file_hashes() -> Vec<(String, [u8; 32])> {
    let file_hashes = get_file_hashes();
    let mut vec = Vec::<(String, [u8; 32])>::new();
    file_hashes.for_each(|k,v| {
        vec.push((std::str::from_utf8(k).unwrap().to_string(), *v));
    });
    vec
}


#[update]
pub fn public_clear_file_hashes() {
    put_file_hashes(&FileHashes::default());
}



