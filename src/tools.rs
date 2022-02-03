use sha2::Digest;



pub fn sha256(bytes: &[u8]) -> [u8; 32] { // [in]ferr[ed] lifetime on the &[u8]-param?
    let mut hasher: sha2::Sha256 = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}



fn principal_as_thirty_bytes(p: &Principal) -> [u8; 30] {
    let mut bytes: [u8; 30] = [0; 30];
    let p_bytes: &[u8] = p.as_slice();
    bytes[0] = p_bytes.len() as u8; 
    bytes[1 .. p_bytes.len() + 1].copy_from_slice(p_bytes); 
    bytes
}

fn thirty_bytes_as_principal(bytes: &[u8; 30]) -> Principal {
    Principal::from_slice(bytes[1..1 + bytes[0] as usize])
} 



fn principal_as_an_icpsubaccount(p: &Principal) -> IcpIdSub {
    let mut sub_bytes = [0u8; 32];
    sub_bytes[..30].copy_from_slice(&principal_as_thirty_bytes(p));
    IcpIdSub(sub_bytes)
}



pub fn user_icp_balance_id(user: &Principal) -> IcpId {
    IcpId::new(&id(), &principal_as_an_icpsubaccount(user))
}


pub fn user_cycles_balance_topup_memo_bytes(user: &Principal) -> [u8; 32] {
    let mut memo_bytes = [0u8; 32];
    memo_bytes[..2].copy_from_slice(b"TP");
    memo_bytes[2..].copy_from_slice(&principal_as_thirty_bytes(user));
    memo_bytes
}

