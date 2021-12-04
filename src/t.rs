use sha2::Digest;
use wasabi_leb128::{ReadLeb128, WriteLeb128};

use crate::stable::FileHashes;


fn type_name<T>(_: &T) -> &'static str {
    std::any::type_name::<T>()
}

fn sha256(bytes: &[u8]) -> [u8; 32] { // [in]ferr[ed] lifetime on the &[u8]-param?
    let mut hasher: sha2::Sha256 = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}


#[test]
fn test1() {
    let sample_web_file: Vec<u8> = vec![1u8, 2u8, 3u8];
    let sample_web_file_hash: [u8; 32] = sha256(&sample_web_file);
    let mut file_hashes = FileHashes::default();
    println!("{:?}", file_hashes);

    file_hashes.insert("index.dart.js".to_string(), sample_web_file_hash);
    println!("{:?}", file_hashes);

    file_hashes.for_each(|k,v| {
        println!("k: {}: \n{:?},\nv: {}: \n{:?}", type_name(&k),k, type_name(&v),v);
    });
}

#[test]
fn testleb128() {
    let mut bytes: Vec<u8> = vec![];
    let count: u128 = 600;
    let leb128_size = bytes.write_leb128(count).unwrap();
    println!("uleb128_bytes with a size: {}: {:?}", leb128_size, bytes);
    let mut bytes: Vec<u8> = vec![];
    let count: i128 = -600;
    let leb128_size = bytes.write_leb128(count).unwrap();
    println!("sleb128_bytes with a size: {}: {:?}", leb128_size, bytes);
    let mut bytes = std::io::Cursor::new([0x83, 0x00, 0x03]);
    println!("type of the cursor: {:?}", type_name(&bytes));
    let (count, leb128_size): (i128, usize) = bytes.read_leb128().unwrap();
    println!("reading the sleb-bytes: 3: with a bytes-size: {}: {:?}", leb128_size, count);
    println!("bytes after read: {:?}", bytes );
    let (count, leb128_size): (u128, usize) = bytes.read_leb128().unwrap();
    println!("reading the sleb-bytes: 3: with a bytes-size: {}: {:?}", leb128_size, count);
    println!("bytes after read: {:?}", bytes );


}



#[test]
fn test2() {

}
