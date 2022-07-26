use crate::{
    ic_cdk::{
        api::{
            time,
            call::{
                RejectionCode
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
pub type UserId = Principal;
pub type UserCanisterId = Principal;
pub type UsersMapCanisterId = Principal;











#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub enum CyclesTransferMemo {
    Nat(u128),
    Text(String),
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







pub mod canister_code {
    use super::{CandidType, Deserialize};
    
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

    #[derive(CandidType, Deserialize, Clone)]
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




pub mod cycles_transferrer {
    use super::*;
    
    pub type CyclesTransferRefund = Cycles;
    
    #[derive(CandidType, Deserialize)]
    pub struct CyclesTransferrerCanisterInit {
        pub cts_id: Principal
    }
    
    #[derive(CandidType, Deserialize, Clone)]
    pub struct CTSUserTransferCyclesQuest {
        pub users_map_canister_id: UsersMapCanisterId,
        pub umc_user_transfer_cycles_quest: cts::UMCUserTransferCyclesQuest
    }
    
    #[derive(CandidType, Deserialize)]
    pub enum CTSUserTransferCyclesError {
        MaxOngoingCyclesTransfers(u64),
        CyclesTransferQuestCandidCodeError(String)
    }
    
    pub type ReTryCyclesTransferrerUserTransferCyclesCallback = ((u32, String)/*the call-error of the last try*/, cts::CyclesTransferrerUserTransferCyclesCallbackQuest, CyclesTransferRefund);
    
}




pub mod cts {
    use super::*;
    
    #[derive(CandidType, Deserialize, Clone)]
    pub struct UMCUserTransferCyclesQuest {
        pub user_canister_id: UserCanisterId,
        pub uc_user_transfer_cycles_quest: users_map_canister::UCUserTransferCyclesQuest,
    }
    
    #[derive(CandidType, Deserialize)]
    pub enum UMCUserTransferCyclesError {
        MaxReTryCtsUserTransferCyclesCallbacks(usize),
        NoCyclesTransferrerCanistersFound,
        CTSUserTransferCyclesError(cycles_transferrer::CTSUserTransferCyclesError),
        CTSUserTransferCyclesCallError(String),
        CTSUserTransferCyclesUnknownError,
    }
    
    #[derive(CandidType, Deserialize, Clone)]
    pub struct CyclesTransferrerUserTransferCyclesCallbackQuest {
        pub cycles_transfer_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)>, // None means callstatus == 'replied'
        pub cts_user_transfer_cycles_quest: cycles_transferrer::CTSUserTransferCyclesQuest
    }
    
}







pub mod users_map_canister {
    use super::*;

    #[derive(CandidType, Deserialize)]
    pub struct UsersMapCanisterInit {
        pub cts_id: Principal
    }

    #[derive(CandidType, Deserialize, Clone)]    
    pub struct UMCUserData {
        pub user_canister_id: UserCanisterId,
        pub user_canister_latest_known_module_hash: [u8; 32],
    }

    #[derive(CandidType,Deserialize)]
    pub enum PutNewUserError {
        CanisterIsFull,
        FoundUser(UMCUserData)
    }

    #[derive(CandidType, Deserialize, Clone)]
    pub struct UCUserTransferCyclesQuest {
        pub user_id: UserId,
        pub cycles_transfer_purchase_log_id: user_canister::CyclesTransferPurchaseLogId,
        pub user_transfer_cycles_quest: user_canister::UserTransferCyclesQuest,
    }
    
    #[derive(CandidType, Deserialize)]
    pub enum UCUserTransferCyclesError {
        UMCUserTransferCyclesError(cts::UMCUserTransferCyclesError),
        UMCUserTransferCyclesCallError(String),
        
    }
    /*
    #[derive(CandidType, Deserialize)]
    pub struct CTSUpgradeUsersCanistersQuest {
        new_user_canister_wasm_module: Vec<u8>,
        users_
    }
    */
    
    pub type UMCUpgradeUCError = (UserCanisterId, UMCUpgradeUCCallErrorType, (u32, String));

    #[derive(CandidType, Deserialize, Clone, Debug)]
    pub enum UMCUpgradeUCCallErrorType {
        StopCanisterCallError,
        UpgradeCodeCallError{wasm_module_hash: [u8; 32]},
        StartCanisterCallError
    }
    




}






pub mod user_canister {
    use super::*;

    #[derive(CandidType, Deserialize)]
    pub struct UserCanisterInit {
        pub user_id: UserId,
        pub umc_id: UsersMapCanisterId,
        pub cts_id: Principal,
        
        pub user_canister_storage_plan: 
    }
    
    #[derive(CandidType, Deserialize, Clone, Copy)]
    pub struct CTSCyclesTransferIntoUser {
        pub canister: Principal,
        pub cycles: Cycles,
        pub timestamp_nanos: u64
    }
    
    #[derive(CandidType, Deserialize, Clone)]
    pub struct UserTransferCyclesQuest {
        pub cycles: Cycles,
        pub canister_id: Principal,
        pub cycles_transfer_memo: CyclesTransferMemo
    }
    
    pub type CyclesTransferPurchaseLogId = u64;
    
    #[derive(CandidType, Deserialize, Clone)]
    pub struct CTSUserTransferCyclesCallbackQuest {
        pub user_id: UserId,
        pub cycles_transfer_purchase_log_id: CyclesTransferPurchaseLogId,
        pub cycles_transfer_refund: Cycles,
        pub cycles_transfer_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)> // None means callstatus == 'replied'
    }
    
    #[derive(CandidType, Deserialize, Clone)]
    pub enum CTSUserTransferCyclesCallbackError {
        WrongUserId,
    }
}






