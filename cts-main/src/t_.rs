use wasabi_leb128::{ReadLeb128, WriteLeb128};

use crate::tools::{
    sha256
};

use crate::stable::{
    FileHashes,
    put_file_hashes,
    get_file_hashes
};

use std::any::type_name;


fn type_name_of_val<T: ?Sized>(_val: &T) -> &'static str {
    type_name::<T>()
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
        println!("k: {}: \n{:?},\nv: {}: \n{:?}", type_name_of_val(&k),k, type_name_of_val(&v),v);
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
    println!("type of the cursor: {:?}", type_name_of_val(&bytes));
    let (count, leb128_size): (i128, usize) = bytes.read_leb128().unwrap();
    println!("reading the sleb-bytes: 3: with a bytes-size: {}: {:?}", leb128_size, count);
    println!("bytes after read: {:?}", bytes );
    let (count, leb128_size): (u128, usize) = bytes.read_leb128().unwrap();
    println!("reading the sleb-bytes: 3: with a bytes-size: {}: {:?}", leb128_size, count);
    println!("bytes after read: {:?}", bytes );


}



#[test]
fn test_put_get_file_hashes() {
    // let mut file_hashes = get_file_hashes();

}

#[test]
fn test_iter_map() {
    let es: Vec<String> = vec![String::from("hello "), String::from(" hi "), String::from("whatsup")];
    for e in es.iter() {
        println!("{}", type_name_of_val(&e));
        println!("{}", type_name_of_val(&e.clone()));
        println!("{:?}", e);

        break;
    }
    for e in es.iter().map(|s| s.trim()) {
        println!("{}", type_name_of_val(&e));
        println!("{}", type_name_of_val(&e.to_string()));
        println!("{:?}", e);

        
        break;
    }
    

}




