use std::collections::HashMap;
// use sha2::Digest;

use crate::{
    NEW_CANISTERS,
    LATEST_KNOWN_CMC_RATE,
    MAX_USERS_MAP_CANISTERS,
    USERS_MAP_CANISTERS,
    CREATE_NEW_USERS_MAP_CANISTER_LOCK,
    USERS_MAP_CANISTER_CODE,
};
//use candid::{CandidType,Deserialize};
use cts_lib::{
    types::{
        UserLock,
        canister_code::CanisterCode,
        Cycles,
        users_map_canister::{
            UsersMapCanisterInit,
            UMCUserData,
            PutNewUserError as UsersMapCanisterPutNewUserError
        },
        UserId,
        UserCanisterId,
        UsersMapCanisterId,
        management_canister::{
            ManagementCanisterInstallCodeMode,
            ManagementCanisterInstallCodeQuest,
            ManagementCanisterCreateCanisterQuest,
            ManagementCanisterCanisterSettings,
            ManagementCanisterOptionalCanisterSettings,
            ManagementCanisterCanisterStatusRecord,
            ManagementCanisterCanisterStatusVariant,
            CanisterIdRecord,
            ChangeCanisterSettingsRecord,
            
        }
    },
    consts::{
        MANAGEMENT_CANISTER_ID,
        ICP_LEDGER_CREATE_CANISTER_MEMO,
        ICP_LEDGER_TOP_UP_CANISTER_MEMO,
        ICP_CTS_TAKE_FEE_MEMO,
        NETWORK_CANISTER_CREATION_FEE_CYCLES
        
    },
    tools::{
        sha256,
        localkey::{
            self,
            refcell::{with, with_mut},
            cell::{},
        },
        user_icp_id,
        principal_icp_subaccount,
        principal_as_thirty_bytes,
        icptokens_to_cycles,
        cycles_to_icptokens,
    },
    ic_cdk::{
        api::{
            id,
            time,
            trap,
            call::{
                CallResult,
                call_raw128,
                call,
                call_with_payment128,
                RejectionCode,
            },
        },
        export::{
            Principal,
            candid::{
                CandidType,
                Deserialize,
                utils::{encode_one, decode_one},
            },
        }
    },
    ic_ledger_types::{
        IcpId,
        IcpIdSub,
        IcpTokens,
        IcpAccountBalanceArgs,
        IcpBlockHeight,
        IcpTimestamp,
        IcpTransferArgs,
        icp_transfer,
        IcpTransferResult,
        IcpTransferError,
        IcpMemo,
        icp_account_balance,
        ICP_DEFAULT_SUBACCOUNT,
        MAINNET_LEDGER_CANISTER_ID,
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        ICP_LEDGER_TRANSFER_DEFAULT_FEE
    }
};








pub async fn check_user_icp_ledger_balance(user_id: &Principal) -> CallResult<IcpTokens> {
    icp_account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs { account: user_icp_id(&id(), user_id) }    
    ).await
}




pub async fn take_user_icp_ledger(user_id: &Principal, icp: IcpTokens) -> CallResult<IcpTransferResult> {
    icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: ICP_FEE_MEMO,
            amount: icp,
            fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            from_subaccount: Some(principal_icp_subaccount(user_id)),
            to: main_cts_icp_id(),
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
        }
    ).await
}



pub fn user_cycles_balance_topup_memo_bytes(user: &Principal) -> [u8; 32] {
    let mut memo_bytes = [0u8; 32];
    memo_bytes[..2].copy_from_slice(USER_CYCLES_BALANCE_TOPUP_MEMO_START);
    memo_bytes[2..].copy_from_slice(&principal_as_thirty_bytes(user));
    memo_bytes
}




pub fn main_cts_icp_id() -> IcpId {  // do once
    IcpId::new(&id(), &ICP_DEFAULT_SUBACCOUNT)
}








pub fn put_new_canister(put_new_canister: Principal) -> Result<(), ()> {
    with_mut(&NEW_CANISTERS, |new_canisters| { 
        if new_canisters.contains(&put_new_canister) {
            return Err(());
        }
        new_canisters.push_back(put_new_canister);  
        Ok(())
    })
}







 

#[derive(CandidType, Deserialize)]
pub enum CheckCurrentXdrPerMyriadPerIcpCmcRateError {
    CmcGetRateCallError((u32, String)),
    CmcGetRateCallSponseCandidError(String),
}

#[derive(CandidType, Deserialize)]
struct IcpXdrConversionRateCertifiedResponse {
    certificate: Vec<u8>, 
    data : IcpXdrConversionRate,
    hash_tree : Vec<u8>
}

#[derive(CandidType, Deserialize, Copy, Clone)]
pub struct IcpXdrConversionRate {
    /// Number of 1/10,000ths of XDR that 1 ICP is worth.
    pub xdr_permyriad_per_icp : u64,
    pub timestamp_seconds : u64
}

pub type CheckCurrentXdrPerMyriadPerIcpCmcRateSponse = Result<u64, CheckCurrentXdrPerMyriadPerIcpCmcRateError>;

// how many 1/10000-xdr for one icp
pub async fn check_current_xdr_permyriad_per_icp_cmc_rate() -> CheckCurrentXdrPerMyriadPerIcpCmcRateSponse {

    let latest_known_cmc_rate: IcpXdrConversionRate = LATEST_KNOWN_CMC_RATE.with(|r| { r.get() }); 
    if time() / 1_000_000_000 - latest_known_cmc_rate.timestamp_seconds < 60*10 {
        return Ok(latest_known_cmc_rate.xdr_permyriad_per_icp);
    }
    
    let call_sponse_candid_bytes: Vec<u8> = match call_raw128(
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        "get_icp_xdr_conversion_rate",
        &encode_one(()).unwrap(),
        0
    ).await {
        Ok(b) => b,
        Err(call_error) => return Err(CheckCurrentXdrPerMyriadPerIcpCmcRateError::CmcGetRateCallError((call_error.0 as u32, call_error.1)))
    };
    let icp_xdr_conversion_rate: IcpXdrConversionRate = match decode_one::<IcpXdrConversionRateCertifiedResponse>(&call_sponse_candid_bytes) {
        Ok(s) => s.data,
        Err(candid_error) => return Err(CheckCurrentXdrPerMyriadPerIcpCmcRateError::CmcGetRateCallSponseCandidError(format!("{}", candid_error))),
    };

    LATEST_KNOWN_CMC_RATE.with(|r| { r.set(icp_xdr_conversion_rate); });
    Ok(icp_xdr_conversion_rate.xdr_permyriad_per_icp)
}












enum SetCanisterError {
    CanisterStatusCallError((u32, String)),
    UninstallCanisterCallError((u32, String)),
    StartCanisterCallError((u32, String)),
    UpdateSettingsCallError((u32, String)),    
    PositCyclesCallError((u32, String)),
}

async fn set_canister(canister_id: Principal, optional_canister_settings: Option<ManagementCanisterOptionalCanisterSettings>, with_cycles: Cycles) -> Result<Principal, SetCanisterError> {
    // get status
    let canister_status_record: ManagementCanisterCanisterStatusRecord = match call::<(CanisterIdRecord,), (ManagementCanisterCanisterStatusRecord,)>(
        MANAGEMENT_CANISTER_ID,
        "canister_status",
        (CanisterIdRecord { canister_id: canister_id },),
    ).await {
        Ok((canister_status_record,)) => canister_status_record,
        Err(canister_status_call_error) => return Err(SetCanisterError::CanisterStatusCallError((canister_status_call_error.0 as u32, canister_status_call_error.1)))
    };
    
    // make sure is empty
    if canister_status_record.module_hash.is_some() {
        match call::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "uninstall_code",
            (CanisterIdRecord{ canister_id: canister_id },)
        ).await {
            Ok(()) => {},
            Err(uninstall_canister_call_error) => return Err(SetCanisterError::UninstallCanisterCallError((uninstall_canister_call_error.0 as u32, uninstall_canister_call_error.1)))
        }
    }
    
    // make sure is running 
    if canister_status_record.status != ManagementCanisterCanisterStatusVariant::running {
        match call::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "start_canister",
            (CanisterIdRecord{ canister_id: canister_id },)
        ).await {
            Ok(()) => {},
            Err(start_canister_call_error) => return Err(SetCanisterError::StartCanisterCallError((start_canister_call_error.0 as u32, start_canister_call_error.1)))
        }
    
    }
    
    // update settings if different
    let mut settings: ManagementCanisterOptionalCanisterSettings = ManagementCanisterOptionalCanisterSettings{
        controllers : Some(vec![id()]),
        compute_allocation : Some(0),
        memory_allocation : Some(0),
        freezing_threshold : Some(2592000), //(30 days).
    };
    
    if let Some(canister_settings) = optional_canister_settings {
        if let Some(controllers) = canister_settings.controllers {
            settings.controllers = Some(controllers);
        }
        if let Some(compute_allocation) = canister_settings.compute_allocation {
            settings.compute_allocation = Some(compute_allocation);
        }
        if let Some(memory_allocation) = canister_settings.memory_allocation {
            settings.memory_allocation = Some(memory_allocation);
        }
        if let Some(freezing_threshold) = canister_settings.freezing_threshold {
            settings.freezing_threshold = Some(freezing_threshold);
        }
    }
    match call::<(ChangeCanisterSettingsRecord,), ()>(
        MANAGEMENT_CANISTER_ID,
        "update_settings",
        (ChangeCanisterSettingsRecord{
            canister_id,
            settings
        },)
    ).await {
        Ok(()) => {},
        Err(update_settings_call_error) => return Err(SetCanisterError::UpdateSettingsCallError((update_settings_call_error.0 as u32, update_settings_call_error.1)))
    }
    
    
    // put cycles (or take cycles? later.) if not enough
    if canister_status_record.cycles < with_cycles {
        match call_with_payment128::<>(
            MANAGEMENT_CANISTER_ID,
            "deposit_cycles",
            (CanisterIdRecord{ canister_id: canister_id },),
            with_cycles - canister_status_record.cycles
        ).await {
            Ok(()) => {},
            Err(posit_cycles_call_error) => return Err(SetCanisterError::PositCyclesCallError((posit_cycles_call_error.0 as u32, posit_cycles_call_error.1)))
        }
    }
    
    Ok(canister_id)
}





#[derive(CandidType, Deserialize)]
pub enum GetNewCanisterError {
    CreateCanisterManagementCallQuestCandidError(String),
    CreateCanisterManagementCallSponseCandidError{candid_error: String, candid_bytes: Vec<u8>},
    CreateCanisterManagementCallError((u32, String))
}

pub async fn get_new_canister(optional_canister_settings: Option<ManagementCanisterOptionalCanisterSettings>, with_cycles: Cycles) -> Result<Principal, GetNewCanisterError> {
    
    if let Some(new_canister) = with_mut(&NEW_CANISTERS, |new_canisters| { new_canisters.pop_front() }) {
        match set_canister(new_canister, optional_canister_settings.clone(), with_cycles).await {
            Ok(canister_id) => return Ok(canister_id),
            Err(set_canister_error) => {
                put_new_canister(new_canister);
                // continue
            }
        }
    }

    let create_canister_management_call_quest_candid_bytes: Vec<u8> = match encode_one(&ManagementCanisterCreateCanisterQuest { settings: optional_canister_settings }) {
        Ok(candid_bytes) => candid_bytes,
        Err(candid_error) => {
            return Err(GetNewCanisterError::CreateCanisterManagementCallQuestCandidError(format!("{}", candid_error)));
        }
    };

    let canister_id: Principal = match call_raw128(
        MANAGEMENT_CANISTER_ID,
        "create_canister",
        &create_canister_management_call_quest_candid_bytes,
        NETWORK_CANISTER_CREATION_FEE_CYCLES + with_cycles
    ).await {
        Ok(call_sponse_candid_bytes) => match decode_one::<CanisterIdRecord>(&call_sponse_candid_bytes) {
            Ok(canister_id_record) => canister_id_record.canister_id,
            Err(candid_error) => {
                return Err(GetNewCanisterError::CreateCanisterManagementCallSponseCandidError{ candid_error: format!("{}", candid_error), candid_bytes: call_sponse_candid_bytes });
            }
        },
        Err(call_error) => {
            return Err(GetNewCanisterError::CreateCanisterManagementCallError((call_error.0 as u32, call_error.1)));
        }
    };
    
    // are new canisters running?

    Ok(canister_id)

}












#[derive(CandidType, Deserialize)]
pub struct CmcNotifyCreateCanisterQuest {
    pub block_index: IcpBlockHeight,
    pub controller: Principal,
}


#[derive(CandidType, Deserialize)]
struct CmcNotifyTopUpCyclesQuest {
    block_index: IcpBlockHeight,
    canister_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub enum CmcNotifyError {
    Refunded { block_index: Option<IcpBlockHeight>, reason: String },
    InvalidTransaction(String),
    Other{ error_message: String, error_code: u64 },
    Processing,
    TransactionTooOld(IcpBlockHeight),
}

type NotifyTopUpResult = Result<Cycles, CmcNotifyError>;









#[derive(CandidType, Deserialize)]
pub enum LedgerTopupCyclesCmcIcpTransferError {
    IcpTransferCallError((u32, String)),
    IcpTransferError(IcpTransferError),
}



// make a public method to re-try a block-height
pub async fn ledger_topup_cycles_cmc_icp_transfer(icp: IcpTokens, from_subaccount: Option<IcpIdSub>, topup_canister: Principal) -> Result<IcpBlockHeight, LedgerTopupCyclesCmcIcpTransferError> {

    let cmc_icp_transfer_block_height: IcpBlockHeight = match icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: ICP_LEDGER_TOP_UP_CANISTER_MEMO,
            amount: icp,                              
            fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            from_subaccount: from_subaccount,
            to: IcpId::new(&MAINNET_CYCLES_MINTING_CANISTER_ID, &principal_icp_subaccount(&topup_canister)),
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
        }
    ).await {
        Ok(transfer_call_sponse) => match transfer_call_sponse {
            Ok(block_index) => block_index,
            Err(transfer_error) => {
                return Err(LedgerTopupCyclesCmcIcpTransferError::IcpTransferError(transfer_error));
            }
        },
        Err(transfer_call_error) => {
            return Err(LedgerTopupCyclesCmcIcpTransferError::IcpTransferCallError((transfer_call_error.0 as u32, transfer_call_error.1)));
        }
    };
    
    Ok(cmc_icp_transfer_block_height)
}


#[derive(CandidType, Deserialize)]
pub enum LedgerTopupCyclesCmcNotifyError {
    CmcNotifyTopUpQuestCandidEncodeError(String),
    CmcNotifyCallError((u32, String)),
    CmcNotifySponseCandidDecodeError{candid_error: String, candid_bytes: Vec<u8>},
    CmcNotifyError(CmcNotifyError),
}

pub async fn ledger_topup_cycles_cmc_notify(cmc_icp_transfer_block_height: IcpBlockHeight, topup_canister_id: Principal) -> Result<Cycles, LedgerTopupCyclesCmcNotifyError> {

    let topup_cycles_cmc_notify_call_candid: Vec<u8> = match encode_one(
        & CmcNotifyTopUpCyclesQuest {
            block_index: cmc_icp_transfer_block_height,
            canister_id: topup_canister_id
        }
    ) {
        Ok(b) => b,
        Err(candid_error) => {
            return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifyTopUpQuestCandidEncodeError(format!("{}", candid_error)));
        }
    };

    let cycles: Cycles = match call_raw128(
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        "notify_top_up",
        &topup_cycles_cmc_notify_call_candid,
        0
    ).await {
        Ok(candid_bytes) => match decode_one::<NotifyTopUpResult>(&candid_bytes) {
            Ok(notify_topup_result) => match notify_topup_result {
                Ok(cycles) => cycles,
                Err(cmc_notify_error) => {
                    return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifyError(cmc_notify_error));
                }
            },
            Err(candid_error) => {
                return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifySponseCandidDecodeError{candid_error: format!("{}", candid_error), candid_bytes: candid_bytes});
            }
        },
        Err(notify_call_error) => {
            return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifyCallError((notify_call_error.0 as u32, notify_call_error.1)));
        }
    };

    Ok(cycles)
}




/*
#[derive(CandidType, Deserialize)]
pub enum LedgerCreateCanisterError {
    IcpTransferCallError(String),
    IcpTransferError(IcpTransferError),
    CmcNotifyCallError{call_error: String, block_height: IcpBlockHeight},   //create_canister_icp_transfer_block_height    
    CmcNotifyError{error: CmcNotifyError, block_height: IcpBlockHeight}
}

pub async fn ledger_create_canister(icp: IcpTokens, from_subaccount: Option<IcpIdSub>, controller: Principal) -> Result<Principal, LedgerCreateCanisterError> {

}
*/










#[derive(CandidType, Deserialize)]
pub enum PutNewUserIntoAUsersMapCanisterError {
    UsersMapCanisterPutNewUserCallFail(UsersMapCanisterId, String), // principal of the failiing users-map-canister
    UsersMapCanisterPutNewUserError(UsersMapCanisterPutNewUserError),
    CreateNewUsersMapCanisterError(CreateNewUsersMapCanisterError),
    
}

// this function as of now does not check if the user exists already in one of the users-map-canisters. use the find_user-function for that.
pub async fn put_new_user_into_a_users_map_canister(user_id: UserId, umc_user_data: UMCUserData) -> Result<UsersMapCanisterId, PutNewUserIntoAUsersMapCanisterError> {
    
    for i in (0..with(&USERS_MAP_CANISTERS, |umcs| umcs.len())).rev() {
        let umc_id:Principal = with(&USERS_MAP_CANISTERS, |umcs| umcs[i]);
        match call::<(UserId, UMCUserData), (Result<(), UsersMapCanisterPutNewUserError>,)>(
            umc_id,
            "put_new_user",
            (user_id, umc_user_data.clone()),
        ).await {
            Ok((users_map_canister_put_new_user_sponse,)) => match users_map_canister_put_new_user_sponse {
                Ok(()) => return Ok(umc_id),
                Err(users_map_canister_put_new_user_error) => match users_map_canister_put_new_user_error {
                    UsersMapCanisterPutNewUserError::CanisterIsFull => continue,
                    _ => return Err(PutNewUserIntoAUsersMapCanisterError::UsersMapCanisterPutNewUserError(users_map_canister_put_new_user_error)) /*error can be that the user is already in the canister. the new_user function should have locked the user and checked for the user first before calling this function.  */
                }
            },
            Err(users_map_canister_put_new_user_call_fail) => return Err(PutNewUserIntoAUsersMapCanisterError::UsersMapCanisterPutNewUserCallFail(umc_id, format!("{:?}", users_map_canister_put_new_user_call_fail))),
        }
    }
    
    // if each users_map_canister is full,
    // create a new users_map_canister
    let umc_id: UsersMapCanisterId = match create_new_users_map_canister().await {
        Ok(new_users_map_canister_id) => new_users_map_canister_id,
        Err(create_new_users_map_canister_error) => return Err(PutNewUserIntoAUsersMapCanisterError::CreateNewUsersMapCanisterError(create_new_users_map_canister_error))
    };
    
    match call::<(UserId, UMCUserData), (Result<(), UsersMapCanisterPutNewUserError>,)>(
        umc_id,
        "put_new_user",
        (user_id, umc_user_data),
    ).await {
        Ok((users_map_canister_put_new_user_sponse,)) => match users_map_canister_put_new_user_sponse {
            Ok(()) => return Ok(umc_id),
            Err(users_map_canister_put_new_user_error) => return Err(PutNewUserIntoAUsersMapCanisterError::UsersMapCanisterPutNewUserError(users_map_canister_put_new_user_error))
        },
        Err(users_map_canister_put_new_user_call_fail) => return Err(PutNewUserIntoAUsersMapCanisterError::UsersMapCanisterPutNewUserCallFail(umc_id, format!("{:?}", users_map_canister_put_new_user_call_fail))),
    }
    

}



#[derive(CandidType, Deserialize)]
pub enum CreateNewUsersMapCanisterError {
    MaxUsersMapCanisters,
    CreateNewUsersMapCanisterLockIsOn,
    GetNewCanisterError(GetNewCanisterError),
    UsersMapCanisterCodeNotFound,
    InstallCodeCallError(String)
}

pub async fn create_new_users_map_canister() -> Result<UsersMapCanisterId, CreateNewUsersMapCanisterError> {
    if with(&USERS_MAP_CANISTERS, |umcs| umcs.len()) >= MAX_USERS_MAP_CANISTERS {
        return Err(CreateNewUsersMapCanisterError::MaxUsersMapCanisters);
    }
    
    if localkey::cell::get(&CREATE_NEW_USERS_MAP_CANISTER_LOCK) == true {
        return Err(CreateNewUsersMapCanisterError::CreateNewUsersMapCanisterLockIsOn);
    }
    localkey::cell::set(&CREATE_NEW_USERS_MAP_CANISTER_LOCK, true);
    
    let new_users_map_canister_id: Principal = match get_new_canister(
        Some(ManagementCanisterOptionalCanisterSettings{
            controllers : None,
            compute_allocation : None,
            memory_allocation : Some(100 * MiB as u128),
            freezing_threshold : None,
        }),
        /*TEST-VALUE*/3_000_000_000_000  //7_000_000_000_000
    ).await {
        Ok(canister_id) => canister_id,
        Err(get_new_canister_error) => {
            CREATE_NEW_USERS_MAP_CANISTER_LOCK.with(|l| { l.set(false); });
            return Err(CreateNewUsersMapCanisterError::GetNewCanisterError(get_new_canister_error));
        }
    };    
    
    // install code
    if with(&USERS_MAP_CANISTER_CODE, |umcc| umcc.module().len() == 0 ) {
        put_new_canister(new_users_map_canister_id);
        CREATE_NEW_USERS_MAP_CANISTER_LOCK.with(|l| { l.set(false); });
        return Err(CreateNewUsersMapCanisterError::UsersMapCanisterCodeNotFound);
    }
    
    match call::<(ManagementCanisterInstallCodeQuest,), ()>(
        MANAGEMENT_CANISTER_ID,
        "install_code",
        (ManagementCanisterInstallCodeQuest{
            mode : ManagementCanisterInstallCodeMode::install,
            canister_id : new_users_map_canister_id,
            wasm_module : unsafe{&*with(&USERS_MAP_CANISTER_CODE, |umc_code| { umc_code.module() as *const Vec<u8> })},
            arg : &encode_one(&UsersMapCanisterInit{
                cts_id: id()
            }).unwrap() // unwrap or return Err(candiderror); 
        },)
    ).await {
        Ok(_) => {
            with_mut(&USERS_MAP_CANISTERS, |users_map_canisters| { users_map_canisters.push(new_users_map_canister_id); }); 
            CREATE_NEW_USERS_MAP_CANISTER_LOCK.with(|l| { l.set(false); });
            Ok(new_users_map_canister_id)    
        },
        Err(install_code_call_error) => {
            put_new_canister(new_users_map_canister_id);
            CREATE_NEW_USERS_MAP_CANISTER_LOCK.with(|l| { l.set(false); });
            return Err(CreateNewUsersMapCanisterError::InstallCodeCallError(format!("{:?}", install_code_call_error)));
        }
    }
}







#[derive(CandidType, Deserialize)]
pub enum FindUserInTheUsersMapCanistersError {
    UsersMapCanistersFindUserCallFails(Vec<(UsersMapCanisterId, (u32, String))>)
}

pub async fn find_user_in_the_users_map_canisters(user_id: UserId) -> Result<Option<(UMCUserData, UsersMapCanisterId)>, FindUserInTheUsersMapCanistersError> {
    
    let call_results: Vec<CallResult<(Option<UMCUserData>,)>> = with(&USERS_MAP_CANISTERS, |umcs| { futures::future::join_all(
        umcs.iter().map(|umc| { 
            call::<(UserId,), (Option<UMCUserData>,)>(
                *umc/*copy*/, 
                "find_user", 
                (user_id,)
            )
        })
    )}).await;
    
    let mut call_fails: Vec<(UsersMapCanisterId, (u32, String))> = Vec::new();
    
    for (i,call_result) in call_results.into_iter().enumerate() {
        let umc_id: UsersMapCanisterId = with(&USERS_MAP_CANISTERS, |umcs| umcs[i]);
        match call_result {
            Ok((optional_umc_user_data,)) => match optional_umc_user_data {
                Some(umc_user_data) => return Ok(Some((umc_user_data, umc_id))),
                None => continue
            },
            Err(find_user_call_error) => call_fails.push((umc_id, (find_user_call_error.0 as u32, find_user_call_error.1)))
        }
    }
    
    match call_fails.len() {
        0 => Ok(None),
        _ => Err(FindUserInTheUsersMapCanistersError::UsersMapCanistersFindUserCallFails(call_fails))
    }

}











