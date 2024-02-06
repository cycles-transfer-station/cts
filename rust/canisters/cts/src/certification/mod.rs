use cts_lib::tools::localkey::refcell::with;
use ic_cdk::api::{set_certified_data, data_certificate};
use ic_certified_map::{self, HashTree, AsHashTree};
use crate::{
    CTSData,
    CTS_DATA,
};
use serde::Serialize;


const LABEL_ASSETS: &[u8; 11] = b"http_assets";


pub fn set_root_hash(cts_data: &CTSData) {
    use ic_certified_map::labeled_hash;
    let root_hash = labeled_hash(LABEL_ASSETS, &cts_data.frontcode_files_hashes.root_hash());
    set_certified_data(&root_hash[..]);
}

pub fn make_file_certificate_header(file_name: &str) -> (String, String) {
    let certificate: Vec<u8> = data_certificate().unwrap_or(vec![]);
    with(&CTS_DATA, |cts_data| {
        let witness: HashTree = cts_data.frontcode_files_hashes.witness(file_name.as_bytes());
        let tree: HashTree = ic_certified_map::labeled(LABEL_ASSETS, witness);
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
    })
}
