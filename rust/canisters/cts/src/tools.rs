use crate::{
    CTS_DATA,
    LATEST_KNOWN_CMC_RATE,
    MAX_CBS_MAPS,
    CREATE_CBS_MAP_CANISTER_CYCLES,
    CREATE_CBS_MAP_CANISTER_NETWORK_MEMORY_ALLOCATION,
};
//use candid::{CandidType,Deserialize};
pub use cts_lib::cmc::*;
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
        CanisterIdRecord,
    },
    consts::{
        MANAGEMENT_CANISTER_ID,
        NETWORK_CANISTER_CREATION_FEE_CYCLES,
        TRILLION,
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
            },
            canister_balance128,
        }
    },
    ic_ledger_types::{
        IcpId,
        IcpTokens,
        IcpAccountBalanceArgs,
        IcpTimestamp,
        IcpTransferArgs,
        icp_transfer,
        IcpTransferResult,
        IcpMemo,
        icp_account_balance,
        ICP_DEFAULT_SUBACCOUNT,
        MAINNET_LEDGER_CANISTER_ID,
        MAINNET_CYCLES_MINTING_CANISTER_ID,
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










 

#[derive(CandidType, Deserialize, Debug)]
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








#[derive(CandidType, Deserialize, Debug)]
pub enum CreateCanisterError {
    CreateCanisterManagementCallQuestCandidError(String),
    CreateCanisterManagementCallSponseCandidError{candid_error: String, candid_bytes: Vec<u8>},
    CreateCanisterManagementCallError((u32, String))
}

pub async fn create_canister(optional_canister_settings: Option<ManagementCanisterOptionalCanisterSettings>, with_cycles: Cycles) -> Result<Principal, CreateCanisterError> {
    
    let create_canister_management_call_quest_candid_bytes: Vec<u8> = match encode_one(&ManagementCanisterCreateCanisterQuest { settings: optional_canister_settings }) {
        Ok(candid_bytes) => candid_bytes,
        Err(candid_error) => {
            return Err(CreateCanisterError::CreateCanisterManagementCallQuestCandidError(format!("{}", candid_error)));
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
                return Err(CreateCanisterError::CreateCanisterManagementCallSponseCandidError{ candid_error: format!("{}", candid_error), candid_bytes: call_sponse_candid_bytes });
            }
        },
        Err(call_error) => {
            return Err(CreateCanisterError::CreateCanisterManagementCallError((call_error.0 as u32, call_error.1)));
        }
    };
    
    Ok(canister_id)

}







#[derive(CandidType, Deserialize, Debug)]
pub enum PutNewUserIntoACBSMError {
    CBSMPutNewUserCallFail(Principal, String), // principal of the failiing users-map-canister
    CBSMPutNewUserError(CBSMPutNewUserError),
    CreateNewCBSMError(CreateNewCBSMError),
    
}

// this function as of now does not check if the user exists already in one of the users-map-canisters. use the find_user-function for that.
pub async fn put_new_user_into_a_cbsm(user_id: Principal, cbsm_user_data: CBSMUserData) -> Result<Principal, PutNewUserIntoACBSMError> {
    
    for i in (0..with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.len() })).rev() {
        let cbsm_id:Principal = with(&CTS_DATA, |cts_data| { cts_data.cbs_maps[i].0 });
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



#[derive(CandidType, Deserialize, Debug)]
pub enum CreateNewCBSMError {
    MaxCBSMapCanisters,
    CreateNewCBSMapLockIsOn,
    CTSCyclesBalanceTooLow{ cycles_balance: Cycles },
    GetNewCanisterError(CreateCanisterError),
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
    let mut possible_c: Option<Principal> = with_mut(&CTS_DATA, |cts_data| { 
        cts_data.create_new_cbs_map_lock = true; 
        cts_data.temp_create_new_cbsmap_holder.take()     
    });
    
    if possible_c.is_none() { 
        if canister_balance128() < CREATE_CBS_MAP_CANISTER_CYCLES + 20*TRILLION {
            with_mut(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock = false; });
            return Err(CreateNewCBSMError::CTSCyclesBalanceTooLow{ cycles_balance: canister_balance128() });
        }
        possible_c = match create_canister(
            Some(ManagementCanisterOptionalCanisterSettings{
                controllers : None,
                compute_allocation : None,
                memory_allocation : Some(CREATE_CBS_MAP_CANISTER_NETWORK_MEMORY_ALLOCATION),
                freezing_threshold : None,
            }),
            CREATE_CBS_MAP_CANISTER_CYCLES
        ).await {
            Ok(canister_id) => Some(canister_id),
            Err(get_new_canister_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.create_new_cbs_map_lock = false; });
                return Err(CreateNewCBSMError::GetNewCanisterError(get_new_canister_error));
            }
        };    
    }
    
    let c: Principal = possible_c.unwrap();
    
    // install code
    if with(&CTS_DATA, |cts_data| cts_data.cbs_map_canister_code.module().len() ) == 0 {
        with_mut(&CTS_DATA, |cts_data| { 
            cts_data.temp_create_new_cbsmap_holder = Some(c);
            cts_data.create_new_cbs_map_lock = false; 
        });
        return Err(CreateNewCBSMError::CBSMapCanisterCodeNotFound);
    }
    
    let cbsm_module_hash: crate::ModuleHash = with(&CTS_DATA, |cts_data| { cts_data.cbs_map_canister_code.module_hash().clone() });
    
    match call::<(ManagementCanisterInstallCodeQuest,), ()>(
        MANAGEMENT_CANISTER_ID,
        "install_code",
        (ManagementCanisterInstallCodeQuest{
            mode : ManagementCanisterInstallCodeMode::install,
            canister_id : c,
            wasm_module : unsafe{&*with(&CTS_DATA, |cts_data| { cts_data.cbs_map_canister_code.module() as *const Vec<u8> })},
            arg : &encode_one(&CBSMInit{
                cts_id: id()
            }).unwrap() // unwrap or return Err(candiderror); 
        },)
    ).await {
        Ok(_) => {
            with_mut(&CTS_DATA, |cts_data| { 
                cts_data.cbs_maps.push((c, cbsm_module_hash)); 
                cts_data.create_new_cbs_map_lock = false; 
            });
            Ok(c)    
        },
        Err(install_code_call_error) => {
            with_mut(&CTS_DATA, |cts_data| { 
                cts_data.temp_create_new_cbsmap_holder = Some(c);
                cts_data.create_new_cbs_map_lock = false; 
            });
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
                |(cbsm, _)| { 
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
        let cbsm_id: Principal = with(&CTS_DATA, |cts_data| cts_data.cbs_maps[i].0);
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











