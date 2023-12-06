use crate::{
    types::{Cycles, CallError},
    tools::call_error_as_u32_and_string,
    ic_cdk::{
        api::{
            call::{
                call_with_payment128,
                call,
            },
        },
    }
};
use serde::Serialize;
use candid::{
    Principal,
    CandidType,
    Deserialize,
};


#[derive(CandidType, Deserialize)]
pub struct ManagementCanisterInstallCodeQuest<'a> {
    pub mode : ManagementCanisterInstallCodeMode,
    pub canister_id : Principal,
    #[serde(with = "serde_bytes")]
    pub wasm_module : &'a [u8],
    #[serde(with = "serde_bytes")]
    pub arg : &'a [u8],
}

pub use ManagementCanisterInstallCodeQuest as InstallCodeQuest;

#[allow(non_camel_case_types)]
#[derive(CandidType, Deserialize)]
pub enum ManagementCanisterInstallCodeMode {
    install, 
    reinstall, 
    upgrade
}

pub use ManagementCanisterInstallCodeMode as InstallCodeMode;

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

#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ManagementCanisterCanisterSettings {
    pub controllers : Vec<Principal>,
    pub compute_allocation : u128,
    pub memory_allocation : u128,
    pub freezing_threshold : u128
}

#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct ManagementCanisterCanisterStatusRecord {
    pub status : ManagementCanisterCanisterStatusVariant,
    pub settings: ManagementCanisterCanisterSettings,
    pub module_hash: Option<[u8; 32]>,
    pub memory_size: u128,
    pub cycles: u128
}

#[allow(non_camel_case_types)]
#[derive(CandidType, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ManagementCanisterCanisterStatusVariant {
    running,
    stopping,
    stopped,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CanisterIdRecord {
    pub canister_id : Principal
}

#[derive(CandidType, Deserialize)]
pub struct ChangeCanisterSettingsRecord {
    pub canister_id : Principal,
    pub settings : ManagementCanisterOptionalCanisterSettings
}



pub async fn create_canister(create_canister_quest: ManagementCanisterCreateCanisterQuest, with_cycles: Cycles) -> Result<Principal, CallError> {
    match call_with_payment128::<(ManagementCanisterCreateCanisterQuest,), (CanisterIdRecord,)>(            
        Principal::management_canister(),
        "create_canister",
        (create_canister_quest,),
        with_cycles,
    ).await {
        Ok((canister_id_record,)) => Ok(canister_id_record.canister_id),
        Err(call_error) => Err(call_error_as_u32_and_string(call_error)),
    }
}
    
pub async fn install_code(install_code_quest: ManagementCanisterInstallCodeQuest<'_>) -> Result<(), CallError> {
    match call::<(ManagementCanisterInstallCodeQuest,), ()>(
        Principal::management_canister(),
        "install_code",
        (install_code_quest,)
    ).await {
        Ok(()) => Ok(()),
        Err(call_error) => Err(call_error_as_u32_and_string(call_error))
    }
}
