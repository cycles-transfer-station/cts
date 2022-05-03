use crate::{
    UserData,

};

use std::collections::HashMap;

use ic_cdk::{
    api::{
        stable::{
            // stable_bytes,
            stable64_size,
            stable64_read,
            stable64_write,
            stable64_grow,
            StableMemoryError
        },
        trap
    },
    export::{
        Principal,
    }
};




const KIB: u64 = 1024;
const MIB: u64 = 1024*KIB;
const GIB: u64 = 1024*MIB;  
const WASM_PAGE_SIZE_BYTES: u64 = 64 * KIB; // 65536-bytes
const STABLE_MEMORY_MAX_SIZE_BYTES: u64 = 8 * GIB;

const STABLE_HEADER_SIZE_BYTES: u64 = 1 * KIB;



//test this if len is more than 64*KIB
fn stable64_get(start: u64, len: u64) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![0; len.try_into().unwrap()];
    stable64_read(start, &mut buf);
    buf
}


fn make_sure_stable_memory_is_big_enough(min_bytes: u64) -> Result<(), StableMemoryError> {
    if stable64_size() * WASM_PAGE_SIZE_BYTES < min_bytes {
        match stable64_grow( ( min_bytes - stable64_size()*WASM_PAGE_SIZE_BYTES ) / WASM_PAGE_SIZE_BYTES + 1 ) {
            Ok(_old_size_pages) => {},
            Err(stable_memory_error) => return Err(stable_memory_error)
        }
    }

    Ok(())
}



pub fn save_users_data<'a>(users_data: &'a HashMap<Principal, UserData>) {
    let mut b: Vec<u8> = Vec::new();
    serde_cbor::to_writer(&mut b, users_data);

    make_sure_stable_memory_is_big_enough(b.len() as u64+STABLE_HEADER_SIZE_BYTES+8).unwrap();

    stable64_write(STABLE_HEADER_SIZE_BYTES, &(b.len() as u64).to_be_bytes());
    stable64_write(STABLE_HEADER_SIZE_BYTES+8, &b);

}

fn get_users_data_bytes_len() -> u64 {
    u64::from_be_bytes(stable64_get(STABLE_HEADER_SIZE_BYTES, 8).try_into().unwrap())
}

pub fn read_users_data() -> HashMap<Principal, UserData> {
    let mut b: Vec<u8> = stable64_get(STABLE_HEADER_SIZE_BYTES+8, get_users_data_bytes_len());
    
    let users_data: HashMap<Principal, UserData> = serde_cbor::from_reader(& *&mut b[..]).unwrap();

    users_data
}


pub fn save_new_canisters(ncs: &Vec<Principal>) {
    let mut b: Vec<u8> = Vec::new();
    serde_cbor::to_writer(&mut b, ncs);
    
    let users_data_bytes_len: u64 = get_users_data_bytes_len();
    make_sure_stable_memory_is_big_enough(STABLE_HEADER_SIZE_BYTES+8+users_data_bytes_len+8+b.len() as u64).unwrap();

    stable64_write(STABLE_HEADER_SIZE_BYTES+8+users_data_bytes_len, &(b.len() as u64).to_be_bytes());
    stable64_write(STABLE_HEADER_SIZE_BYTES+8+users_data_bytes_len+8, &b);
    

}

pub fn read_new_canisters() -> Vec<Principal> {
    let users_data_bytes_len: u64 = get_users_data_bytes_len();
    let mut b: Vec<u8> = stable64_get(STABLE_HEADER_SIZE_BYTES+8+users_data_bytes_len+8, u64::from_be_bytes(stable64_get(STABLE_HEADER_SIZE_BYTES+8+users_data_bytes_len, 8).try_into().unwrap()));
    
    let ncs: Vec<Principal> = serde_cbor::from_reader(& *&mut b[..]).unwrap();

    ncs
}