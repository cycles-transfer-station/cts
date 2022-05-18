use crate::*;
use crate::ic_ledger_types::*;
use serde::Serialize;

use candid::{
    Principal,
    CandidType,
    Deserialize,
    
};

use ic_cdk::api::time;




pub type Cycles = u128;





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





// when collecting the untaken_icp_to_collect, collect the user_data.untaken_icp_to_collect - LEDGER_TRANSFER_FEE

#[derive(CandidType, Deserialize, Copy, Clone, serde::Serialize)]
pub struct UserData {
    pub cycles_balance: Cycles,
    pub untaken_icp_to_collect: IcpTokens,
    pub user_canister: Option<Principal>,
}

impl UserData {
    pub fn new() -> Self {
        Self {
            cycles_balance: 0u128,
            untaken_icp_to_collect: IcpTokens::ZERO,
            user_canister: None
        }
    }
    
    
    /*
    pub const SERIALIZE_SIZE: usize = 50; // variable_size? cycles_transfer_purchases_max_len

    pub fn serialize_forward(&self) -> [u8; Self::SERIALIZE_SIZE] {
        let mut b: [u8; Self::SERIALIZE_SIZE] = [0; Self::SERIALIZE_SIZE];
        // 9
        b[..9].copy_from_slice(self.user_lock.serialize());        
        // 16
        b[9..25].copy_from_slice(self.cycles_balance.to_be_bytes());
        // 8
        b[25..33].copy_from_slice(self.untaken_icp_to_collect.e8s().to_be_bytes());
        // 
        b[33..35].copy_from_slice(&(self.cycles_transfer_purchases.len() as u16).to_be_bytes());
        for i in 0..self.cycles_transfer_purchases.len() {
            b[
                35+CyclesTransferPurchaseLog::SIZE*i
                ..
                35+CyclesTransferPurchaseLog::SIZE*i+CyclesTransferPurchaseLog::SIZE
            ].copy_from_slice(self.cycles_transfer_purchases[i].serialize());
        }
        let cbps_start: usize = 35+self.cycles_transfer_purchases_max_len*CyclesTransferPurchaseLog::SIZE;
        b[cbps_start .. cbps_start+2].copy_from_slice(&(self.cycles_bank_purchases.len() as u16).to_be_bytes());
        for i in 0..self.cycles_bank_purchases.len() {
            b[
                cbps_start+2+CyclesBankPurchaseLog::SIZE*i
                ..
                cbps_start+2+CyclesBankPurchaseLog::SIZE*i+CyclesBankPurchaseLog::SIZE
            ].copy_from_slice(self.cycles_bank_purchases[i].serialize());
        }
        
        b

    }

    // pub fn serialize_backward(&self, &[u8]) -> Self {

    // }
    */

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
    pub cycles_sent: u128,
    pub cycles_accepted: u128, // 64?
    pub cycles_transfer: CyclesTransfer,
    pub timestamp: u64,
}

#[derive(CandidType, Deserialize, Copy, Clone, serde::Serialize)]
pub struct CyclesBankPurchaseLog {
    pub cycles_bank_principal: Principal,
    pub cost_cycles: u128,
    pub timestamp: u64,
    // module_hash?
}



#[derive(CandidType, Deserialize)]
pub struct UserCanisterInit {
    pub user: Principal,
    pub callers_whitelist: Vec<Principal>,
}
