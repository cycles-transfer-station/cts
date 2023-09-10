

// [48, 60, 48, 12, 6, 10, 43, 6, 1, 4, 1, 131, 184, 67, 1, 2, 3, 44, 0, 10, 0, 0, 0, 0, 0, 0, 0, 7, 1, 1, 118, 90, 236, 5, 49, 201, 75, 5, 238, 31, 207, 22, 219, 124, 220, 50, 162, 252, 96, 83, 28, 73, 204, 210, 46, 44, 87, 145, 95, 48, 50, 189]



use ic_crypto_iccsa::{types::{SignatureBytes, PublicKeyBytes}};
use ic_types::crypto::threshold_sig::IcRootOfTrust;    
use candid::Principal;


pub fn verify(
    msg: impl AsRef<[u8]>,
    authorization: Vec<u8>,
    canister: Principal,
    seed: impl AsRef<[u8]>,
    ic_root_key: [u8; 96],
) -> bool {
    ic_crypto_iccsa::verify(
        msg.as_ref(),
        SignatureBytes(authorization),
        make_canister_public_key_bytes(canister, seed),
        IcRootOfTrust::from(ic_root_key).as_ref()
    )
    .is_ok()
}

fn make_canister_public_key_bytes(canister_id: Principal, seed: impl AsRef<[u8]>) -> PublicKeyBytes {
    let canister_id_slice = canister_id.as_slice();
    let mut v: Vec<u8> = Vec::new();
    v.push(canister_id_slice.len() as u8);
    v.extend(canister_id_slice);
    v.extend(seed.as_ref());
    PublicKeyBytes(v)
}

