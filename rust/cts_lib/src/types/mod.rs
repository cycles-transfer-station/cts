use crate::{
    ic_cdk::{
        api::{
            time,
            call::{
                CallResult
            },
        },
    }
};

use candid::{
    CandidType,
    Deserialize,
    Principal,
};



//pub struct Cycles(pub u128);
pub type Cycles = u128;
//pub struct CyclesTransferRefund(pub Cycles);
pub type CyclesTransferRefund = Cycles;
//pub struct CTSFuel(pub Cycles);
pub type CTSFuel = Cycles;

pub type XdrPerMyriadPerIcp = u64;


pub type CallError = (u32, String);





#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub enum CyclesTransferMemo {
    Nat(u128),
    Int(i128),
    Text(String),
    Blob(Vec<u8>)   // with serde bytes
}

#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub struct CyclesTransfer {
    pub memo: CyclesTransferMemo
}






pub mod canister_code {
    use super::{CandidType, Deserialize};
    use serde::Serialize;
    
    #[derive(CandidType, Serialize, Deserialize, Clone)]
    pub struct CanisterCode {
        #[serde(with = "serde_bytes")]
        module: Vec<u8>,
        module_hash: [u8; 32]
    }

    impl CanisterCode {
        pub fn new(mut module: Vec<u8>) -> Self { // :mut for the shrink_to_fit
            module.shrink_to_fit();
            Self {
                module_hash: crate::tools::sha256(&module), // put this on the top if move error
                module: module,
            }
        }
        pub fn empty() -> Self {
            Self {
                module_hash: [0u8; 32],
                module: Vec::new()
            }
        }
        pub fn module(&self) -> &Vec<u8> {
            &self.module
        }
        pub fn module_hash(&self) -> &[u8; 32] {
            &self.module_hash
        }
        pub fn change_module(&mut self, module: Vec<u8>) {
            *self = Self::new(module);
        }
    }
}






pub mod cache {
    use super::{time};
    use std::collections::{HashMap};
    
    // private
    #[derive(Clone, Copy)]
    struct CacheData<T> {
        timestamp_nanos: u64,
        data: T,
    }

    // cacha for this. with a max users->user-canisters
    // on a new user, put/update insert the new user into this cache
    // on a user-contract-termination, void[remove/delete] the (user,user-canister)-log in this cache
    use core::hash::Hash;
    pub struct Cache<E: Eq + PartialEq + Hash + Clone, T> {
        hashmap: HashMap<E, CacheData<T>>,
        max_size: usize
    }
    impl<E: Eq + PartialEq + Hash + Clone, T> Cache<E, T> {
        
        pub fn new(max_size: usize) -> Self {
            Self {
                hashmap: HashMap::new(),
                max_size
            }
        }
        
        pub fn put(&mut self, key: E, v: T) {
            if self.hashmap.len() >= self.max_size {
                // file a bug report, if clone is not a trait bound of E, then the below code fails with a different error
                self.hashmap.remove(
                    &(self.hashmap.iter().min_by_key(
                        |(_key, cache_data)| {
                            cache_data.timestamp_nanos
                        }
                    ).unwrap().0.clone())
                );
            }
            self.hashmap.insert(key, CacheData{ data: v, timestamp_nanos: time() });
        }
        
        pub fn check(&mut self, key: &E) -> Option<&T> {
            match self.hashmap.get_mut(&key) {
                None => None,
                Some(cache_data) => {
                    cache_data.timestamp_nanos = time(); // keeps the most used items in the cache
                    Some(&(cache_data.data))
                }
            }
        }
    }

}



pub mod cts {
    use super::*;
    
    pub struct UserAndCB {
        pub user_id: Principal,
        pub cb_id: Principal
    }
    impl UserAndCB {
        pub fn create_cts_cb_authorization_msg(&self) -> Vec<u8> {
            let user_id_slice = self.user_id.as_slice();
            let cb_id_slice = self.cb_id.as_slice();
            let mut v: Vec<u8> = Vec::new();
            v.push(user_id_slice.len() as u8);
            v.extend(user_id_slice);
            v.push(cb_id_slice.len() as u8);
            v.extend(cb_id_slice);
            v
        }
    }
        
    
    #[derive(CandidType, Deserialize)]
    pub struct CyclesBankLifetimeTerminationQuest {
        pub user_id: Principal,
        pub cycles_balance: Cycles
    }

    #[derive(CandidType, serde::Serialize, Deserialize, Clone)]
    pub struct LengthenMembershipQuest {
        pub lengthen_years: u128,
    }
        
    
}




pub mod cbs_map {
    use super::*;

    #[derive(CandidType, Deserialize)]
    pub struct CBSMInit {
        pub cts_id: Principal
    }

    #[derive(CandidType, serde::Serialize, Deserialize, Clone)]    
    pub struct CBSMUserData {
        pub cycles_bank_canister_id: Principal,
        pub first_membership_creation_timestamp_nanos: u128,
        pub cycles_bank_latest_known_module_hash: [u8; 32],
        pub cycles_bank_lifetime_termination_timestamp_seconds: u128,
        pub membership_termination_cb_uninstall_data: Option<CyclesBankTerminationUninstallData> // some if canister is uninstalled
    }
    
    #[derive(CandidType, serde::Serialize, Deserialize, Clone)]
    pub struct CyclesBankTerminationUninstallData {
        pub uninstall_timestamp_nanos: u64,
        pub user_cycles_balance: Cycles,
    }

    #[derive(CandidType,Deserialize)]
    pub enum PutNewUserError {
        CanisterIsFull,
        FoundUser(CBSMUserData)
    }
    
    #[derive(CandidType, Deserialize)]
    pub enum UpdateUserError {
        UserNotFound
    }
    
    pub type CBSMUpgradeCBError = (Principal, CBSMUpgradeCBErrorKind);

    #[derive(CandidType, Deserialize, Clone, Debug)]
    pub enum CBSMUpgradeCBErrorKind {
        StopCanisterCallError(u32, String),
        UpgradeCodeCallError{wasm_module_hash: [u8; 32], call_error: (u32, String)},
        UpgradeCodeCallCandidError{candid_error: String},
        StartCanisterCallError(u32, String)
    }
    




}






pub mod cycles_bank {
    use super::*;

    #[derive(CandidType, Deserialize)]
    pub struct CyclesBankInit {
        pub user_id: Principal,
        pub cts_id: Principal,
        pub cbsm_id: Principal,
        pub storage_size_mib: u128,                         
        pub lifetime_termination_timestamp_seconds: u128,
        pub start_with_user_cycles_balance: Cycles,
    }
    
    #[derive(CandidType, Deserialize)]
    pub struct LengthenLifetimeQuest {
        pub set_lifetime_termination_timestamp_seconds: u128
    }
    
}




pub mod cycles_transferrer {
    use super::{Principal, CyclesTransferMemo, Cycles, CandidType, Deserialize};
    
    #[derive(CandidType, Deserialize)]
    pub struct CyclesTransferrerCanisterInit {
        pub cts_id: Principal
    }
    
    #[derive(CandidType, Deserialize)]
    pub struct CyclesTransfer {
        pub memo: CyclesTransferMemo,
        pub original_caller: Option<Principal>
    }
    
    #[derive(CandidType, Deserialize)]    
    pub struct TransferCyclesQuest{
        pub user_cycles_transfer_id: u128,
        pub for_the_canister: Principal,
        pub cycles: Cycles,
        pub cycles_transfer_memo: CyclesTransferMemo
    }
    
    #[derive(CandidType, Deserialize)]
    pub enum TransferCyclesError {
        MsgCyclesTooLow{ transfer_cycles_fee: Cycles },
        MaxOngoingCyclesTransfers,
        CyclesTransferQuestCandidCodeError(String)
    }
    
    #[derive(CandidType, Deserialize, Clone)]
    pub struct TransferCyclesCallbackQuest {
        pub user_cycles_transfer_id: u128,
        pub opt_cycles_transfer_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)> // None means callstatus == 'replied'
    }
    
}


pub mod cycles_market;


pub mod safe_caller {
    use super::{Principal, Cycles, CallResult, CandidType, Deserialize};
    
    #[derive(CandidType, Deserialize)]
    pub struct SafeCallerInit {
        pub cts_id: Principal
    }
        
    #[derive(CandidType, Deserialize)]    
    pub struct SafeCallQuest{
        pub call_id: u128,
        pub callee: Principal,
        pub method: String,
        pub data: Vec<u8>,
        pub cycles: Cycles,
        pub callback_method: String
    }
    
    #[derive(CandidType, Deserialize)]
    pub enum SafeCallError {
        MsgCyclesTooLow{ safe_call_fee: Cycles },
        SafeCallerIsBusy
    }
    
    #[derive(CandidType, Deserialize, Clone)]
    pub struct SafeCallCallbackQuest {
        pub call_id: u128,
        pub call_result: CallResult<Vec<u8>>,
    }
    
}





