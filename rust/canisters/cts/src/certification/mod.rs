
use cts_lib::{
    ic_certified_map::{self, HashTree, AsHashTree},
    types::cts::{
        UserAndCB,
    },
    cts_cb_authorizations::CTS_CB_AUTHORIZATIONS_SEED,
    tools::{
        time_nanos_u64,
        sha256,
        localkey::refcell::{with},
    },
    consts::{
        NANOS_IN_A_SECOND,
        SECONDS_IN_A_DAY
    },
    ic_cdk::api::{set_certified_data, data_certificate, trap,},
};

use crate::{
    CTSData,
    CTS_DATA,
       
};
use serde::Serialize;






mod signature_map;
pub use signature_map::SignatureMap as CBAuths;



const LABEL_ASSETS: &[u8; 11] = b"http_assets";
const LABEL_CB_AUTHS: &[u8; 3] = b"sig";


pub fn put_cb_auth(cb_auths: &mut CBAuths, user_and_cb: UserAndCB) {
    cb_auths.put(
        sha256(&CTS_CB_AUTHORIZATIONS_SEED[..]),
        sha256(&user_and_cb.create_cts_cb_authorization_msg()[..]),
        time_nanos_u64().saturating_add((NANOS_IN_A_SECOND * SECONDS_IN_A_DAY) as u64) 
    );
}



pub fn set_root_hash(cts_data: &CTSData) {
    use ic_certified_map::{fork_hash, labeled_hash};
    let root_hash = fork_hash(
        &labeled_hash(LABEL_ASSETS, &cts_data.frontcode_files_hashes.root_hash()),
        &labeled_hash(LABEL_CB_AUTHS, &cts_data.cb_auths.root_hash())
    );
    set_certified_data(&root_hash[..]);
}


pub fn make_file_certificate_header(file_name: &str) -> (String, String) {
    let certificate: Vec<u8> = data_certificate().unwrap_or(vec![]);
    with(&CTS_DATA, |cts_data| {
        let witness: HashTree = cts_data.frontcode_files_hashes.witness(file_name.as_bytes());
        let tree: HashTree = ic_certified_map::fork(
            ic_certified_map::labeled(LABEL_ASSETS, witness),
            HashTree::Pruned(ic_certified_map::labeled_hash(LABEL_CB_AUTHS, &cts_data.cb_auths.root_hash())),
        );
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

pub fn get_cb_auth_(user_and_cb: UserAndCB) -> Vec<u8> {
    with(&CTS_DATA, |cts_data| {
        let witness: HashTree = cts_data.cb_auths.witness(
            sha256(&CTS_CB_AUTHORIZATIONS_SEED[..]),
            sha256(&user_and_cb.create_cts_cb_authorization_msg()[..]),
        ).unwrap_or_else(|| trap("User and cb not found in the auth cache. Must call set_cts_cb_auth before calling this method."));
        let tree: HashTree = ic_certified_map::fork(
            HashTree::Pruned(ic_certified_map::labeled_hash(LABEL_ASSETS, &cts_data.frontcode_files_hashes.root_hash())),
            ic_certified_map::labeled(LABEL_CB_AUTHS, witness),            
        );
        let mut serializer = serde_cbor::ser::Serializer::new(vec![]);
        serializer.self_describe().unwrap();
        #[derive(Serialize)]
        struct Auth<'a>{ certificate: Vec<u8>, tree: HashTree<'a>}
        Auth{
            certificate: data_certificate().unwrap_or_else(|| trap("can get cts_cb_auth in an unreplicated query call.")),
            tree: tree,
        }
        .serialize(&mut serializer).unwrap();
        serializer.into_inner()
    })
}

