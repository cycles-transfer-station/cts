use std::cell::RefCell;
use std::collections::HashSet;
use serde_bytes::ByteBuf;
use num_traits::cast::ToPrimitive;
use cts_lib::{
    types::http_request::*,
    tools::{
        localkey::{
            refcell::{
                with, 
                with_mut,
            }
        },
        caller_is_controller_gaurd,
    },
};
use ic_cdk::{
    api::{
        trap,
        caller, 
        call::{
            arg_data,
            reply,
        },
    },
    update, 
    query, 
    init, 
    pre_upgrade, 
    post_upgrade
};
use candid::{
    Principal,
    CandidType,
    Deserialize,
    Func,
};
use canister_tools::MemoryId;

mod frontcode;
use frontcode::{
    File, 
    Files, 
    FilesHashes, 
    create_opt_stream_callback_token,
};

mod certification;
use certification::*;


// -------


#[derive(CandidType, Deserialize)]
pub struct CTSData {
    frontcode_files: Files,
    frontcode_files_hashes: FilesHashes,
    batch_creators: HashSet<Principal>,
    current_batch: Files,
}
impl CTSData {
    fn new() -> Self {
        Self {
            frontcode_files: Files::new(),
            frontcode_files_hashes: FilesHashes::new(),
            batch_creators: HashSet::new(),
            current_batch: Files::new(),
        }
    }
}

 
const CTS_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);


thread_local! {
    pub static CTS_DATA: RefCell<CTSData> = RefCell::new(CTSData::new());    
}


// -------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CTSInit {
    batch_creators: Option<HashSet<Principal>>,
}

#[init]
fn init(cts_init: CTSInit) {
    canister_tools::init(&CTS_DATA, CTS_DATA_MEMORY_ID);
    
    with_mut(&CTS_DATA, |d| {
        if let Some(batch_creators) = cts_init.batch_creators {
            d.batch_creators = batch_creators;
        }
    });
} 


// -------------------------------------------------------------


#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() { 
    canister_tools::post_upgrade(&CTS_DATA, CTS_DATA_MEMORY_ID, None::<fn(CTSData) -> CTSData>);
    
    with(&CTS_DATA, |cts_data| {
        set_root_hash(&cts_data);
    });
} 



// ----- METRICS -----

#[derive(CandidType, Deserialize)]
pub struct CTSMetrics {
    stable_size: u64,
    cycles_balance: u128,
}

#[query]
pub fn controller_view_metrics() -> CTSMetrics {
    caller_is_controller_gaurd(&caller());
    with(&CTS_DATA, |_cts_data| {
        CTSMetrics {
            stable_size: ic_cdk::api::stable::stable64_size(),
            cycles_balance: ic_cdk::api::canister_balance128(),
        }
    })
}



// ---------------------------- :FRONTCODE. -----------------------------------


fn caller_is_batch_creator_gaurd(d: &CTSData) {
    if d.batch_creators.contains(&caller()) == false {
        trap("Caller must be a batch-creator");
    } 
}


#[derive(CandidType, Deserialize)]
pub struct CreateBatch {}

#[update]
pub fn create_batch(_q: CreateBatch) {
    with(&CTS_DATA, caller_is_batch_creator_gaurd);
    
    with_mut(&CTS_DATA, |d| {
        d.current_batch = Files::new();        
    }); 
}


#[derive(CandidType, Deserialize)]
pub struct UploadFile {
    pub filename: String,
    pub headers: Vec<(String, String)>,
    pub first_chunk: ByteBuf,
    pub chunks: u32
}

#[update]
pub fn upload_file(q: UploadFile) {
    with(&CTS_DATA, caller_is_batch_creator_gaurd);
    
    if q.chunks == 0 {
        trap("there must be at least 1 chunk.");
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.current_batch.insert(
            q.filename, 
            File{
                headers: q.headers,
                content_chunks: {
                    let mut v: Vec<ByteBuf> = vec![ByteBuf::new(); q.chunks.try_into().unwrap()];
                    v[0] = q.first_chunk;
                    v
                }
            }
        ); 
    });
}

#[update]
pub fn upload_file_chunks(file_path: String, chunk_i: u32, chunk: ByteBuf) -> () {
    with(&CTS_DATA, caller_is_batch_creator_gaurd);
    
    with_mut(&CTS_DATA, |cts_data| {
        match cts_data.current_batch.get_mut(&file_path) {
            Some(file) => {
                file.content_chunks[chunk_i as usize] = chunk;
            },
            None => {
                trap("File not found. Call the upload_file method to upload a new file.");
            }
        }
    });    
}

#[query]
pub fn view_current_batch_file_hashes() -> Vec<(String, [u8; 32])> {
    with(&CTS_DATA, |cts_data| { 
        let mut vec = Vec::<(String, [u8; 32])>::new();
        for (k,file) in cts_data.current_batch.iter() {
            vec.push((k.clone(), file.sha256_hash()));
        }
        vec
    })
}

#[update]
pub fn controller_commit_batch() {
    caller_is_controller_gaurd(&caller());

    with_mut(&CTS_DATA, |d| {
        d.frontcode_files = std::mem::take(&mut d.current_batch);
        d.frontcode_files_hashes = FilesHashes::new();        
        for (filename, file) in d.frontcode_files.iter() {
            d.frontcode_files_hashes.insert(
                filename.clone(),
                file.sha256_hash(), 
            );
        }
        set_root_hash(&d);
    });
}

#[query]
pub fn view_file_hashes() -> Vec<(String, [u8; 32])> {
    with(&CTS_DATA, |cts_data| { 
        let mut vec = Vec::<(String, [u8; 32])>::new();
        cts_data.frontcode_files_hashes.for_each(|k,v| {
            vec.push((std::str::from_utf8(k).unwrap().to_string(), *v));
        });
        vec
    })
}

#[query]
pub fn view_batch_creators() -> HashSet<Principal> {
    with(&CTS_DATA, |d| {
        d.batch_creators.clone()
    })
}

#[update]
pub fn controller_add_batch_creators(batch_creators: HashSet<Principal>) {
    caller_is_controller_gaurd(&caller());

    with_mut(&CTS_DATA, |d| {
        for p in batch_creators.into_iter() {
            d.batch_creators.insert(p);
        }
    });
}

#[update]
pub fn controller_remove_batch_creators(batch_creators: HashSet<Principal>) {
    caller_is_controller_gaurd(&caller());

    with_mut(&CTS_DATA, |d| {
        for p in batch_creators.into_iter() {
            d.batch_creators.remove(&p);
        }
    });
}




#[update]
pub fn controller_clear_files() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.frontcode_files = Files::new();
        cts_data.frontcode_files_hashes = FilesHashes::new();
        set_root_hash(&cts_data);
    });
}

#[update]
pub fn controller_clear_file(filename: String) {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.frontcode_files.remove(&filename);
        cts_data.frontcode_files_hashes.delete(filename.as_bytes());
        set_root_hash(&cts_data);
    });
}

// -------

#[export_name = "canister_query http_request"]
pub fn http_request() {
    
    let (quest,): (HttpRequest,) = arg_data::<(HttpRequest,)>(); 
    
    let file_name: &str = quest.url.split("?").next().unwrap();
    
    with(&CTS_DATA, |cts_data| {
        match cts_data.frontcode_files.get(file_name) {
            None => {
                reply::<(HttpResponse,)>(
                    (HttpResponse {
                        status_code: 404,
                        headers: vec![],
                        body: &ByteBuf::from(vec![]),
                        streaming_strategy: None
                    },)
                );        
            }, 
            Some(file) => {
                let (file_certificate_header_key, file_certificate_header_value): (String, String) = make_file_certificate_header(file_name); 
                let mut headers: Vec<(&str, &str)> = vec![(&file_certificate_header_key, &file_certificate_header_value),];
                headers.extend(file.headers.iter().map(|tuple: &(String, String)| { (&*tuple.0, &*tuple.1) }));
                reply::<(HttpResponse,)>(
                    (HttpResponse {
                        status_code: 200,
                        headers: headers, 
                        body: &file.content_chunks[0],
                        streaming_strategy: if let Some(stream_callback_token) = create_opt_stream_callback_token(file_name, file, 0) {
                            Some(StreamStrategy::Callback{ 
                                callback: StreamCallback(Func{
                                    principal: ic_cdk::api::id(),
                                    method: "http_request_stream_callback".to_string(),
                                }),
                                token: stream_callback_token 
                            })
                        } else {
                            None
                        }
                    },)
                );
            }
        }
    });
    return;
}




#[export_name = "canister_query http_request_stream_callback"]
fn http_request_stream_callback() {
    let (token,): (StreamCallbackTokenBackwards,) = arg_data::<(StreamCallbackTokenBackwards,)>(); 
    
    with(&CTS_DATA, |cts_data| {
        match cts_data.frontcode_files.get(&token.key) {
            None => {
                trap("the file is not found");        
            }, 
            Some(file) => {
                let chunk_i: usize = token.index.0.to_usize().unwrap_or_else(|| { trap("invalid index"); }); 
                reply::<(StreamCallbackHttpResponse,)>((StreamCallbackHttpResponse {
                    body: &file.content_chunks[chunk_i],
                    token: create_opt_stream_callback_token(&token.key, file, chunk_i),
                },));
            }
        }
    })
    
}



ic_cdk::export_candid!();







