use std::{
    collections::HashMap,
    io::{
        Cursor,
        Read,
        Seek
    },
    cmp::max,
};
use num_traits::{PrimInt,AsPrimitive};
use ic_certified_map::RbTree;
use ic_cdk::api::stable::{
    stable_bytes,
    stable64_size,
    stable64_read,
    stable64_write,
    stable64_grow,
    StableMemoryError,
};

use wasabi_leb128::{ReadLeb128, WriteLeb128};



const KIB: u64 = 1024;
const MIB: u64 = 1024*KIB;
const GIB: u64 = 1024*MIB;  
const WASM_PAGE_SIZE_BYTES: u64 = 64 * KIB;
const STABLE_MEMORY_MAX_SIZE_BYTES: u64 = 8 * GIB;


const HEADER_START_I: u64 = 0;
const HEADER_SIZE_BYTES: u64 = 1 * KIB;

const FILEHASHES_START_I: u64 = HEADER_SIZE_BYTES;
const FILEHASHES_MAX_SIZE_BYTES: u64 = 1 * KIB;

const FILES_START_I: u64 = FILEHASHES_START_I + FILEHASHES_MAX_SIZE_BYTES;
const FILES_MAX_SIZE_BYTES: u64 = 20 * MIB;



#[repr(packed)]
struct Header {
    magic: [u8; 3],

}


pub type FileHashes = RbTree<String, ic_certified_map::Hash>;
pub type Files = HashMap<&'static str, (Vec<(String, String)>, &'static [u8])>;



fn stable_get(start: u64, len: u64) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![0; len.try_into().unwrap()];
    stable64_read(start, &mut buf);
    let buf: Vec<u8> = buf;
    buf
}

fn write_leb128<N: PrimInt + AsPrimitive<u8>>(num: N) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::<u8>::new();
    bytes.write_leb128(num).unwrap();
    let bytes: Vec<u8> = bytes;
    bytes
}





// fn put_header(header: &Header) {

// }

// fn get_header() -> Header {

// }

// pub fn put_files(files: &Files) {

// }

// fn get_files() -> Files {
    
// }

pub fn put_file_hashes(file_hashes: &FileHashes) {
    let (first_leb128, first_leb128_bytes_size): (u64, usize) = Cursor::new(stable_get(FILEHASHES_START_I, 15)).read_leb128().unwrap(); // tells the number of the bytes that the serialization of this structure takes after this first leb128bytes
    let total_bytes_length_before: usize = first_leb128 as usize + first_leb128_bytes_size;
    let mut bytes: Vec<u8> = vec![];
    let mut items_size: u64 = 0;
    file_hashes.for_each(|k: &[u8], v: &[u8; 32]| {
        bytes.write_leb128(k.len()).unwrap();       // k.len(): usize - uleb128
        bytes.extend_from_slice(k);
        bytes.extend_from_slice(v); // v is 32-bytes-length in this hashtree
        items_size += 1;
    });
    bytes.splice(..0, write_leb128(items_size));
    bytes.splice(..0, write_leb128(bytes.len()));
    if bytes.len() < total_bytes_length_before {
        bytes.extend_from_slice(&vec![0; total_bytes_length_before - bytes.len()]);
    }
    stable64_write(FILEHASHES_START_I, &bytes);
}

fn get_file_hashes() -> FileHashes { 
    let (first_leb128, first_leb128_bytes_size): (u64, usize) = Cursor::new(stable_get(FILEHASHES_START_I, 15)).read_leb128().unwrap();
    let mut file_hashes_bytes = Cursor::new(stable_get(FILEHASHES_START_I + first_leb128_bytes_size as u64, first_leb128));
    let (items_size, items_size_leb128_bytes_size): (u64, usize) = file_hashes_bytes.read_leb128().unwrap();
    let mut file_hashes: FileHashes = FileHashes::default();
    for item in 0..items_size {
        let (key_bytes_size, key_bytes_size_leb128_bytes_size): (u64, usize) = file_hashes_bytes.read_leb128().unwrap();
        let mut key_bytes: Vec<u8> = vec![0; key_bytes_size.try_into().unwrap()];
        file_hashes_bytes.read_exact(&mut key_bytes).unwrap();
        let mut value_bytes: [u8; 32] = [0; 32];
        file_hashes_bytes.read_exact(&mut value_bytes).unwrap();
        file_hashes.insert(String::from_utf8(key_bytes).unwrap(), value_bytes);
    }
    assert_eq!(file_hashes_bytes.position() as usize, file_hashes_bytes.get_ref().len());
    file_hashes
}




