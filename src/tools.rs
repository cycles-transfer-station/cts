use std::collections::HashMap;
use sha2::Digest;
use ic_cdk::{
    api::{
        id,
        time,
        trap,
        call::{
            CallResult,
            RejectionCode,
        },
    },
    export::{
        Principal
    }
};

use crate::{
    ICP_DEFAULT_SUBACCOUNT,
    USERS_DATA,
    UserLock,
    UserData,
    IcpId,
    IcpIdSub,
    IcpTokens,

};




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
    Principal::from_slice(&bytes[1..1 + bytes[0] as usize])
} 



pub fn principal_icp_subaccount(user: &Principal) -> IcpIdSub {
    let mut sub_bytes = [0u8; 32];
    sub_bytes[..30].copy_from_slice(&principal_as_thirty_bytes(user));
    IcpIdSub(sub_bytes)
}



pub fn user_icp_balance_id(user: &Principal) -> IcpId {
    IcpId::new(&id(), &principal_icp_subaccount(user))
}


pub fn user_cycles_balance_topup_memo_bytes(user: &Principal) -> [u8; 32] {
    let mut memo_bytes = [0u8; 32];
    memo_bytes[..2].copy_from_slice(b"TP");
    memo_bytes[2..].copy_from_slice(&principal_as_thirty_bytes(user));
    memo_bytes
}


pub async fn check_user_icp_balance(user: &Principal) -> CallResult<IcpTokens> {
    use ic_ledger_types::{account_balance, AccountBalanceArgs, MAINNET_LEDGER_CANISTER_ID};
    let mut icp_balance: IcpTokens = account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        AccountBalanceArgs { account: user_icp_balance_id(user) }    
    ).await?;
    icp_balance -= USERS_DATA.with(|ud| { ud.borrow_mut().entry(*user).or_default().untaken_icp_to_collect });
    Ok(icp_balance)
}


pub fn check_user_cycles_balance(user: &Principal) -> u128 {
    USERS_DATA.with(|ud| {
        match ud.borrow().get(user) {
            Some(user_data) => user_data.cycles_balance,
            None            => 0                               
        }
    })
}


pub fn main_cts_icp_id() -> IcpId {  // do once
    IcpId::new(&id(), &ICP_DEFAULT_SUBACCOUNT)
}


pub fn check_lock_and_lock_user(user: &Principal) {
    USERS_DATA.with(|ud| {
        let users_data: &mut HashMap<Principal, UserData> = &mut ud.borrow_mut();
        let user_lock: &mut UserLock = &mut users_data.entry(*user).or_default().user_lock;
        let current_time: u64 = time();
        if user_lock.lock == true && current_time - user_lock.last_lock_time_nanos < 3*60*1_000_000_000 {
            trap("this user is in the middle of a different call");
        }
        user_lock.lock = true;
        user_lock.last_lock_time_nanos = current_time;
    });
}

pub fn unlock_user(user: &Principal) {
    USERS_DATA.with(|ud| {
        ud.borrow_mut().entry(*user).or_default().user_lock.lock = false;
    });
}


pub const DEFAULT_CYCLES_PER_XDR: u128 = 1_000_000_000_000u128; // 1T cycles = 1 XDR


pub fn icptokens_to_cycles(icpts: IcpTokens, xdr_permyriad_per_icp: u64) -> u128 {
    icpts.e8s() as u128 
    * xdr_permyriad_per_icp as u128 
    * DEFAULT_CYCLES_PER_XDR 
    / (IcpTokens::SUBDIVIDABLE_BY as u128 * 10_000)
}