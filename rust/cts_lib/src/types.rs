use crate::ic_cdk::{
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






pub mod management_canister {
    use super::*;
    
    #[derive(CandidType, Deserialize)]
    pub struct ManagementCanisterInstallCodeQuest<'a> {
        pub mode : ManagementCanisterInstallCodeMode,
        pub canister_id : Principal,
        pub wasm_module : &'a [u8],
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

    #[derive(CandidType, Deserialize)]
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











pub mod user_canister {
    use super::*;

    #[derive(CandidType, Deserialize)]
    pub struct UserCanisterInit {
        pub user_id: UserId,
        pub users_map_canister_id: UsersMapCanisterId,
        pub cts_id: Principal,
    }
    
    #[derive(CandidType, Deserialize)]
    pub struct CyclesTransferIntoUser {
        pub canister: Principal,
        pub cycles: Cycles,
        pub timestamp_nanos: u64
    }

        
}






pub mod users_map_canister {
    use super::*;

    #[derive(CandidType, Deserialize)]
    pub struct UsersMapCanisterInit {
        pub cts_id: Principal
    }

    #[derive(CandidType,Deserialize)]
    pub enum PutNewUserError {
        CanisterIsFull,
        FoundUser(UserCanisterId)
    }



}











