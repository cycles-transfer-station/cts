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
use std::convert::TryInto;


#[cfg(test)]
mod t;

mod tools;

mod stable;
use stable::{FileHashes, Files};


mod frontcode;
use frontcode::{http_request, upload_frontcode_files_chunks, public_get_file_hashes, public_clear_file_hashes};










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




#[query]
fn __get_candid_interface_tmp_hack() -> String {
    include_str!("../cycles-transfer-station.did").to_string()
}






