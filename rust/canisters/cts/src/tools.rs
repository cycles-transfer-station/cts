use crate::{
    CTS_DATA,
    LATEST_KNOWN_CMC_RATE,
    MAX_CBS_MAPS,
};
//use candid::{CandidType,Deserialize};
use cts_lib::{
    types::{
        Cycles,
        cbs_map::{
            CBSMInit,
            CBSMUserData,
            PutNewUserError as CBSMPutNewUserError
        },
    },
     management_canister::{
        ManagementCanisterInstallCodeMode,
        ManagementCanisterInstallCodeQuest,
        ManagementCanisterCreateCanisterQuest,
        ManagementCanisterOptionalCanisterSettings,
        ManagementCanisterCanisterStatusRecord,
        ManagementCanisterCanisterStatusVariant,
        CanisterIdRecord,
        ChangeCanisterSettingsRecord,    
    },
    consts::{
        MiB,
        MANAGEMENT_CANISTER_ID,
        ICP_LEDGER_TOP_UP_CANISTER_MEMO,
        NETWORK_CANISTER_CREATION_FEE_CYCLES
        
    },
    tools::{
        localkey::{
            refcell::{with, with_mut},
        },
        user_icp_id,
        principal_icp_subaccount,
    },
    ic_cdk::{
        api::{
            id,
            time,
            call::{
                CallResult,
                call_raw128,
                call,
                call_with_payment128,
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
use candid::{
    Principal,
    CandidType,
    Deserialize,
    utils::{encode_one, decode_one},
};








pub async fn check_user_icp_ledger_balance(user_id: &Principal) -> CallResult<IcpTokens> {
    icp_account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs { account: user_icp_id(&id(), user_id) }    
    ).await
}




pub async fn transfer_user_icp_ledger(user_id: &Principal, icp: IcpTokens, icp_fee: IcpTokens, memo: IcpMemo) -> CallResult<IcpTransferResult> {
    icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: memo,
            amount: icp,
            fee: icp_fee,
            from_subaccount: Some(principal_icp_subaccount(user_id)),
            to: main_cts_icp_id(),
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time()-1_000_000_000 })
        }
    ).await
}





pub fn main_cts_icp_id() -> IcpId {  // do once
    IcpId::new(&id(), &ICP_DEFAULT_SUBACCOUNT)
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
    
    if with(&CTS_DATA, |cts_data| { cts_data.canisters_for_the_use.len() }) >= 1 {
        let new_canister: Principal = with_mut(&CTS_DATA, |cts_data| { cts_data.canisters_for_the_use.take(&(cts_data.canisters_for_the_use.iter().next().unwrap().clone())).unwrap() }); 
     
        match set_canister(new_canister, optional_canister_settings.clone(), with_cycles).await {
            Ok(canister_id) => return Ok(canister_id),
            Err(_set_canister_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.canisters_for_the_use.insert(new_canister); });
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









#[derive(CandidType, Deserialize)]
pub enum PutNewUserIntoACBSMError {
    CBSMPutNewUserCallFail(Principal, String), // principal of the failiing users-map-canister
    CBSMPutNewUserError(CBSMPutNewUserError),
    CreateNewCBSMError(CreateNewCBSMError),
    
}

// this function as of now does not check if the user exists already in one of the users-map-canisters. use the find_user-function for that.
pub async fn put_new_user_into_a_cbsm(user_id: Principal, cbsm_user_data: CBSMUserData) -> Result<Principal, PutNewUserIntoACBSMError> {
    
    for i in (0..with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.len() })).rev() {
        let cbsm_id:Principal = with(&CTS_DATA, |cts_data| { cts_data.cbs_maps[i] });
        match call::<(Principal, CBSMUserData), (Result<(), CBSMPutNewUserError>,)>(
            cbsm_id,
            "put_new_user",
            (user_id, cbsm_user_data.clone()),
        ).await {
            Ok((cbsm_put_new_user_sponse,)) => match cbsm_put_new_user_sponse {
                Ok(()) => return Ok(cbsm_id),
                Err(cbsm_put_new_user_error) => match cbsm_put_new_user_error {
                    CBSMPutNewUserError::CanisterIsFull => continue,
                    _ => return Err(PutNewUserIntoACBSMError::CBSMPutNewUserError(cbsm_put_new_user_error)) /*error can be that the user is already in the canister. the new_user function should have locked the user and checked for the user first before calling this function.  */
                }
            },
            Err(cbsm_put_new_user_call_fail) => return Err(PutNewUserIntoACBSMError::CBSMPutNewUserCallFail(cbsm_id, format!("{:?}", cbsm_put_new_user_call_fail))),
        }
    }
    
    // if each cbs_map is full,
    // create a new cbs_map
    let cbsm_id: Principal = match create_new_cbs_map().await {
        Ok(new_cbs_map_canister_id) => new_cbs_map_canister_id,
        Err(create_new_cbs_map_error) => return Err(PutNewUserIntoACBSMError::CreateNewCBSMError(create_new_cbs_map_error))
    };
    
    match call::<(Principal, CBSMUserData), (Result<(), CBSMPutNewUserError>,)>(
        cbsm_id,
        "put_new_user",
        (user_id, cbsm_user_data),
    ).await {
        Ok((cbsm_put_new_user_sponse,)) => match cbsm_put_new_user_sponse {
            Ok(()) => return Ok(cbsm_id),
            Err(cbsm_put_new_user_error) => return Err(PutNewUserIntoACBSMError::CBSMPutNewUserError(cbsm_put_new_user_error))
        },
        Err(cbsm_put_new_user_call_fail) => return Err(PutNewUserIntoACBSMError::CBSMPutNewUserCallFail(cbsm_id, format!("{:?}", cbsm_put_new_user_call_fail))),
    }
    

}



#[derive(CandidType, Deserialize)]
pub enum CreateNewCBSMError {
    MaxCBSMapCanisters,
    CreateNewCBSMapLockIsOn,
    GetNewCanisterError(GetNewCanisterError),
    CBSMapCanisterCodeNotFound,
    InstallCodeCallError(String)
}

pub async fn create_new_cbs_map() -> Result<Principal, CreateNewCBSMError> {
    if with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.len() }) >= MAX_CBS_MAPS {
        return Err(CreateNewCBSMError::MaxCBSMapCanisters);
    }
    
    if with(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock }) == true {
        return Err(CreateNewCBSMError::CreateNewCBSMapLockIsOn);
    }
    with_mut(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock = true; });
    
    let new_cbs_map_canister_id: Principal = match get_new_canister(
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
            with_mut(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock = false; });
            return Err(CreateNewCBSMError::GetNewCanisterError(get_new_canister_error));
        }
    };    
    
    // install code
    if with(&CTS_DATA, |cts_data| cts_data.cbs_map_canister_code.module().len() ) == 0 {
        with_mut(&CTS_DATA, |cts_data| { cts_data.canisters_for_the_use.insert(new_cbs_map_canister_id); });
        with_mut(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock = false; });
        return Err(CreateNewCBSMError::CBSMapCanisterCodeNotFound);
    }
    
    match call::<(ManagementCanisterInstallCodeQuest,), ()>(
        MANAGEMENT_CANISTER_ID,
        "install_code",
        (ManagementCanisterInstallCodeQuest{
            mode : ManagementCanisterInstallCodeMode::install,
            canister_id : new_cbs_map_canister_id,
            wasm_module : unsafe{&*with(&CTS_DATA, |cts_data| { cts_data.cbs_map_canister_code.module() as *const Vec<u8> })},
            arg : &encode_one(&CBSMInit{
                cts_id: id()
            }).unwrap() // unwrap or return Err(candiderror); 
        },)
    ).await {
        Ok(_) => {
            with_mut(&CTS_DATA, |cts_data| { cts_data.cbs_maps.push(new_cbs_map_canister_id); }); 
            with_mut(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock = false; });
            Ok(new_cbs_map_canister_id)    
        },
        Err(install_code_call_error) => {
            with_mut(&CTS_DATA, |cts_data| { cts_data.canisters_for_the_use.insert(new_cbs_map_canister_id); });
            with_mut(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock = false; });
            return Err(CreateNewCBSMError::InstallCodeCallError(format!("{:?}", install_code_call_error)));
        }
    }
}







#[derive(CandidType, Deserialize)]
pub enum FindUserInTheCBSMapsError {
    CBSMapsFindUserCallFails(Vec<(Principal, (u32, String))>)
}

pub async fn find_user_in_the_cbs_maps(user_id: Principal) -> Result<Option<(CBSMUserData, Principal)>, FindUserInTheCBSMapsError> {
    
    let call_results: Vec<CallResult<(Option<CBSMUserData>,)>> = futures::future::join_all(
        with(&CTS_DATA, |cts_data| { 
            cts_data.cbs_maps.iter().map(
                |cbsm| { 
                    call::<(Principal,), (Option<CBSMUserData>,)>(
                        cbsm.clone(), 
                        "find_user", 
                        (user_id,)
                    )
                }
            ).collect::<Vec<_>>()
        })
    ).await;
    
    let mut call_fails: Vec<(Principal, (u32, String))> = Vec::new();
    
    for (i,call_result) in call_results.into_iter().enumerate() {
        let cbsm_id: Principal = with(&CTS_DATA, |cts_data| cts_data.cbs_maps[i]);
        match call_result {
            Ok((optional_cbsm_user_data,)) => match optional_cbsm_user_data {
                Some(cbsm_user_data) => return Ok(Some((cbsm_user_data, cbsm_id))),
                None => continue
            },
            Err(find_user_call_error) => call_fails.push((cbsm_id, (find_user_call_error.0 as u32, find_user_call_error.1)))
        }
    }
    
    match call_fails.len() {
        0 => Ok(None),
        _ => Err(FindUserInTheCBSMapsError::CBSMapsFindUserCallFails(call_fails))
    }

}











