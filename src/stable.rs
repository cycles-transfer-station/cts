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
use ic_cdk::{
    api::{
        stable::{
            stable_bytes,
            stable64_size,
            stable64_read,
            stable64_write,
            stable64_grow,
            StableMemoryError,
        },
        trap
    },
    export::{
        candid,
        candid::{CandidType},
        Principal,
    }
};
use wasabi_leb128::{ReadLeb128, WriteLeb128};
use crate::frontcode::{
    File,
    Files,
    FileHashes,
};




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



fn stable_get(start: u64, len: u64) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![0; len.try_into().unwrap()];
    stable64_read(start, &mut buf);
    buf
}

fn write_leb128<N: PrimInt + AsPrimitive<u8>>(num: N) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::<u8>::new();
    bytes.write_leb128(num).unwrap();
    bytes
}





// fn put_header(header: &Header) {

// }

// fn get_header() -> Header {

// }

// store first the total-bytes-length of the Files, then store the count of the files(how many files there are), then 
// for each file store the name-length as a leb128, then store the name, then store the File-struct-length as leb128, then the File-struct-bytes(i think candid),
// this way, to find a file, i dont have to de-serialize each File-struct, only the names, and if its not the right name, i can skip over the File-struct-bytes and go to the next filename 
pub fn put_files(files: &Files) {
    let mut bytes: Vec<u8> = vec![];
    bytes.write_leb128(files.len()).unwrap();
    for (file_name,file) in files.iter() {
        bytes.write_leb128(file_name.len()).unwrap();
        bytes.extend_from_slice(file_name.as_bytes());
        let file_bytes: Vec<u8> = candid::utils::encode_one(file).unwrap();
        bytes.write_leb128(file_bytes.len()).unwrap();
        bytes.extend_from_slice(&file_bytes);
    }
    bytes.splice(..0, write_leb128(bytes.len()));
    if bytes.len() as u64 > FILES_MAX_SIZE_BYTES {
        trap("the Files you are trying to write are too big for the FILES_MAX_SIZE_BYTES ");
    }
    let (first_leb128, first_leb128_bytes_size): (usize, usize) = Cursor::new(stable_get(FILES_START_I, 15)).read_leb128().unwrap(); // tells the number of the bytes that the serialization of this structure takes after this first leb128bytes
    let total_bytes_length_before = first_leb128_bytes_size + first_leb128;
    if bytes.len() < total_bytes_length_before {
        bytes.extend_from_slice(&vec![0; total_bytes_length_before - bytes.len()]);
    }
    stable64_write(FILES_START_I, &bytes);
}

pub fn get_files() -> Files {
    let mut files = Files::new();
    let (first_leb128, first_leb128_bytes_size): (u64, usize) = Cursor::new(stable_get(FILES_START_I, 15)).read_leb128().unwrap();
    if first_leb128 == 0u64 { return files; }
    let mut files_cursor = Cursor::new(stable_get(FILES_START_I + first_leb128_bytes_size as u64, first_leb128));
    let (items_size, items_size_leb128_bytes_size): (usize, usize) = files_cursor.read_leb128().unwrap();
    for i in 0..items_size {
        let (file_name_bytes_size, file_name_bytes_size_leb128_bytes_size): (u64, usize) = files_cursor.read_leb128().unwrap();
        let mut file_name_utf8: Vec<u8> = vec![0; file_name_bytes_size.try_into().unwrap()];
        files_cursor.read_exact(&mut file_name_utf8).unwrap();
        let (file_bytes_size, file_bytes_size_leb128_bytes_size): (u64, usize) = files_cursor.read_leb128().unwrap();
        let mut file_bytes: Vec<u8> = vec![0; file_bytes_size.try_into().unwrap()];
        files_cursor.read_exact(&mut file_bytes).unwrap();
        files.insert(String::from_utf8(file_name_utf8).unwrap(), candid::utils::decode_one(&file_bytes).unwrap());
    }
    assert_eq!(files_cursor.position() as usize, files_cursor.get_ref().len());
    files
}

// pub fn put_file(file_name: &str, file: &File) {
//     // go through the names of the [exi]sting files, if find a match, overwrite the File-struct-bytes-length and the File-struct-bytes and move the rest of the files over  
// }
// pub fn get_file(file_name: &str) -> File {

// }


pub fn put_file_hashes(file_hashes: &FileHashes) {
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
    if bytes.len() as u64 > FILEHASHES_MAX_SIZE_BYTES {
        trap("Cannot write file hashes, quested file_hashes-write is bigger than the FILEHASHES_MAX_SIZE_BYTES");
    } 
    let (first_leb128, first_leb128_bytes_size): (usize, usize) = Cursor::new(stable_get(FILEHASHES_START_I, 15)).read_leb128().unwrap(); // tells the number of the bytes that the serialization of this structure takes after this first leb128bytes
    let total_bytes_length_before = first_leb128_bytes_size + first_leb128;
    if bytes.len() < total_bytes_length_before {
        bytes.extend_from_slice(&vec![0; total_bytes_length_before - bytes.len()]);
    }
    stable64_write(FILEHASHES_START_I, &bytes);
}

pub fn get_file_hashes() -> FileHashes { 
    let mut file_hashes = FileHashes::default();
    let (first_leb128, first_leb128_bytes_size): (u64, usize) = Cursor::new(stable_get(FILEHASHES_START_I, 15)).read_leb128().unwrap();
    if first_leb128 == 0u64 { return file_hashes; } 
    let mut file_hashes_bytes = Cursor::new(stable_get(FILEHASHES_START_I + first_leb128_bytes_size as u64, first_leb128)); // mut cause its a cursor
    let (items_size, items_size_leb128_bytes_size): (u64, usize) = file_hashes_bytes.read_leb128().unwrap();
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




