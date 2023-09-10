

// [48, 60, 48, 12, 6, 10, 43, 6, 1, 4, 1, 131, 184, 67, 1, 2, 3, 44, 0, 10, 0, 0, 0, 0, 0, 0, 0, 7, 1, 1, 118, 90, 236, 5, 49, 201, 75, 5, 238, 31, 207, 22, 219, 124, 220, 50, 162, 252, 96, 83, 28, 73, 204, 210, 46, 44, 87, 145, 95, 48, 50, 189]

// seed == user_id and cycles_bank_id

//use ic_crypto_iccsa::verify;

use crate::CTS_ID;
use cts_lib::{
    tools::localkey,
    types::cts::{UserAndCB, CTS_CB_AUTHORIZATIONS_SEED},
};
use candid::Principal;

use ic_crypto_iccsa::{types::{SignatureBytes, PublicKeyBytes}};
use ic_types::crypto::threshold_sig::IcRootOfTrust;    





pub const IC_ROOT_KEY: [u8; 96] = [129, 76, 14, 110, 199, 31, 171, 88, 59, 8, 189, 129, 55, 60, 37, 92, 60, 55, 27, 46, 132, 134, 60, 152, 164, 241, 224, 139, 116, 35, 93, 20, 251, 93, 156, 12, 213, 70, 217, 104, 95, 145, 58, 12, 11, 44, 197, 52, 21, 131, 191, 75, 67, 146, 228, 103, 219, 150, 214, 91, 155, 180, 203, 113, 113, 18, 248, 71, 46, 13, 90, 77, 20, 80, 95, 253, 116, 132, 176, 18, 145, 9, 28, 95, 135, 185, 136, 131, 70, 63, 152, 9, 26, 11, 170, 174];


fn cts_cb_authorizations_public_key_bytes() -> PublicKeyBytes {
    let cts_id: Principal = localkey::cell::get(&CTS_ID);
    let cts_id_slice = cts_id.as_slice();
    let mut v: Vec<u8> = Vec::new();
    v.push(cts_id_slice.len() as u8);
    v.extend(cts_id_slice);
    v.extend(CTS_CB_AUTHORIZATIONS_SEED);
    PublicKeyBytes(v)
}

pub fn is_cts_cb_authorization_valid(user_and_cb: UserAndCB, authorization: Vec<u8>) -> bool {
    ic_crypto_iccsa::verify(
        &user_and_cb.create_cts_cb_authorization_msg(),
        SignatureBytes(authorization),
        cts_cb_authorizations_public_key_bytes(),
        IcRootOfTrust::from(IC_ROOT_KEY).as_ref()
    )
    .is_ok()
}




