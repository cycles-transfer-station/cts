use std::collections::HashMap;
// use sha2::Digest;
use ic_cdk::{
    api::{
        id,
        time,
        trap,
        call::{
            CallResult,
            call_raw128,
            call,
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
};

use crate::{
    USERS_DATA,
    NEW_CANISTERS,
    UserLock,
    UserData,
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
    MANAGEMENT_CANISTER_PRINCIPAL,
    Cycles,
    ICP_LEDGER_TRANSFER_DEFAULT_FEE,
    LatestKnownCmcRate,
    LATEST_KNOWN_CMC_RATE,
    with,
    with_mut,
    

    


};

use cts_lib::tools::sha256;






pub const CYCLES_BALANCE_TOPUP_MEMO_START: &'static [u8] = b"TP";

pub const ICP_TOP_UP_CANISTER_MEMO: IcpMemo = IcpMemo(0x50555054); // == 'TPUP'

pub const DEFAULT_CYCLES_PER_XDR: u128 = 1_000_000_000_000; // 1T cycles = 1 XDR





fn principal_as_thirty_bytes(p: &Principal) -> [u8; 30] {
    let mut bytes: [u8; 30] = [0; 30];
    let p_bytes: &[u8] = p.as_slice();
    bytes[0] = p_bytes.len() as u8; 
    bytes[1 .. p_bytes.len() + 1].copy_from_slice(p_bytes); 
    bytes
}

pub fn thirty_bytes_as_principal(bytes: &[u8; 30]) -> Principal {
    Principal::from_slice(&bytes[1..1 + bytes[0] as usize])
} 



pub fn principal_icp_subaccount(user: &Principal) -> IcpIdSub {
    let mut sub_bytes = [0u8; 32];
    sub_bytes[..30].copy_from_slice(&principal_as_thirty_bytes(user));
    IcpIdSub(sub_bytes)
}



pub fn user_icp_balance_id(user: &Principal) -> IcpId {
    IcpId::new(&id(), &principal_icp_subaccount(user))
}


pub fn check_user_icp_ledger_balance(user_id: &Principal) -> CallResult<IcpTokens> {
    icp_account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs { account: user_icp_balance_id(user_id) }    
    ).await
}

pub async fn check_user_icp_balance(user: &Principal) -> CallResult<IcpTokens> {
    let mut icp_balance: IcpTokens = check_user_icp_ledger_balance(user).await?;
    with(&USERS_DATA, |ud| { 
        if let Some(u) = ud.get(user) {
            *&mut icp_balance -= u.untaken_icp_to_collect;
        } 
    });
    //icp_balance -= USERS_DATA.with(|ud| { ud.borrow_mut().entry(*user).or_default().untaken_icp_to_collect });
    Ok(icp_balance)
}


pub async fn take_user_icp_ledger(user_id: &Principal, icp: IcpTokens) -> CallResult<IcpTransferResult> {
    icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: ICP_TAKE_FEE_MEMO,
            amount: icp,
            fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            from_subaccount: Some(principal_icp_subaccount(user_id)),
            to: main_cts_icp_id(),                        
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
        }
    ).await
}


#[derive(CandidType, Deserialize)]
pub enum FindUserError {
    
}

pub async fn find_user(user: &Principal, ) -> Result<UserData, FindUserError> {
    call(),
}


#[derive(CandidType, Deserialize)]
pub enum FindAndLockUserError {
    
}

pub type FindAndLockUserSponse = Result<(UserData, Principal), FindAndLockUserError>;

pub async fn find_and_lock_user(user: &Principal) -> FindAndLockUserSponse {
    call(),
}


pub fn user_cycles_balance_topup_memo_bytes(user: &Principal) -> [u8; 32] {
    let mut memo_bytes = [0u8; 32];
    memo_bytes[..2].copy_from_slice(CYCLES_BALANCE_TOPUP_MEMO_START);
    memo_bytes[2..].copy_from_slice(&principal_as_thirty_bytes(user));
    memo_bytes
}


#[derive(CandidType, Deserialize)]
pub enum CheckUserCyclesBalanceError {
    FindUserError(FindUserError),
}

pub async fn check_user_cycles_balance(user: &Principal) -> Result<Cycles, CheckUserCyclesBalanceError> {
    with(&USERS_DATA, |users_data| {
        if let Some(u) = users_data.get(user) {
            Ok(u.cycles_balance)
        } else {
            Ok(0)
        }
        //ud.borrow_mut().entry(*user).or_default().cycles_balance
    })
}


pub fn main_cts_icp_id() -> IcpId {  // do once
    IcpId::new(&id(), &ICP_DEFAULT_SUBACCOUNT)
}


pub fn check_lock_and_lock_user(user: &Principal) {
    USERS_DATA.with(|ud| {
        let users_data: &mut HashMap<Principal, UserData> = &mut ud.borrow_mut();
        let user_lock: &mut UserLock = &mut users_data.entry(*user).or_default().user_lock;
        let current_time: u64 = time();
        if user_lock.lock == true && current_time - user_lock.last_lock_time_nanos < 30*60*1_000_000_000 {
            trap("this user is in the middle of a different call");
        }
        user_lock.lock = true;
        user_lock.last_lock_time_nanos = current_time;
    });
}

pub fn unlock_user(user: &Principal) {
    USERS_DATA.with(|ud| {
        ud.borrow_mut().get_mut(user).unwrap().user_lock.lock = false;
    });
}


#[derive(CandidType, Deserialize)]
pub enum CheckCurrentXdrPerMyriadPerIcpCmcRateError {
    CmcGetRateCallError(String),
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
    pub xdr_permyriad_per_icp : u64,
    pub timestamp_seconds : u64
}

pub type CheckCurrentXdrPerMyriadPerIcpCmcRateSponse = Result<u64, CheckCurrentXdrPerMyriadPerIcpCmcRateError>;

// how many 1/10000-xdr for one icp
pub async fn check_current_xdr_permyriad_per_icp_cmc_rate() -> CheckCurrentXdrPerMyriadPerIcpCmcRateSponse {

    let latest_known_cmc_rate: LatestKnownCmcRate = LATEST_KNOWN_CMC_RATE.with(|r| { r.get() }); 
    if time() / 1_000_000_000 - latest_known_cmc_rate.timestamp_seconds < 10*60 {
        return Ok(latest_known_cmc_rate.xdr_permyriad_per_icp);
    }
    
    let call_sponse_candid_bytes: Vec<u8> = match call_raw128(
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        "get_icp_xdr_conversion_rate",
        &encode_one(()).unwrap(),
        0
    ).await {
        Ok(b) => b,
        Err(call_error) => return Err(CheckCurrentXdrPerMyriadPerIcpCmcRateError::CmcGetRateCallError(format!("{:?}", call_error)))
    };
    let icp_xdr_conversion_rate: IcpXdrConversionRate = match decode_one::<IcpXdrConversionRateCertifiedResponse>(&call_sponse_candid_bytes) {
        Ok(s) => s.data,
        Err(candid_error) => return Err(CheckCurrentXdrPerMyriadPerIcpCmcRateError::CmcGetRateCallSponseCandidError(format!("{}", candid_error))),
    };

    LATEST_KNOWN_CMC_RATE.with(|r| { r.set(icp_xdr_conversion_rate); });
    Ok(icp_xdr_conversion_rate.xdr_permyriad_per_icp)
}



pub fn icptokens_to_cycles(icpts: IcpTokens, xdr_permyriad_per_icp: u64) -> u128 {
    icpts.e8s() as u128 
    * xdr_permyriad_per_icp as u128 
    * DEFAULT_CYCLES_PER_XDR 
    / (IcpTokens::SUBDIVIDABLE_BY as u128 * 10_000)
}

pub fn cycles_to_icptokens(cycles: u128, xdr_permyriad_per_icp: u64) -> IcpTokens {
    IcpTokens::from_e8s(
        ( cycles
        * (IcpTokens::SUBDIVIDABLE_BY as u128 * 10_000)
        / DEFAULT_CYCLES_PER_XDR
        / xdr_permyriad_per_icp as u128 ) as u64    
    )
}


#[derive(CandidType, Deserialize)]
pub struct ManagementCanisterCreateCanisterQuest {
    settings : Option<ManagementCanisterOptionalCanisterSettings>
}

#[derive(CandidType, Deserialize)]
pub struct ManagementCanisterOptionalCanisterSettings {
    pub controllers : Option<Vec<Principal>>,
    pub compute_allocation : Option<u128>,
    pub memory_allocation : Option<u128>,
    pub freezing_threshold : Option<u128>,
}

#[derive(CandidType, Deserialize)]
pub struct ManagementCanisterCanisterSettings {
    pub controllers : Vec<Principal>,
    pub compute_allocation : u128,
    pub memory_allocation : u128,
    pub freezing_threshold : u128
}

#[derive(CandidType, Deserialize)]
pub struct ManagementCanisterCanisterStatusRecord {
    pub status : ManagementCanisterCanisterStatusVariant,
    pub settings: ManagementCanisterCanisterSettings,
    pub module_hash: Option<[u8; 32]>,
    pub memory_size: u128,
    pub cycles: u128
}

#[derive(CandidType, Deserialize, PartialEq)]
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


#[derive(CandidType, Deserialize)]
pub enum GetNewCanisterError {
    CreateCanisterManagementCallQuestCandidError(String),
    CreateCanisterManagementCallSponseCandidError{candid_error: String, candid_bytes: Vec<u8>},
    CreateCanisterManagementCallError(String)
}

pub async fn get_new_canister() -> Result<Principal, GetNewCanisterError> {
    
    if let Some(principal) = NEW_CANISTERS.with(|nc| nc.borrow_mut().pop()) {
        return Ok(principal);
    } 

    let create_canister_management_call_quest_candid_bytes: Vec<u8> = match encode_one(
        &ManagementCanisterCreateCanisterQuest {
            settings: Some(ManagementCanisterOptionalCanisterSettings{
                controllers: Some(vec![ic_cdk::api::id()]),
                compute_allocation : None,
                memory_allocation : None,
                freezing_threshold : None
            })
        }
    ) {
        Ok(candid_bytes) => candid_bytes,
        Err(candid_error) => {
            return Err(GetNewCanisterError::CreateCanisterManagementCallQuestCandidError(format!("{}", candid_error)))
        }
    };

    let create_canister_management_call: CallResult<Vec<u8>> = call_raw128(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "create_canister",
        &create_canister_management_call_quest_candid_bytes,
        100_000_000_000/*create_canister-cost*/ + 501_000_000_000 
    ).await;

    let canister_principal: Principal = match create_canister_management_call {
        Ok(call_sponse_candid_bytes) => match decode_one::<CanisterIdRecord>(&call_sponse_candid_bytes) {
            Ok(canister_id_record) => canister_id_record.canister_id,
            Err(candid_error) => {
                return Err(GetNewCanisterError::CreateCanisterManagementCallSponseCandidError{ candid_error: format!("{}", candid_error), candid_bytes: call_sponse_candid_bytes });
            }
        },
        Err(call_error) => {
            return Err(GetNewCanisterError::CreateCanisterManagementCallError(format!("{:?}", call_error)));
        }
    };

    Ok(canister_principal)

}













#[derive(CandidType, Deserialize)]
struct NotifyTopUpArg {
    block_index: IcpBlockHeight,
    canister_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub enum NotifyError {
    Refunded { block_index: Option<IcpBlockHeight>, reason: String },
    InvalidTransaction(String),
    Other{ error_message: String, error_code: u64 },
    Processing,
    TransactionTooOld(IcpBlockHeight),
}

type NotifyTopUpResult = Result<Cycles, NotifyError>;



#[derive(CandidType, Deserialize)]
pub enum LedgerTopupCyclesError {
    IcpTransferCallError(String),
    IcpTransferError(IcpTransferError),
    CmcNotifyTopUpQuestCandidEncodeError { candid_error: String, topup_transfer_block_height: IcpBlockHeight },
    CmcNotifyCallError { notify_call_error: String, topup_transfer_block_height: IcpBlockHeight },
    CmcNotifySponseCandidDecodeError { candid_error: String, candid_bytes: Vec<u8>, topup_transfer_block_height: IcpBlockHeight },
    CmcNotifyError{notify_error: NotifyError, topup_transfer_block_height: IcpBlockHeight},
    CmcNotifySponseRefund(String, Option<IcpBlockHeight>),
    UnknownCmcNotifySponse
}

// make a public method to re-try a block-height
pub async fn ledger_topup_cycles(icp: IcpTokens, from_subaccount: Option<IcpIdSub>, topup_canister: Principal) -> Result<Cycles, LedgerTopupCyclesError> {

    let topup_cycles_icp_transfer_call: CallResult<IcpTransferResult> = icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: ICP_TOP_UP_CANISTER_MEMO,
            amount: icp,                              
            fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            from_subaccount: from_subaccount,
            to: IcpId::new(&MAINNET_CYCLES_MINTING_CANISTER_ID, &principal_icp_subaccount(&topup_canister)),
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
        }
    ).await; 
    
    let topup_cycles_icp_transfer_call_block_index: IcpBlockHeight = match topup_cycles_icp_transfer_call {
        Ok(transfer_call_sponse) => match transfer_call_sponse {
            Ok(block_index) => block_index,
            Err(transfer_error) => {
                return Err(LedgerTopupCyclesError::IcpTransferError(transfer_error));
            }
        },
        Err(transfer_call_error) => {
            return Err(LedgerTopupCyclesError::IcpTransferCallError(format!("{:?}", transfer_call_error)));
        }
    }; 

    // :give-back: topup_cycles_icp_transfer_call_block_index on each follow[ing]-error-case.

    let topup_cycles_cmc_notify_call_candid: Vec<u8> = match encode_one(
        & NotifyTopUpArg {
            block_index: topup_cycles_icp_transfer_call_block_index,
            canister_id: topup_canister
        }
    ) {
        Ok(b) => b,
        Err(candid_error) => {
            return Err(LedgerTopupCyclesError::CmcNotifyTopUpQuestCandidEncodeError { candid_error: format!("{}", candid_error), topup_transfer_block_height: topup_cycles_icp_transfer_call_block_index });
        }
    };

    let topup_cycles_cmc_notify_call: CallResult<Vec<u8>> = call_raw128(
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        "notify_top_up",
        &topup_cycles_cmc_notify_call_candid,
        0
    ).await;
    let cycles: Cycles = match topup_cycles_cmc_notify_call {
        Ok(candid_bytes) => match decode_one::<NotifyTopUpResult>(&candid_bytes) {
            Ok(notify_topup_result) => match notify_topup_result {
                Ok(cycles) => cycles,
                Err(notify_error) => {
                    return Err(LedgerTopupCyclesError::CmcNotifyError{notify_error: notify_error, topup_transfer_block_height: topup_cycles_icp_transfer_call_block_index});
                }
            },
            Err(candid_error) => {
                return Err(LedgerTopupCyclesError::CmcNotifySponseCandidDecodeError { candid_error: format!("{}", candid_error), candid_bytes: candid_bytes, topup_transfer_block_height: topup_cycles_icp_transfer_call_block_index });
            }
        },
        Err(notify_call_error) => {
            return Err(LedgerTopupCyclesError::CmcNotifyCallError { notify_call_error: format!("{:?}", notify_call_error), topup_transfer_block_height: topup_cycles_icp_transfer_call_block_index });
        }
    };

    Ok(cycles)
}

#[derive(CandidType, Deserialize)]
pub enum LedgerCreateCanisterError {

}

pub async fn ledger_create_canister(icp: IcpTokens, from_subaccount: Option<IcpIdSub>, controller: Principal) -> Result<Principal, LedgerCreateCanisterError> {


}


