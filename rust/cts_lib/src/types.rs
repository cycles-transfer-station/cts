use crate::ic_ledger_types::*;


use ic_cdk::{
    api::time,
    export::{
        Principal,
        candid::{
            CandidType,
            Deserialize,   
        }
    }
};




pub type Cycles = u128;
pub type UserId = Principal;
pub type UserCanisterId = Principal;
pub type UsersMapCanisterId = Principal;











#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub enum CyclesTransferMemo {
    Text(String),
    Nat64(u64),
    Blob(Vec<u8>)
}

#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub struct CyclesTransfer {
    pub memo: CyclesTransferMemo
}











// repr(packed) ?
#[derive(CandidType, Deserialize, Copy, Clone, serde::Serialize)]
pub struct UserLock {
    lock: bool,
    last_lock_time_nanos: u64 
}
impl UserLock {
    pub fn new() -> Self {
        Self {
            lock: false,
            last_lock_time_nanos: 0
        }
    }
    
    pub const MAX_LOCK_TIME_NANOS: u64 =  30 * 60 * 1_000_000_000;
    
    pub fn is_lock_on(&self) -> bool {
        self.lock && time() - self.last_lock_time_nanos <= Self::MAX_LOCK_TIME_NANOS
    }
    pub fn lock(&mut self) {
        self.lock = true;
        self.last_lock_time_nanos = time();
    }
    pub fn unlock(&mut self) {
        self.lock = false;
    }
    
    pub const SERIALIZE_SIZE: usize = 9;
    pub fn serialize(&self) -> [u8; Self::SERIALIZE_SIZE] {
        let mut b: [u8; Self::SERIALIZE_SIZE] = [0; Self::SERIALIZE_SIZE];
        b[0] = if self.lock { 1 } else { 0 };
        b[1..9].copy_from_slice(&self.last_lock_time_nanos.to_be_bytes());
        b
    }
    pub fn backwards(b: &[u8; Self::SERIALIZE_SIZE]) -> Result<Self, String> {
        Ok(Self {
            lock: match b[0] { 1 => true, 0 => false, _ => return Err("unknown lock byte".to_string()) },
            last_lock_time_nanos: u64::from_be_bytes((&b[1..9]).try_into().unwrap())
        })
    }
}





// for these two, make sure the fee for each purchase-type pays for the storage-cost of the Log for a certain amount of time, a year or 3 and then check the timestamp and delete expired ones or option to pay for longer storage
#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub struct CyclesTransferPurchaseLog {
    pub canister: Principal,
    pub cycles_sent: Cycles,
    pub cycles_accepted: Cycles,
    pub cycles_transfer_memo: CyclesTransferMemo,
    pub timestamp: u64,
}

#[derive(CandidType, Deserialize, Copy, Clone, serde::Serialize)]
pub struct CyclesBankPurchaseLog {
    pub cycles_bank_principal: Principal,
    pub cost_cycles: Cycles,
    pub timestamp: u64,
    // cycles-bank-module_hash?
}



#[derive(CandidType, Deserialize)]
pub struct UserCanisterInit {
    pub user_id: UserId,
    pub users_map_canister_id: UsersMapCanisterId,
    pub cts_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub struct UsersMapCanisterInit {
    pub cts_id: Principal
}
