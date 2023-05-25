use crate::{
    ic_cdk::{
        api::{
            time,
            call::{
                CallResult
            },
        },
        export::{
            Principal,
            candid::{
                CandidType,
                Deserialize,
            }
        }
    }
};




pub type Cycles = u128;
pub type CyclesTransferRefund = Cycles;
pub type CTSFuel = Cycles;
pub type XdrPerMyriadPerIcp = u64;


pub type CallError = (u32, String);


#[derive(CandidType, Deserialize)]
pub struct DownloadRChunkQuest {
    pub chunk_size: u64,
    pub chunk_i: u64,
    pub opt_height: Option<u64>,
}

#[derive(CandidType)]
pub struct RChunkData<'a, T: 'a> {
    pub latest_height: u64,
    pub data: Option<&'a [T]>
}





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






pub mod cycles_banks_cache {
    use super::{Principal, time};
    use std::collections::{HashMap};
    
    // private
    #[derive(Clone, Copy)]
    struct CBCacheData {
        timestamp_nanos: u64,
        opt_cycles_bank_canister_id: Option<Principal>
    }

    // cacha for this. with a max users->user-canisters
    // on a new user, put/update insert the new user into this cache
    // on a user-contract-termination, void[remove/delete] the (user,user-canister)-log in this cache
        
    pub struct CBSCache {
        hashmap: HashMap<Principal, CBCacheData>,
        max_size: usize
    }
    impl CBSCache {
        
        pub fn new(max_size: usize) -> Self {
            Self {
                hashmap: HashMap::new(),
                max_size
            }
        }
        
        pub fn put(&mut self, user_id: Principal, opt_cycles_bank_canister_id: Option<Principal>) {
            if self.hashmap.len() >= self.max_size {
                self.hashmap.remove(
                    &(self.hashmap.iter().min_by_key(
                        |(_user_id, user_cache_data)| {
                            user_cache_data.timestamp_nanos
                        }
                    ).unwrap().0.clone())
                );
            }
            self.hashmap.insert(user_id, CBCacheData{ opt_cycles_bank_canister_id, timestamp_nanos: time() });
        }
        
        pub fn check(&mut self, user_id: Principal) -> Option<Option<Principal>> {
            match self.hashmap.get_mut(&user_id) {
                None => None,
                Some(user_cache_data) => {
                    user_cache_data.timestamp_nanos = time();
                    Some(user_cache_data.opt_cycles_bank_canister_id)
                }
            }
        }
    }

}



pub mod cts {
    use super::*;
    
    #[derive(CandidType, Deserialize)]
    pub struct CyclesBankLifetimeTerminationQuest {
        pub user_id: Principal,
        pub cycles_balance: Cycles
    }
    
}




pub mod cbs_map {
    use super::*;

    #[derive(CandidType, Deserialize)]
    pub struct CBSMInit {
        pub cts_id: Principal
    }

    #[derive(CandidType, Deserialize, Clone)]    
    pub struct CBSMUserData {
        pub cycles_bank_canister_id: Principal,
        pub cycles_bank_latest_known_module_hash: [u8; 32],
        pub cycles_bank_lifetime_termination_timestamp_seconds: u128
    }

    #[derive(CandidType,Deserialize)]
    pub enum PutNewUserError {
        CanisterIsFull,
        FoundUser(CBSMUserData)
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
        pub cycles_transferrer_canisters: Vec<Principal>
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





