use crate::{
    ic_cdk::{
        api::{
            time,
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
        pub fn new(m: Vec<u8>) -> Self {
            Self {
                module_hash: crate::tools::sha256(&m),
                module: m
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
        pub fn verify_module_hash(&self) -> Result<(), ()> {
            if *(self.module_hash()) != crate::tools::sha256(self.module()) {
                Err(())
            } else {
                Ok(())
            }
        }
    }
}
pub use canister_code::CanisterCode;





pub mod cache {
    use super::{time};
    use std::collections::{HashMap};
    
    // private
    #[derive(Clone, Copy, candid::CandidType, serde::Serialize, serde::Deserialize)]
    struct CacheData<T> {
        timestamp_nanos: u64,
        data: T,
    }

    // cacha for this. with a max users->user-canisters
    // on a new user, put/update insert the new user into this cache
    // on a user-contract-termination, void[remove/delete] the (user,user-canister)-log in this cache
    use core::hash::Hash;
    #[derive(candid::CandidType, serde::Serialize, serde::Deserialize)]
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
    
    #[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]    
    pub struct OldCBSMUserData {
        pub cycles_bank_canister_id: Principal,
        pub first_membership_creation_timestamp_nanos: u128,
        pub cycles_bank_latest_known_module_hash: [u8; 32],
        pub cycles_bank_lifetime_termination_timestamp_seconds: u128,
        pub membership_termination_cb_uninstall_data: Option<CyclesBankTerminationUninstallData>,
    }
    
    #[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]    
    pub struct CBSMUserData {
        pub cycles_bank_canister_id: Principal,
        pub first_membership_creation_timestamp_nanos: u128,
        pub cycles_bank_latest_known_module_hash: [u8; 32],
        pub cycles_bank_lifetime_termination_timestamp_seconds: u128,
        pub membership_termination_cb_uninstall_data: Option<CyclesBankTerminationUninstallData>, // some if canister is uninstalled
        pub sns_control: bool, // true if this cb is control by a sns.
    }
    
    // None means values are kept as they are. Some means change the field-values.
    #[derive(CandidType, serde::Serialize, Deserialize, Clone, Debug, Default)]    
    pub struct CBSMUserDataUpdateFields {
        pub cycles_bank_latest_known_module_hash: Option<[u8; 32]>,
        pub cycles_bank_lifetime_termination_timestamp_seconds: Option<u128>,
        pub membership_termination_cb_uninstall_data: Option<Option<CyclesBankTerminationUninstallData>>,
    }
    
    #[derive(CandidType, serde::Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub struct CyclesBankTerminationUninstallData {
        pub uninstall_timestamp_nanos: u64,
        pub user_cycles_balance: Cycles,
    }

    #[derive(CandidType,Deserialize, Debug)]
    pub enum PutNewUserError {
        CanisterIsFull,
        FoundUser(CBSMUserData)
    }
    
    #[derive(CandidType, Deserialize, Debug)]
    pub enum UpdateUserError {
        UserNotFound
    }
    
    pub type UpdateUserResult = Result<(), UpdateUserError>;

}




pub mod cycles_bank {
    use super::*;
    use super::cycles_market::cm_main::TradeContractIdAndLedgerId;
    use crate::cmc::*; 
    use crate::ic_ledger_types::IcpTokens;
    
    #[derive(CandidType, Deserialize)]
    pub struct CyclesBankInit {
        pub user_id: Principal,
        pub cts_id: Principal,
        pub cbsm_id: Principal,
        pub storage_size_mib: u128,                         
        pub lifetime_termination_timestamp_seconds: u128,
        pub start_with_user_cycles_balance: Cycles,
        pub sns_control: bool,
    }
    
    #[derive(CandidType, Deserialize, Debug, PartialEq, Eq)]
    pub struct UserCBMetrics {
        pub cycles_balance: Cycles,
        pub ctsfuel_balance: CTSFuel,
        pub storage_size_mib: u128,
        pub lifetime_termination_timestamp_seconds: u128,
        pub user_id: Principal,
        pub user_canister_creation_timestamp_nanos: u128,
        pub storage_usage: u128,
        pub cycles_transfers_id_counter: u128,
        pub cycles_transfers_in_len: u128,
        pub cycles_transfers_out_len: u128,
        pub cm_trade_contracts: Vec<TradeContractIdAndLedgerId>,   
        pub cts_cb_authorization: bool, 
        pub cbsm_id: Principal,
        pub sns_control: bool,
    }    
    

    #[derive(CandidType, Deserialize, Debug)]
    pub enum UserIsInTheMiddleOfADifferentCall {
        BurnIcpMintCyclesCall{ must_call_complete: bool },
    }
    
    // burn icp mint cycles
    #[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
    pub struct BurnIcpMintCyclesQuest {
        pub burn_icp: IcpTokens, 
        pub burn_icp_transfer_fee: IcpTokens,   
    }
    
    #[derive(CandidType, Deserialize, Debug)]
    pub enum BurnIcpMintCyclesError {
        UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
        LedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
        LedgerTopupCyclesCmcNotifyRefund{ block_index: u64, reason: String},
        MidCallError(BurnIcpMintCyclesMidCallError)
    }
    
    #[derive(CandidType, Deserialize, Debug)]
    pub enum BurnIcpMintCyclesMidCallError {
        LedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
    }
    
    #[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
    pub struct BurnIcpMintCyclesSuccess {
        pub mint_cycles: Cycles
    }
    
    pub type BurnIcpMintCyclesResult = Result<BurnIcpMintCyclesSuccess, BurnIcpMintCyclesError>;
    
    // cm_methods
    #[derive(CandidType, Deserialize, Debug)]
    pub enum CBTradeCyclesError {
        MemoryIsFull,
        CyclesBalanceTooLow{ cycles_balance: Cycles },
        CMTradeCyclesCallError((u32, String)),
        CMTradeCyclesCallSponseCandidDecodeError{candid_error: String, sponse_bytes: Vec<u8> },
    }
    
    pub type CBTradeCyclesResult = Result<cm::tc::BuyTokensResult, CBTradeCyclesError>;
    
    #[derive(CandidType, Deserialize, Debug)]
    pub enum CBTradeTokensError {
        MemoryIsFull,
        CMTradeTokensCallError(CallError),
        CMTradeTokensCallSponseCandidDecodeError{candid_error: String, sponse_bytes: Vec<u8> },
    }
    
    pub type CBTradeTokensResult = Result<cm::tc::SellTokensResult, CBTradeTokensError>;

}





pub mod cycles_market;
pub use cycles_market as cm;



pub mod http_request{
    use super::*;
    use serde_bytes::ByteBuf;
    use candid::Nat;
    
    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct HttpRequest {
        pub method: String,
        pub url: String,
        pub headers: Vec<(String, String)>,
        #[serde(with = "serde_bytes")]
        pub body: Vec<u8>,
    }
    
    #[derive(Clone, Debug, CandidType)]
    pub struct HttpResponse<'a> {
        pub status_code: u16,
        pub headers: Vec<(&'a str, &'a str)>,
        pub body: &'a ByteBuf,
        pub streaming_strategy: Option<StreamStrategy<'a>>,
    }
    

    candid::define_function!(pub StreamCallback : (StreamCallbackTokenBackwards) -> (StreamCallbackHttpResponse) query);
    
    #[derive(Clone, Debug, CandidType)]
    pub enum StreamStrategy<'a> {
        Callback { callback: StreamCallback, token: StreamCallbackToken<'a>},
    }
    
    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct StreamCallbackToken<'a> {
        pub key: &'a str,
        pub content_encoding: &'a str,
        pub index: Nat,
        // We don't care about the sha, we just want to be backward compatible.
        pub sha256: Option<[u8; 32]>,
    }
    
    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct StreamCallbackTokenBackwards {
        pub key: String,
        pub content_encoding: String,
        pub index: Nat,
        // We don't care about the sha, we just want to be backward compatible.
        pub sha256: Option<[u8; 32]>,
    }
    
    #[derive(Clone, Debug, CandidType)]
    pub struct StreamCallbackHttpResponse<'a> {
        pub body: &'a ByteBuf,
        pub token: Option<StreamCallbackToken<'a>>,
    }

}




