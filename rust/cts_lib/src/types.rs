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
                self,
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
    use super::{candid, CandidType, Deserialize};
    
    #[derive(CandidType, Deserialize, Clone)]
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





pub mod management_canister {
    use super::*;
    
    #[derive(CandidType, Deserialize)]
    pub struct ManagementCanisterInstallCodeQuest<'a> {
        pub mode : ManagementCanisterInstallCodeMode,
        pub canister_id : Principal,
        #[serde(with = "serde_bytes")]
        pub wasm_module : &'a [u8],
        #[serde(with = "serde_bytes")]
        pub arg : &'a [u8],
    }

    #[allow(non_camel_case_types)]
    #[derive(CandidType, Deserialize)]
    pub enum ManagementCanisterInstallCodeMode {
        install, 
        reinstall, 
        upgrade
    }
    
    #[derive(CandidType, Deserialize)]
    pub struct ManagementCanisterCreateCanisterQuest {
        pub settings : Option<ManagementCanisterOptionalCanisterSettings>
    }

    #[derive(CandidType, Deserialize, Clone)]
    pub struct ManagementCanisterOptionalCanisterSettings {
        pub controllers : Option<Vec<Principal>>,
        pub compute_allocation : Option<u128>,
        pub memory_allocation : Option<u128>,
        pub freezing_threshold : Option<u128>,
    }

    #[derive(CandidType, Deserialize, Clone, PartialEq, Eq)]
    pub struct ManagementCanisterCanisterSettings {
        pub controllers : Vec<Principal>,
        pub compute_allocation : u128,
        pub memory_allocation : u128,
        pub freezing_threshold : u128
    }

    #[derive(CandidType, Deserialize, Clone)]
    pub struct ManagementCanisterCanisterStatusRecord {
        pub status : ManagementCanisterCanisterStatusVariant,
        pub settings: ManagementCanisterCanisterSettings,
        pub module_hash: Option<[u8; 32]>,
        pub memory_size: u128,
        pub cycles: u128
    }

    #[allow(non_camel_case_types)]
    #[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
    pub enum ManagementCanisterCanisterStatusVariant {
        running,
        stopping,
        stopped,
    }

    #[derive(CandidType, Deserialize)]
    pub struct CanisterIdRecord {
        pub canister_id : Principal
    }

    #[derive(CandidType, Deserialize)]
    pub struct ChangeCanisterSettingsRecord {
        pub canister_id : Principal,
        pub settings : ManagementCanisterOptionalCanisterSettings
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
        pub cycles_market_id: Principal, 
        pub cycles_market_cmcaller: Principal,
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
    use super::{Principal, CyclesTransferMemo, Cycles, candid, CandidType, Deserialize};
    
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



pub mod cycles_market {
    use super::{CandidType, Deserialize, Cycles, XdrPerMyriadPerIcp};
    use crate::ic_ledger_types::{IcpTokens, IcpBlockHeight, IcpTransferError, IcpId};
    use ic_cdk::export::Principal;
    
    pub type PositionId = u128;
    pub type PurchaseId = u128;

    #[derive(CandidType, Deserialize)]
    pub struct CreateCyclesPositionQuest {
        pub cycles: Cycles,
        pub minimum_purchase: Cycles,
        pub xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    }

    #[derive(CandidType, Deserialize)]
    pub enum CreateCyclesPositionError{
        MinimumPurchaseMustBeEqualOrLessThanTheCyclesPosition,
        MsgCyclesTooLow{ create_position_fee: Cycles },
        CyclesMustBeAMultipleOfTheXdrPerMyriadPerIcpRate,
        MinimumPurchaseMustBeAMultipleOfTheXdrPerMyriadPerIcpRate,
        CyclesMarketIsBusy,
        CyclesMarketIsFull,
        CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: XdrPerMyriadPerIcp, minimum_cycles_position_for_a_bump: Cycles },
        MinimumCyclesPosition(Cycles),
        MinimumPurchaseCannotBeZero
    }

    #[derive(CandidType, Deserialize)]
    pub struct CreateCyclesPositionSuccess {
        pub position_id: PositionId,
    }
    
    pub type CreateCyclesPositionResult = Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>;

    #[derive(CandidType, Deserialize)]
    pub struct CreateIcpPositionQuest {
        pub icp: IcpTokens,
        pub minimum_purchase: IcpTokens,
        pub xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    }

    #[derive(CandidType, Deserialize)]
    pub enum CreateIcpPositionError {
        MinimumPurchaseMustBeEqualOrLessThanTheIcpPosition,
        MsgCyclesTooLow{ create_position_fee: Cycles },
        CyclesMarketIsFull,
        CyclesMarketIsBusy,
        CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
        CheckUserCyclesMarketIcpLedgerBalanceError((u32, String)),
        UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
        CyclesMarketIsFull_MaximumRateAndMinimumIcpPositionForABump{ maximum_rate_for_a_bump: XdrPerMyriadPerIcp, minimum_icp_position_for_a_bump: IcpTokens },
        MinimumIcpPosition(IcpTokens),
        MinimumPurchaseCannotBeZero
    }

    #[derive(CandidType, Deserialize)]
    pub struct CreateIcpPositionSuccess {
        pub position_id: PositionId
    }
        
    pub type CreateIcpPositionResult = Result<CreateIcpPositionSuccess, CreateIcpPositionError>;

    #[derive(CandidType, Deserialize)]
    pub struct PurchaseCyclesPositionQuest {
        pub cycles_position_id: PositionId,
        pub cycles: Cycles
    }

    #[derive(CandidType, Deserialize)]
    pub enum PurchaseCyclesPositionError {
        MsgCyclesTooLow{ purchase_position_fee: Cycles },
        CyclesMarketIsBusy,
        CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
        CheckUserCyclesMarketIcpLedgerBalanceError((u32, String)),
        UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
        CyclesPositionNotFound,
        CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: Cycles },
        CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: Cycles },
        PurchaseCyclesMustBeAMultipleOfTheXdrPerMyriadPerIcpRate,
    }

    #[derive(CandidType, Deserialize)]
    pub struct PurchaseCyclesPositionSuccess {
        pub purchase_id: PurchaseId,
    }

    pub type PurchaseCyclesPositionResult = Result<PurchaseCyclesPositionSuccess, PurchaseCyclesPositionError>;

    #[derive(CandidType, Deserialize)]
    pub struct PurchaseIcpPositionQuest {
        pub icp_position_id: PositionId,
        pub icp: IcpTokens
    }

    #[derive(CandidType, Deserialize)]
    pub enum PurchaseIcpPositionError {
        MsgCyclesTooLow{ purchase_position_fee: Cycles },
        CyclesMarketIsBusy,
        IcpPositionNotFound,
        IcpPositionIcpIsLessThanThePurchaseQuest{ icp_position_icp: IcpTokens },
        IcpPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ icp_position_minimum_purchase: IcpTokens }
    }

    #[derive(CandidType, Deserialize)]
    pub struct PurchaseIcpPositionSuccess {
        pub purchase_id: PurchaseId
    }

    pub type PurchaseIcpPositionResult = Result<PurchaseIcpPositionSuccess, PurchaseIcpPositionError>;

    #[derive(CandidType, Deserialize)]
    pub struct VoidPositionQuest {
        pub position_id: PositionId
    }

    #[derive(CandidType, Deserialize)]
    pub enum VoidPositionError {
        WrongCaller,
        MinimumWaitTime{ minimum_wait_time_seconds: u128, position_creation_timestamp_seconds: u128 },
        CyclesMarketIsBusy,
        PositionNotFound,
    }

    pub type VoidPositionResult = Result<(), VoidPositionError>;

    #[derive(CandidType, Deserialize)]
    pub struct TransferIcpBalanceQuest {
        pub icp: IcpTokens,
        pub icp_fee: IcpTokens,
        pub to: IcpId
    }

    #[derive(CandidType, Deserialize)]
    pub enum TransferIcpBalanceError {
        MsgCyclesTooLow{ transfer_icp_balance_fee: Cycles },
        CyclesMarketIsBusy,
        CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
        CheckUserCyclesMarketIcpLedgerBalanceCallError((u32, String)),
        UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
        IcpTransferCallError((u32, String)),
        IcpTransferError(IcpTransferError)
    }

    pub type TransferIcpBalanceResult = Result<IcpBlockHeight, TransferIcpBalanceError>;

    #[derive(CandidType, Deserialize)]
    pub struct CMVoidCyclesPositionPositorMessageQuest {
        pub position_id: PositionId,
        // cycles in the call
        pub timestamp_nanos: u128
    }

    #[derive(CandidType, Deserialize)]
    pub struct CMVoidIcpPositionPositorMessageQuest {
        pub position_id: PositionId,
        pub void_icp: IcpTokens,
        pub timestamp_nanos: u128
    }

    #[derive(CandidType, Deserialize)]
    pub struct CMCyclesPositionPurchasePositorMessageQuest {
        pub cycles_position_id: PositionId,
        pub purchase_id: PurchaseId,
        pub purchaser: Principal,
        pub purchase_timestamp_nanos: u128,
        pub cycles_purchase: Cycles,
        pub cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
        pub icp_payment: IcpTokens,
        pub icp_transfer_block_height: IcpBlockHeight,
        pub icp_transfer_timestamp_nanos: u128,
    }
    
    #[derive(CandidType, Deserialize)]
    pub struct CMCyclesPositionPurchasePurchaserMessageQuest {
        pub cycles_position_id: PositionId,
        pub cycles_position_positor: Principal,
        pub cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
        pub purchase_id: PurchaseId,
        pub purchase_timestamp_nanos: u128,
        // cycles in the call
        pub icp_payment: IcpTokens,
    }

    #[derive(CandidType, Deserialize)]
    pub struct CMIcpPositionPurchasePositorMessageQuest {
        pub icp_position_id: PositionId,
        pub icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
        pub purchaser: Principal,
        pub purchase_id: PurchaseId,
        pub icp_purchase: IcpTokens,
        pub purchase_timestamp_nanos: u128,
        // cycles in the call
    }
    
    #[derive(CandidType, Deserialize)]
    pub struct CMIcpPositionPurchasePurchaserMessageQuest {
        pub icp_position_id: PositionId,
        pub purchase_id: PurchaseId, 
        pub positor: Principal,
        pub purchase_timestamp_nanos: u128,
        pub cycles_payment: Cycles,
        pub icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
        pub icp_purchase: IcpTokens,
        pub icp_transfer_block_height: IcpBlockHeight,
        pub icp_transfer_timestamp_nanos: u128,
    }
    


}




pub mod cm_caller {
    use super::{Principal, Cycles, CandidType, Deserialize};
    
    #[derive(CandidType, Deserialize)]
    pub struct CMCallerInit {
        pub cycles_market_id: Principal,
        pub cts_id: Principal
    }

    #[derive(CandidType, Deserialize)]    
    pub struct CMCallQuest{
        pub cm_call_id: u128,
        pub for_the_canister: Principal,
        pub method: String,
        #[serde(with = "serde_bytes")]
        pub put_bytes: Vec<u8>,
        pub cycles: Cycles,
        pub cm_callback_method: String,
    }

    #[derive(CandidType, Deserialize)]
    pub enum CMCallError {
        MaxCalls,
    }
    
    pub type CMCallResult = Result<(), CMCallError>;

    #[derive(CandidType, Deserialize, Clone)]
    pub struct CMCallbackQuest {
        pub cm_call_id: u128,
        pub opt_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)> // None means callstatus == 'replied'
        // sponse_bytes? do i care? CallResult
    }

}



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





pub mod icrc1 {
    use super::{Principal, CallResult, CandidType, Deserialize};

    #[derive(CandidType, Deserialize)]
    pub struct Account {
        owner: Principal,
        subaccount: Option<[u8; 32]>
    }
    
    #[derive(CandidType, Deserialize)]
    pub struct TransferArgs {
        from_subaccount : Option<[u8; 32]>,
        to : Account,
        amount : u128,
        fee : Option<u128>,
        memo : Option<Vec<u8>>,
        created_at_time : Option<u64>,
    }

    #[derive(CandidType, Deserialize)]
    pub enum TransferError {
        BadFee{ expected_fee : u128 },
        BadBurn{ min_burn_amount : u128 },
        InsufficientFunds{ balance : u128 },
        TooOld,
        CreatedInFuture{ ledger_time: u64 },
        Duplicate{ duplicate_of : u128 },
        TemporarilyUnavailable,
        GenericError{ error_code : u128, message : String },
    }


}

