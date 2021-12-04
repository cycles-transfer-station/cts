use ic_cdk_macros::{update, query};
use ic_cdk::export::candid::{CandidType, Deserialize, Func};
use ic_cdk::api::{data_certificate, set_certified_data};




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
struct HttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}


#[derive(Clone, Debug, CandidType, Deserialize)]
struct HttpResponse {
    status_code: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    streaming_strategy: Option<StreamingStrategy>,
}






fn set_root_hash(tree: &AssetHashes) {
    let root_hash = ic_certified_map::labeled_hash(LABEL_ASSETS, &tree.root_hash());
    set_certified_data(&root_hash); // [..] 
}


#[update]
fn upload_frontcode_files_chunks() -> () {

}


fn make_file_certificate_header(file_name: &str, asset_hashes: &AssetHashes) -> (String, String) {
    let certificate: Vec<u8> = data_certificate().unwrap();
    let witness: HashTree<'a> = asset_hashes.witness(file_name.as_bytes());
    let tree: HashTree<'a> = ic_certified_map::labeled(LABEL_ASSETS, witness)
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
fn http_request(quest: HttpRequest) -> HttpResponse {
     


    let url_parts: Vec<&str> = quest.url.split('?').collect();
    match url_parts[0] {
        file_name => {
            let certificate_header: (String, String) = make_file_certificate_header(file_name, get_asset_hashes()); 
            
            
        }
    }
    
    
    HttpResponse {
        status_code: 200u16,
        headers: vec![("Hello".to_string(), "Hello".to_string())],
        body: vec![1,2,3],
        streaming_strategy: None;
    }
}







