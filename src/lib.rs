#![allow(unused)] // take this out when done

use ic_cdk::{
    api::{
        caller,
        canister_balance128,
        id,
        time,
        trap,
        data_certificate,
        set_certified_data,
        print,
        call::{},
        stable::{
            stable_bytes,
            stable64_size,
            stable64_read,
            stable64_write,
            stable64_grow,
            StableMemoryError,
        }
    },
    export::{
        candid,
        candid::{CandidType},
        Principal,
    },
    block_on,


};
use ic_cdk_macros::{
    init,
    pre_upgrade,
    post_upgrade,
    inspect_message,
    heartbeat,
    update,
    query,
    import
};
use ic_certified_map::RbTree;
use serde::{Serialize, Deserialize};

#[cfg(test)]
mod t;

mod tools;
mod stable;
mod frontcode;

use frontcode::{File, Files, FilesHashes, HttpRequest, HttpResponse, set_root_hash, make_file_certificate_header};






thread_local! {
    static FRONTCODE_FILES:        RefCell<Files>       = RefCell::new(Files::new());
    static FRONTCODE_FILES_HASHES: RefCell<FilesHashes> = RefCell::new(FilesHashes::default());

}







#[init]
fn test1() -> () {
    // let files: Files = Files::new();
    // let file_hashes: <RbTree<&'static str, [u8; 32]> as Trait>::new();
    // stable::put_files(&files);
    // stable::put_file_hashes(&file_hashes);
    if stable64_size() < 1u64 {
        stable64_grow(1);
    }
    
}

#[export_name = "start"] 
fn start() {

}


#[pre_upgrade]
fn pre_upgrade() -> () {

}

#[post_upgrade]
fn post_upgrade() -> () {

}



#[update]
pub fn upload_frontcode_file_chunks(file_path: String, file: File) -> () {
    // let mut file_hashes: FileHashes = get_file_hashes();
    // file_hashes.insert(file_path.clone(), sha256(&file.content));
    // put_file_hashes(&file_hashes);
    FRONTCODE_FILES_HASHES.borrow_mut().insert(file_path.clone(), sha256(&file.content));
    
    // set_root_hash(&file_hashes);
    set_root_hash(FRONTCODE_FILES_HASHES.borrow());

    // let mut files: Files = get_files();
    // files.insert(file_path, file);
    // put_files(&files);
    FRONTCODE_FILES.borrow_mut().insert(file_path, file);
}


#[query]
pub fn http_request(quest: HttpRequest) -> HttpResponse {
    let file_name = quest.url;
    // let files: Files = get_files();
    let file: &File = FRONTCODE_FILES.borrow().get(&file_name).unwrap();
    let certificate_header: (String, String) = make_file_certificate_header(&file_name);
    
    HttpResponse {
        status_code: 200,
        headers: vec![
            certificate_header, 
            ("content-type".to_string(), file.content_type.clone()),
            ("content-encoding".to_string(), file.content_encoding.clone())
        ],
        body: file.content.to_vec(),
        streaming_strategy: None
    }
}


#[query]
pub fn public_get_file_hashes() -> Vec<(String, [u8; 32])> {
    let file_hashes = FRONTCODE_FILES_HASHES.borrow();
    let mut vec = Vec::<(String, [u8; 32])>::new();
    file_hashes.for_each(|k,v| {
        vec.push((std::str::from_utf8(k).unwrap().to_string(), *v));
    });
    vec
}


#[update]
pub fn public_clear_file_hashes() {
    // put_file_hashes(&FileHashes::default());
    FRONTCODE_FILES_HASHES.replace(FilesHashes::default());
    set_root_hash(FRONTCODE_FILES_HASHES.borrow());
}










#[update]
pub fn see_caller() -> Principal {
    caller()
}


#[update]
fn test_update(num: u32) -> u32 {
    num + 5
}








#[update]
fn public_stable_bytes() -> Vec<u8> {
    stable_bytes()
}

#[update]
fn public_stable_grow(grow: u64) -> u64 { // grows by memory pages 1grow = +65536bytes 
    match stable64_grow(grow) {
        Ok(grow_sponse) => return grow_sponse,
        Err(e) => panic!("panicking")
    };
}

#[update]
fn public_stable_size() -> u64 { // gives back count of wasm memory pages (1 wasm memory page = 65536-bytes)
    stable64_size()
}

#[update]
fn public_stable_write(offset: u64, buf: Vec<u8>) -> () { // offset is the byte index // offset-i is the first i to be write on
    stable64_write(offset, &buf);                           
}

#[update]
fn public_stable_read(offset: u64, length: u64) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![0; length.try_into().unwrap()];
    stable64_read(offset, &mut buf);
    buf
}


#[export_name = "canister_heartbeat"]
fn heartbeat() {
    // heartbeat_counter += 1;
}




// #[query]
// fn __get_candid_interface_tmp_hack() -> String {
//     include_str!("../cycles-transfer-station.did").to_string()
// }






