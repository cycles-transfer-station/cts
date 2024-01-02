use std::{
    cell::{Cell, RefCell}, 
    collections::{HashMap},
};
use serde_bytes::ByteBuf;
use num_traits::cast::ToPrimitive;
use sha2::Digest;

use cts_lib::{
    self,
    types::{
        Cycles,
        CTSFuel,
        CyclesTransfer,
        CyclesTransferMemo,
        XdrPerMyriadPerIcp,
        CallError,
        canister_code::CanisterCode,
        cache::Cache,
        cbs_map::{
            CBSMUserData,
            self,
        },
        cycles_bank::{
            CyclesBankInit,
        },
        cts::{LengthenMembershipQuest, UserAndCB},
        http_request::*,
    },
    management_canister::{
        ManagementCanisterInstallCodeMode,
        ManagementCanisterInstallCodeQuest,
        ManagementCanisterCanisterSettings,
        ManagementCanisterOptionalCanisterSettings,
        ManagementCanisterCanisterStatusRecord,
        ManagementCanisterCanisterStatusVariant,
        CanisterIdRecord,
        ChangeCanisterSettingsRecord,
        ManagementCanisterCreateCanisterQuest,
        self,
    },
    consts::{
        TRILLION,
        MiB,
        MANAGEMENT_CANISTER_ID,
        NETWORK_CANISTER_CREATION_FEE_CYCLES,
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES,
        CTS_TRANSFER_ICP_FEE_ICP_MEMO,
        CTS_PURCHASE_CYCLES_BANK_COLLECT_PAYMENT_ICP_MEMO,
        NANOS_IN_A_SECOND,
        SECONDS_IN_A_DAY,
    },
    tools::{
        sha256,
        localkey::{
            self,
            refcell::{
                with, 
                with_mut,
            }
        },
        principal_icp_subaccount,
        cycles_to_icptokens,
        caller_is_controller_gaurd,
        call_error_as_u32_and_string,
        upgrade_canisters::*,
    },
    ic_cdk::{
        self,
        api::{
            canister_balance128,
            trap,
            caller, 
            time,
            id,
            call::{
                arg_data,
                arg_data_raw_size,
                call_raw128,
                call,
                call_with_payment128,
                CallResult,
                msg_cycles_available128,
                msg_cycles_accept128,
                reject,
                reply,
            },
        },
        update, 
        query, 
        init, 
        pre_upgrade, 
        post_upgrade
    },
    ic_ledger_types::{
        IcpMemo,
        IcpId,
        IcpTokens,
        IcpBlockHeight,
        IcpTimestamp,
        ICP_LEDGER_TRANSFER_DEFAULT_FEE,
        MAINNET_LEDGER_CANISTER_ID, 
        icp_transfer,
        IcpTransferArgs, 
        IcpTransferError,
        ICP_DEFAULT_SUBACCOUNT,
    },
    //global_allocator_counter::get_allocated_bytes_count
};
use candid::{
    Principal,
    CandidType,
    Deserialize,
    Func,
    utils::{
        encode_one, 
    }
};

use canister_tools::{
    self,
    MemoryId,
};



#[cfg(test)]
mod tests;

mod tools;
use tools::{
    check_user_icp_ledger_balance,
    CheckCurrentXdrPerMyriadPerIcpCmcRateError,
    CheckCurrentXdrPerMyriadPerIcpCmcRateSponse,
    check_current_xdr_permyriad_per_icp_cmc_rate,
    IcpXdrConversionRate,
    transfer_user_icp_ledger,
    CmcNotifyError,
    PutNewUserIntoACBSMError,
    put_new_user_into_a_cbsm,
    FindUserInTheCBSMapsError,
    find_user_in_the_cbs_maps,
    ledger_topup_cycles_cmc_icp_transfer,
    ledger_topup_cycles_cmc_notify,
    LedgerTopupCyclesCmcIcpTransferError,
    LedgerTopupCyclesCmcNotifyError,  
};

mod frontcode;
use frontcode::{
    File, 
    Files, 
    FilesHashes, 
    create_opt_stream_callback_token,
};

mod certification;
use certification::*;


// -------


#[derive(CandidType, Deserialize)]
pub struct CTSData {
    cycles_market_main: Principal,
    cycles_bank_canister_code: CanisterCode,
    cbs_map_canister_code: CanisterCode,
    frontcode_files: Files,
    frontcode_files_hashes: FilesHashes,
    cb_auths: CBAuths,
    cbs_maps: HashMap<Principal, CBSMStatus>,
    create_new_cbs_map_lock: bool,
    temp_create_new_cbsmap_holder: Option<Principal>,
    users_purchase_cycles_bank: HashMap<Principal, PurchaseCyclesBankData>,
    users_transfer_icp: HashMap<Principal, TransferIcpData>,
    users_lengthen_membership: HashMap<Principal, LengthenMembershipMidCallData>,
    users_lengthen_membership_cb_cycles_payment: HashMap<Principal, LengthenMembershipMidCallData>,
    cb_cache: Cache<Principal/*user-id*/, Option<Principal/*cycles-bank*/>>,
}
impl CTSData {
    fn new() -> Self {
        Self {
            cycles_market_main: Principal::from_slice(&[]),
            cycles_bank_canister_code: CanisterCode::empty(),
            cbs_map_canister_code: CanisterCode::empty(),
            frontcode_files: Files::new(),
            frontcode_files_hashes: FilesHashes::new(),
            cb_auths: CBAuths::default(),
            cbs_maps: HashMap::new(),
            create_new_cbs_map_lock: false,
            temp_create_new_cbsmap_holder: None,
            users_purchase_cycles_bank: HashMap::new(),
            users_transfer_icp: HashMap::new(),
            users_lengthen_membership: HashMap::new(),
            users_lengthen_membership_cb_cycles_payment: HashMap::new(),
            cb_cache: Cache::<Principal, Option<Principal>>::new(CB_CACHE_SIZE),
        }
    }
}

#[derive(CandidType, Deserialize)]
pub struct CBSMStatus {
    module_hash: [u8; 32],
}
    
    
type ModuleHash = [u8; 32];
 
 
 

pub const MEMBERSHIP_COST_CYCLES: Cycles = 15 * TRILLION;
pub const NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS: u128 = SECONDS_IN_A_DAY * 365; // 1-year
pub const NEW_CYCLES_BANK_CTSFUEL: CTSFuel = 5 * TRILLION;
#[allow(non_upper_case_globals)]
pub const NEW_CYCLES_BANK_STORAGE_SIZE_MiB: u128 = 100;
#[allow(non_upper_case_globals)]
pub const NEW_CYCLES_BANK_NETWORK_MEMORY_ALLOCATION_MiB: u128 = cts_lib::consts::cb_storage_size_mib_as_cb_network_memory_allocation_mib(NEW_CYCLES_BANK_STORAGE_SIZE_MiB);
pub const NEW_CYCLES_BANK_BACKUP_CYCLES: Cycles = 2 * TRILLION;
pub const NEW_CYCLES_BANK_CREATION_CYCLES: Cycles = {
    NETWORK_CANISTER_CREATION_FEE_CYCLES
    + (
        NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS as u128 
        * NEW_CYCLES_BANK_NETWORK_MEMORY_ALLOCATION_MiB as u128 
        * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES as u128 
        / 1024 /*network mib storage per second*/ )
    + NEW_CYCLES_BANK_CTSFUEL
    + NEW_CYCLES_BANK_BACKUP_CYCLES
};
pub const NEW_CYCLES_BANK_FREEZING_THRESHOLD: u128 = 2592000 * 3;

pub const MAX_USERS_PURCHASE_CYCLES_BANK: usize = 170; // the max number of entries in the hashmap at the same-time

pub const MAX_CBS_MAPS: usize = 4; // can be 30-million at 1-gb, or 3-million at 0.1-gb,

pub const CREATE_CBS_MAP_CANISTER_CYCLES: Cycles = 20 * TRILLION;
pub const CREATE_CBS_MAP_CANISTER_NETWORK_MEMORY_ALLOCATION: u128 = 100 * MiB as u128;

pub const MAX_USERS_TRANSFER_ICP: usize = 170;
pub const CTS_TRANSFER_ICP_FEE: Cycles = 30_000_000_000; // taken as the icptokens by the conversion-rate

const MAX_USERS_LENGTHEN_MEMBERSHIP: usize = 170;
const MAX_USERS_LENGTHEN_MEMBERSHIP_CB_CYCLES_PAYMENT: usize = 170; 

pub const MINIMUM_CTS_CYCLES_TRANSFER_IN_CYCLES: Cycles = 5_000_000_000;

pub const CB_CACHE_SIZE: usize = {
    #[cfg(not(debug_assertions))]
    {1400}
    #[cfg(debug_assertions)]
    {4}
};

pub const CB_AUTHS_MAX_SIZE: usize = CB_CACHE_SIZE;
pub const MINIMUM_CB_AUTH_DURATION_NANOS: u64 = (NANOS_IN_A_SECOND * SECONDS_IN_A_DAY) as u64;



const STABLE_MEMORY_CTS_DATA_SERIALIZATION_MEMORY_ID: MemoryId = MemoryId::new(0);



thread_local! {
    
    pub static CTS_DATA: RefCell<CTSData> = RefCell::new(CTSData::new());
    
    // not save through the upgrades
    pub static LATEST_KNOWN_CMC_RATE: Cell<IcpXdrConversionRate> = Cell::new(IcpXdrConversionRate{ xdr_permyriad_per_icp: 0, timestamp_seconds: 0 });
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    
}



// -------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CTSInit {
    cycles_market_main: Principal,
} 

#[init]
fn init(cts_init: CTSInit) {
    canister_tools::init(&CTS_DATA, STABLE_MEMORY_CTS_DATA_SERIALIZATION_MEMORY_ID);

    with_mut(&CTS_DATA, |cts_data| { 
        cts_data.cycles_market_main = cts_init.cycles_market_main; 
    });
    
    if NEW_CYCLES_BANK_CREATION_CYCLES > MEMBERSHIP_COST_CYCLES {
        trap("NEW_CYCLES_BANK_CREATION_CYCLES > MEMBERSHIP_COST_CYCLES");
    }
    
} 


// -------------------------------------------------------------


#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() { 
    canister_tools::post_upgrade(&CTS_DATA, STABLE_MEMORY_CTS_DATA_SERIALIZATION_MEMORY_ID, None::<fn(CTSData) -> CTSData>);
    
    with(&CTS_DATA, |cts_data| {
        set_root_hash(&cts_data);
    });
    
    if NEW_CYCLES_BANK_CREATION_CYCLES > MEMBERSHIP_COST_CYCLES {
        trap("NEW_CYCLES_BANK_CREATION_CYCLES > MEMBERSHIP_COST_CYCLES");
    }
    
} 


// test this!
#[no_mangle]
pub fn canister_inspect_message() {
    // caution: this function is only called for ingress messages 
    use ic_cdk::api::call::{method_name,accept_message};
    
    if caller() == Principal::anonymous() 
        && !["view_fees", "local_put_ic_root_key"].contains(&&method_name()[..])
        {
        trap("caller cannot be anonymous for this method.");
    }
    
    // check the size of the arg_data_raw_size()

    if &method_name()[..] == "cycles_transfer" {
        trap("caller must be a canister for this method.");
    }
    
    if method_name()[..].starts_with("controller") {
        caller_is_controller_gaurd(&caller());
    }

    accept_message();
}







// ----------------------------------------------------------------------------------------



#[derive(CandidType, Deserialize, Debug)]
pub enum UserIsInTheMiddleOfADifferentCall {
    PurchaseCyclesBankCall{ must_call_complete: bool },
    BurnIcpMintCyclesCall{ must_call_complete: bool },
    TransferIcpCall{ must_call_complete: bool },
    LengthenMembershipCall{ must_call_complete: bool },
    LengthenMembershipCBCyclesPaymentCall{ must_call_complete: bool },
}


fn check_if_user_is_in_the_middle_of_a_different_call(cts_data: &CTSData, user_id: &Principal) -> Result<(), UserIsInTheMiddleOfADifferentCall> {
    if let Some(purchase_cycles_bank_data) = cts_data.users_purchase_cycles_bank.get(user_id) {
        return Err(UserIsInTheMiddleOfADifferentCall::PurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock });
    }
    if let Some(transfer_icp_data) = cts_data.users_transfer_icp.get(user_id) {
        return Err(UserIsInTheMiddleOfADifferentCall::TransferIcpCall{ must_call_complete: !transfer_icp_data.lock });
    }
    if let Some(lengthen_membership_mid_call_data) = cts_data.users_lengthen_membership.get(user_id) {
        return Err(UserIsInTheMiddleOfADifferentCall::LengthenMembershipCall{ must_call_complete: !lengthen_membership_mid_call_data.lock });
    }
    if let Some(lengthen_membership_mid_call_data) = cts_data.users_lengthen_membership_cb_cycles_payment.get(user_id) {
        return Err(UserIsInTheMiddleOfADifferentCall::LengthenMembershipCBCyclesPaymentCall{ must_call_complete: !lengthen_membership_mid_call_data.lock });
    }
    
    Ok(())           
}




// -----------------------------------------------



#[export_name = "canister_update cycles_transfer"]
pub fn cycles_transfer() {
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); }
    
    if arg_data_raw_size() > 100 {
        reject("arg_data_raw_size must be <= 100");
        return;
    }

    if msg_cycles_available128() < MINIMUM_CTS_CYCLES_TRANSFER_IN_CYCLES {
        reject(&format!("minimum cycles: {}", MINIMUM_CTS_CYCLES_TRANSFER_IN_CYCLES));
        return;
    }

    let (ct,): (CyclesTransfer,) = arg_data::<(CyclesTransfer,)>();
    
    match ct.memo {
        CyclesTransferMemo::Blob(b) => {
            if b == b"DONATION" {
                msg_cycles_accept128(msg_cycles_available128());
            } else {
                reject("unknown CyclesTransferMemo");
                return;
            }
        },
        _ => {
            reject("unknown CyclesTransferMemo");
            return;
        }
    }
            
}










#[derive(CandidType, Deserialize)]
pub struct Fees {
    membership_cost_per_year_cycles: Cycles,
    cts_transfer_icp_fee: Cycles,
    
    
    
}

#[query]
pub fn view_fees() -> Fees {
    Fees {
        membership_cost_per_year_cycles: MEMBERSHIP_COST_CYCLES,
        cts_transfer_icp_fee: CTS_TRANSFER_ICP_FEE,
    }
}











// save the fees in the purchase_cycles_bank_data so the fees cant change while creating a new user

#[derive(Clone, CandidType, Deserialize)]
pub struct PurchaseCyclesBankData {
    start_time_nanos: u128,
    lock: bool,    
    purchase_cycles_bank_quest: PurchaseCyclesBankQuest,
    // the options and bools are for the memberance of the steps
    current_xdr_icp_rate: Option<XdrPerMyriadPerIcp>,
    original_user_icp_ledger_balance: Option<IcpTokens>,
    look_if_user_is_in_the_cbs_maps: bool,
    create_cycles_bank_canister_block_height: Option<IcpBlockHeight>,
    create_cycles_bank_canister_cmc_notify_topup_cycles: Option<Cycles>,
    cycles_bank_canister: Option<Principal>,
    cbs_map: Option<Principal>,
    cycles_bank_canister_uninstall_code: bool,
    cycles_bank_canister_install_code: Option<ModuleHash>,
    cycles_bank_canister_status_record: Option<ManagementCanisterCanisterStatusRecord>,
    update_cbsm_user_data_with_the_cb_module_hash: bool,
    collect_icp: bool,
    transfer_mainder_user_cts_icp_balance: Option<u64>,
    
}



#[derive(CandidType, Deserialize, Debug)]
pub enum PurchaseCyclesBankMidCallError{
    CBSMapsFindUserCallFails(Vec<(Principal, (u32, String))>),
    PutNewUserIntoACBSMError(PutNewUserIntoACBSMError),
    CreateCyclesBankCanisterLedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
    CreateCyclesBankCanisterLedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
    CreateCyclesBankManagementCallError(CallError),
    CyclesBankUninstallCodeCallError((u32, String)),
    CyclesBankCodeNotFound,
    CyclesBankInstallCodeCallError((u32, String)),
    CyclesBankCanisterStatusCallError((u32, String)),
    CyclesBankModuleVerificationError,
    CyclesBankStartCanisterCallError((u32, String)),
    CyclesBankUpdateSettingsCallError((u32, String)),
    UpdateCBSMUserDataWithTheCBModuleHashCallError(CallError),
    UpdateCBSMUserDataWithTheCBModuleHashError(cbs_map::UpdateUserError),
    CollectIcpTransferError(IcpTransferError),
    CollectIcpTransferCallError((u32, String)),
    TransferMainderUserCTSICPTransferCallError(CallError),
    TransferMainderUserCTSICPTransferError(IcpTransferError),
    
}


#[derive(CandidType, Deserialize, Debug)]
pub enum PurchaseCyclesBankError{
    CheckIcpBalanceCallError((u32, String)),
    CheckCurrentXdrPerMyriadPerIcpCmcRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    UserIcpLedgerBalanceTooLow{
        membership_cost_icp: IcpTokens,
        user_icp_ledger_balance: IcpTokens,
        icp_ledger_transfer_fee: IcpTokens
    },
    UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
    CTSIsBusy,
    FoundCyclesBank(Principal),
    CreateCyclesBankCanisterCmcNotifyError(CmcNotifyError),
    MidCallError(PurchaseCyclesBankMidCallError),    // call complete_purchase_cycles_bank on this sponse
}


#[derive(CandidType, Deserialize, Clone, PartialEq, Eq)]
pub struct PurchaseCyclesBankQuest {
    sns_control: Option<bool>,
}


#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesBankSuccess {
    cycles_bank_canister_id: Principal,
}


fn write_purchase_cycles_bank_data(user_id: &Principal, purchase_cycles_bank_data: PurchaseCyclesBankData) {
    with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_purchase_cycles_bank.get_mut(user_id) {
            Some(nud) => { *nud = purchase_cycles_bank_data; },
            None => {}
        }
    });
}

// for the now a user must pay with the icp.
#[update]
pub async fn purchase_cycles_bank(q: PurchaseCyclesBankQuest) -> Result<PurchaseCyclesBankSuccess, PurchaseCyclesBankError> {

    let user_id: Principal = caller();
    
    let purchase_cycles_bank_data: PurchaseCyclesBankData = with_mut(&CTS_DATA, |cts_data| {
        check_if_user_is_in_the_middle_of_a_different_call(cts_data, &user_id).map_err(|e| PurchaseCyclesBankError::UserIsInTheMiddleOfADifferentCall(e))?;
        if cts_data.users_purchase_cycles_bank.len() >= MAX_USERS_PURCHASE_CYCLES_BANK {
            return Err(PurchaseCyclesBankError::CTSIsBusy);
        }
        let purchase_cycles_bank_data: PurchaseCyclesBankData = PurchaseCyclesBankData{
            start_time_nanos: time() as u128,
            lock: true,
            purchase_cycles_bank_quest: q,
            // the options and bools are for the memberance of the steps
            current_xdr_icp_rate: None,
            original_user_icp_ledger_balance: None,
            look_if_user_is_in_the_cbs_maps: false,
            create_cycles_bank_canister_block_height: None,
            create_cycles_bank_canister_cmc_notify_topup_cycles: None,
            cycles_bank_canister: None,
            cbs_map: None,
            cycles_bank_canister_uninstall_code: false,
            cycles_bank_canister_install_code: None,
            cycles_bank_canister_status_record: None,
            update_cbsm_user_data_with_the_cb_module_hash: false,
            collect_icp: false,
            transfer_mainder_user_cts_icp_balance: None,
        };
        cts_data.users_purchase_cycles_bank.insert(user_id, purchase_cycles_bank_data.clone());
        Ok(purchase_cycles_bank_data)
    })?;    
    
    purchase_cycles_bank_(user_id, purchase_cycles_bank_data).await
}


#[derive(CandidType, Deserialize)]
pub enum CompletePurchaseCyclesBankError {
    UserIsNotInTheMiddleOfAPurchaseCyclesBankCall,
    PurchaseCyclesBankError(PurchaseCyclesBankError)
}


#[update]
pub async fn complete_purchase_cycles_bank() -> Result<PurchaseCyclesBankSuccess, CompletePurchaseCyclesBankError> {

    let user_id: Principal = caller();
    
    complete_purchase_cycles_bank_(user_id).await
    
}


async fn complete_purchase_cycles_bank_(user_id: Principal) -> Result<PurchaseCyclesBankSuccess, CompletePurchaseCyclesBankError> {
    
    let purchase_cycles_bank_data: PurchaseCyclesBankData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_purchase_cycles_bank.get_mut(&user_id) {
            Some(purchase_cycles_bank_data) => {
                if purchase_cycles_bank_data.lock == true {
                    return Err(CompletePurchaseCyclesBankError::PurchaseCyclesBankError(PurchaseCyclesBankError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::PurchaseCyclesBankCall{ must_call_complete: false })));
                }
                purchase_cycles_bank_data.lock = true;
                Ok(purchase_cycles_bank_data.clone())
            },
            None => {
                return Err(CompletePurchaseCyclesBankError::UserIsNotInTheMiddleOfAPurchaseCyclesBankCall);
            }
        }
    })?;

    purchase_cycles_bank_(user_id, purchase_cycles_bank_data).await
        .map_err(|purchase_cycles_bank_error| { 
            CompletePurchaseCyclesBankError::PurchaseCyclesBankError(purchase_cycles_bank_error) 
        })
    
}


async fn purchase_cycles_bank_(user_id: Principal, mut purchase_cycles_bank_data: PurchaseCyclesBankData) -> Result<PurchaseCyclesBankSuccess, PurchaseCyclesBankError> {
    
    if purchase_cycles_bank_data.current_xdr_icp_rate.is_none() {

        let (
            check_user_icp_ledger_balance_sponse,
            check_current_xdr_permyriad_per_icp_cmc_rate_sponse,
        ): (
            CallResult<IcpTokens>,
            CheckCurrentXdrPerMyriadPerIcpCmcRateSponse
        ) = futures::future::join(
            check_user_icp_ledger_balance(&user_id), 
            check_current_xdr_permyriad_per_icp_cmc_rate()
        ).await; 
        
        let user_icp_ledger_balance: IcpTokens = match check_user_icp_ledger_balance_sponse {
            Ok(tokens) => tokens,
            Err(check_balance_call_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
                return Err(PurchaseCyclesBankError::CheckIcpBalanceCallError((check_balance_call_error.0 as u32, check_balance_call_error.1)));
            }
        };
                
        let current_xdr_icp_rate: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate_sponse {
            Ok(rate) => rate,
            Err(check_xdr_icp_rate_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
                return Err(PurchaseCyclesBankError::CheckCurrentXdrPerMyriadPerIcpCmcRateError(check_xdr_icp_rate_error));
            }
        };
        
        let current_membership_cost_icp: IcpTokens = cycles_to_icptokens(MEMBERSHIP_COST_CYCLES, current_xdr_icp_rate); 
        
        if user_icp_ledger_balance < current_membership_cost_icp + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) {
            with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
            return Err(PurchaseCyclesBankError::UserIcpLedgerBalanceTooLow{
                membership_cost_icp: current_membership_cost_icp,
                user_icp_ledger_balance,
                icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE
            });
        }   
        
        purchase_cycles_bank_data.current_xdr_icp_rate = Some(current_xdr_icp_rate);
        purchase_cycles_bank_data.original_user_icp_ledger_balance = Some(user_icp_ledger_balance);
    }
    
    
    if purchase_cycles_bank_data.look_if_user_is_in_the_cbs_maps == false {
        // check in the list of the users-whos cycles-balance is save but without a user-canister 
        
        match find_cycles_bank_(&user_id).await {
            Ok(opt_cycles_bank_canister_id) => match opt_cycles_bank_canister_id {
                Some(cycles_bank_canister_id) => {
                    with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
                    return Err(PurchaseCyclesBankError::FoundCyclesBank(cycles_bank_canister_id));
                },
                None => {
                    purchase_cycles_bank_data.look_if_user_is_in_the_cbs_maps = true;
                }
            },
            Err(find_user_in_the_cbsms_error) => match find_user_in_the_cbsms_error {
                FindUserInTheCBSMapsError::CBSMapsFindUserCallFails(umc_call_errors) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CBSMapsFindUserCallFails(umc_call_errors)));
                }
            }
        }   
    }
    
    if purchase_cycles_bank_data.create_cycles_bank_canister_block_height.is_none() {
        let create_cycles_bank_canister_block_height = match ledger_topup_cycles_cmc_icp_transfer(
            cycles_to_icptokens(NEW_CYCLES_BANK_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()),
            ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            Some(principal_icp_subaccount(&user_id)),
            ic_cdk::api::id(),
        ).await {
            Ok(block_height) => block_height,
            Err(e) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankCanisterLedgerTopupCyclesCmcIcpTransferError(e)));
            }
        };
        
        /*
        let create_cycles_bank_canister_block_height: IcpBlockHeight = match icp_transfer(
            MAINNET_LEDGER_CANISTER_ID,
            IcpTransferArgs {
                memo: ICP_LEDGER_CREATE_CANISTER_MEMO,
                amount: cycles_to_icptokens(NEW_CYCLES_BANK_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()),
                fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                from_subaccount: Some(principal_icp_subaccount(&user_id)),
                to: IcpId::new(&MAINNET_CYCLES_MINTING_CANISTER_ID, &principal_icp_subaccount(&id())),
                created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
            }
        ).await {
            Ok(transfer_result) => match transfer_result {
                Ok(block_height) => block_height,
                Err(transfer_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankCanisterIcpTransferError(transfer_error)));                    
                }
            },
            Err(transfer_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankCanisterIcpTransferCallError((transfer_call_error.0 as u32, transfer_call_error.1))));
            }
        };
        */
    
        purchase_cycles_bank_data.create_cycles_bank_canister_block_height = Some(create_cycles_bank_canister_block_height);
    }
    
    if purchase_cycles_bank_data.create_cycles_bank_canister_cmc_notify_topup_cycles.is_none() {
        match ledger_topup_cycles_cmc_notify(
            purchase_cycles_bank_data.create_cycles_bank_canister_block_height.unwrap(),
            ic_cdk::api::id(),
        ).await {
            Ok(topup_cycles) => {
                purchase_cycles_bank_data.create_cycles_bank_canister_cmc_notify_topup_cycles = Some(topup_cycles);    
            }
            Err(e) => {
                if let LedgerTopupCyclesCmcNotifyError::CmcNotifyError(ref cmc_notify_error) = e {
                    if let CmcNotifyError::TransactionTooOld(_) | CmcNotifyError::Refunded{ .. } = cmc_notify_error {
                        with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
                        return Err(PurchaseCyclesBankError::CreateCyclesBankCanisterCmcNotifyError(cmc_notify_error.clone()));
                    }        
                } 
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankCanisterLedgerTopupCyclesCmcNotifyError(e)));   
            }
        }
    }
    
    if purchase_cycles_bank_data.cycles_bank_canister.is_none() {
        if canister_balance128() < NEW_CYCLES_BANK_CREATION_CYCLES + 10*TRILLION {
            purchase_cycles_bank_data.lock = false;
            write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
            return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankManagementCallError((u32::MAX, "canister_balance128() < NEW_CYCLES_BANK_CREATION_CYCLES + 10*TRILLION".to_string()))));    
        }
        
        let cycles_bank_canister: Principal = match management_canister::create_canister(
            ManagementCanisterCreateCanisterQuest {
                settings: None,
            },
            NEW_CYCLES_BANK_CREATION_CYCLES,
        ).await {
            Ok(canister_id) => canister_id,
            Err(call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankManagementCallError(call_error)));
            }
        };
        
        /*
        let cycles_bank_canister: Principal = match call::<(CmcNotifyCreateCanisterQuest,), (Result<Principal, CmcNotifyError>,)>(
            MAINNET_CYCLES_MINTING_CANISTER_ID,
            "notify_create_canister",
            (CmcNotifyCreateCanisterQuest {
                controller: id(),
                block_index: purchase_cycles_bank_data.create_cycles_bank_canister_block_height.unwrap(),
                subnet_type: if ic_cdk::api::id() == Principal::from_slice(CTS_LOCAL_ID) { None } else { Some("fiduciary") }
            },)
        ).await {
            Ok((notify_result,)) => match notify_result {
                Ok(new_canister_id) => new_canister_id,
                Err(cmc_notify_error) => {
                    // match on the cmc_notify_error, if it failed bc of the cmc icp transfer block height expired, remove the user from the NEW_USERS map.     
                    match cmc_notify_error {
                        CmcNotifyError::TransactionTooOld(_) | CmcNotifyError::Refunded{ .. } => {
                            with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
                            return Err(PurchaseCyclesBankError::CreateCyclesBankCanisterCmcNotifyError(cmc_notify_error));
                        },
                        CmcNotifyError::InvalidTransaction(_) // 
                        | CmcNotifyError::Other{ .. }
                        | CmcNotifyError::Processing
                        => {
                            purchase_cycles_bank_data.lock = false;
                            write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                            return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankCanisterCmcNotifyError(cmc_notify_error)));   
                        },
                    }                    
                }
            },
            Err(cmc_notify_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CreateCyclesBankCanisterCmcNotifyCallError((cmc_notify_call_error.0 as u32, cmc_notify_call_error.1))));
            }      
        };
        */
        
        purchase_cycles_bank_data.cycles_bank_canister = Some(cycles_bank_canister);
        with_mut(&CTS_DATA, |cts_data| { cts_data.cb_cache.put(user_id, Some(cycles_bank_canister)); });
        purchase_cycles_bank_data.cycles_bank_canister_uninstall_code = true; // because a fresh cmc canister is empty 
    }
  
    
    if purchase_cycles_bank_data.cbs_map.is_none() {
        
        let cbs_map: Principal = match put_new_user_into_a_cbsm(
            user_id, 
            CBSMUserData{
                cycles_bank_canister_id: purchase_cycles_bank_data.cycles_bank_canister.as_ref().unwrap().clone(),
                first_membership_creation_timestamp_nanos: purchase_cycles_bank_data.start_time_nanos,
                cycles_bank_latest_known_module_hash: [0u8; 32],
                cycles_bank_lifetime_termination_timestamp_seconds: purchase_cycles_bank_data.start_time_nanos/NANOS_IN_A_SECOND + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS,
                membership_termination_cb_uninstall_data: None,
                sns_control: purchase_cycles_bank_data.purchase_cycles_bank_quest.sns_control.unwrap_or(false),
            }
        ).await {
            Ok(cbsm_id) => cbsm_id,
            Err(put_new_user_into_a_cbsm_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::PutNewUserIntoACBSMError(put_new_user_into_a_cbsm_error)));
            }
        };
        
        purchase_cycles_bank_data.cbs_map = Some(cbs_map);
    }

    if purchase_cycles_bank_data.cycles_bank_canister_uninstall_code == false {
        
        match call::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "uninstall_code",
            (CanisterIdRecord { canister_id: purchase_cycles_bank_data.cycles_bank_canister.unwrap() },),
        ).await {
            Ok(_) => {},
            Err(uninstall_code_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankUninstallCodeCallError(call_error_as_u32_and_string(uninstall_code_call_error))));
            }
        }
        
        purchase_cycles_bank_data.cycles_bank_canister_uninstall_code = true;
    }


    if purchase_cycles_bank_data.cycles_bank_canister_install_code.is_none() {
    
        if with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module().len() == 0 }) {
            purchase_cycles_bank_data.lock = false;
            write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
            return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankCodeNotFound));
        }
        
        let (install_code_call_future, module_hash) = with(&CTS_DATA, |cts_data| {
            (
                call_raw128( //::<(ManagementCanisterInstallCodeQuest,), ()>(
                    MANAGEMENT_CANISTER_ID,
                    "install_code",
                    encode_one(&ManagementCanisterInstallCodeQuest {
                        mode : ManagementCanisterInstallCodeMode::install,
                        canister_id : purchase_cycles_bank_data.cycles_bank_canister.unwrap(),
                        wasm_module : cts_data.cycles_bank_canister_code.module(),
                        arg : &encode_one(&CyclesBankInit{ 
                            cts_id: id(), 
                            cbsm_id: purchase_cycles_bank_data.cbs_map.unwrap(),
                            user_id: user_id,
                            storage_size_mib: NEW_CYCLES_BANK_STORAGE_SIZE_MiB,                         
                            lifetime_termination_timestamp_seconds: purchase_cycles_bank_data.start_time_nanos/NANOS_IN_A_SECOND + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS,
                            start_with_user_cycles_balance: 0,
                            sns_control: purchase_cycles_bank_data.purchase_cycles_bank_quest.sns_control.unwrap_or(false),
                        }).unwrap()
                    }).unwrap(),
                    0
                ),
                cts_data.cycles_bank_canister_code.module_hash().clone()
            )                
        });
        
        match install_code_call_future.await {
            Ok(_) => {},
            Err(put_code_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankInstallCodeCallError((put_code_call_error.0 as u32, put_code_call_error.1))));
            }
        }
        
        purchase_cycles_bank_data.cycles_bank_canister_install_code = Some(module_hash);
    }
    
    if purchase_cycles_bank_data.cycles_bank_canister_status_record.is_none() {
        
        let canister_status_record: ManagementCanisterCanisterStatusRecord = match call(
            MANAGEMENT_CANISTER_ID,
            "canister_status",
            (CanisterIdRecord { canister_id: purchase_cycles_bank_data.cycles_bank_canister.unwrap() },),
        ).await {
            Ok((canister_status_record,)) => canister_status_record,
            Err(canister_status_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankCanisterStatusCallError((canister_status_call_error.0 as u32, canister_status_call_error.1))));
            }
        };
        
        purchase_cycles_bank_data.cycles_bank_canister_status_record = Some(canister_status_record);
    }
        
    // no async in this if-block so no PurchaseCyclesBankData field needed. can make it for the optimization though
    if with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module().len() == 0 }) {
        purchase_cycles_bank_data.lock = false;
        write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
        return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankCodeNotFound));
    }
    if purchase_cycles_bank_data.cycles_bank_canister_status_record.as_ref().unwrap().module_hash.is_none() || purchase_cycles_bank_data.cycles_bank_canister_status_record.as_ref().unwrap().module_hash.as_ref().unwrap().clone() != with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module_hash().clone() }) {
        // go back a couple of steps
        purchase_cycles_bank_data.cycles_bank_canister_uninstall_code = false;                                  
        purchase_cycles_bank_data.cycles_bank_canister_install_code = None;
        purchase_cycles_bank_data.cycles_bank_canister_status_record = None;
        purchase_cycles_bank_data.lock = false;
        write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
        return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankModuleVerificationError));
    }
    

    if purchase_cycles_bank_data.cycles_bank_canister_status_record.as_ref().unwrap().status != ManagementCanisterCanisterStatusVariant::running {
    
        match call::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "start_canister",
            (CanisterIdRecord { canister_id: purchase_cycles_bank_data.cycles_bank_canister.unwrap() },)
        ).await {
            Ok(_) => {
                purchase_cycles_bank_data.cycles_bank_canister_status_record.as_mut().unwrap().status = ManagementCanisterCanisterStatusVariant::running; 
            },
            Err(start_canister_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankStartCanisterCallError((start_canister_call_error.0 as u32, start_canister_call_error.1))));
            }
        }   
    }
    
    let put_cycles_bank_canister_settings: ManagementCanisterCanisterSettings = ManagementCanisterCanisterSettings{
        controllers : vec![
            id(), 
            purchase_cycles_bank_data.cbs_map.as_ref().unwrap().clone(),
            purchase_cycles_bank_data.cycles_bank_canister.as_ref().unwrap().clone(),
        ],
        compute_allocation : 0,
        memory_allocation : 0, //NEW_CYCLES_BANK_NETWORK_MEMORY_ALLOCATION_MiB as u128 * MiB as u128,
        freezing_threshold : NEW_CYCLES_BANK_FREEZING_THRESHOLD,
    };
    
    if purchase_cycles_bank_data.cycles_bank_canister_status_record.as_ref().unwrap().settings != put_cycles_bank_canister_settings {
                
        match call::<(ChangeCanisterSettingsRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "update_settings",
            (ChangeCanisterSettingsRecord{
                canister_id: purchase_cycles_bank_data.cycles_bank_canister.as_ref().unwrap().clone(),
                settings: ManagementCanisterOptionalCanisterSettings{
                    controllers : Some(put_cycles_bank_canister_settings.controllers.clone()),
                    compute_allocation : Some(put_cycles_bank_canister_settings.compute_allocation),
                    memory_allocation : Some(put_cycles_bank_canister_settings.memory_allocation),
                    freezing_threshold : Some(put_cycles_bank_canister_settings.freezing_threshold),
                }
            },)
        ).await {
            Ok(()) => {
                purchase_cycles_bank_data.cycles_bank_canister_status_record.as_mut().unwrap().settings = put_cycles_bank_canister_settings;
            },
            Err(update_settings_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankUpdateSettingsCallError((update_settings_call_error.0 as u32, update_settings_call_error.1))));
            }
        }
    }
    
    if purchase_cycles_bank_data.update_cbsm_user_data_with_the_cb_module_hash == false {
        match call::<(Principal, cbs_map::CBSMUserDataUpdateFields), (cbs_map::UpdateUserResult,)>(
            purchase_cycles_bank_data.cbs_map.as_ref().unwrap().clone(),
            "update_user",
            (user_id, cbs_map::CBSMUserDataUpdateFields{
                cycles_bank_latest_known_module_hash: Some(purchase_cycles_bank_data.cycles_bank_canister_install_code.as_ref().unwrap().clone()),
                ..Default::default()
            })
        ).await {
            Ok((update_user_result,)) => match update_user_result {
                Ok(()) => {
                    purchase_cycles_bank_data.update_cbsm_user_data_with_the_cb_module_hash = true;
                }
                Err(update_user_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::UpdateCBSMUserDataWithTheCBModuleHashError(update_user_error)));
                }
            }
            Err(update_user_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::UpdateCBSMUserDataWithTheCBModuleHashCallError(call_error_as_u32_and_string(update_user_call_error))));
            }
        }
    }
    
    if purchase_cycles_bank_data.collect_icp == false {
        match transfer_user_icp_ledger(
            &user_id, 
            {
                cycles_to_icptokens(MEMBERSHIP_COST_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()) 
                -
                cycles_to_icptokens(NEW_CYCLES_BANK_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap())
            },
            ICP_LEDGER_TRANSFER_DEFAULT_FEE, 
            CTS_PURCHASE_CYCLES_BANK_COLLECT_PAYMENT_ICP_MEMO
        ).await {
            Ok(icp_transfer_result) => match icp_transfer_result {
                Ok(_block_height) => {
                    purchase_cycles_bank_data.collect_icp = true;
                },
                Err(icp_transfer_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CollectIcpTransferError(icp_transfer_error)));          
                }
            }, 
            Err(icp_transfer_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CollectIcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1))));          
            }               
        }
    }
    
    if purchase_cycles_bank_data.transfer_mainder_user_cts_icp_balance.is_none() 
    && purchase_cycles_bank_data.original_user_icp_ledger_balance.unwrap().e8s() 
    > cycles_to_icptokens(MEMBERSHIP_COST_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()).e8s() + (ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 3) 
    {
        let block_height: IcpBlockHeight = match icp_transfer(
            MAINNET_LEDGER_CANISTER_ID,
            IcpTransferArgs {
                memo: IcpMemo(u64::from_be_bytes(*b"pcbtmuci")),
                amount: {
                    IcpTokens::from_e8s(
                        purchase_cycles_bank_data.original_user_icp_ledger_balance.unwrap().e8s()
                        .saturating_sub(
                            cycles_to_icptokens(MEMBERSHIP_COST_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()).e8s() 
                            + (ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 3)
                        )                        
                    )
                },
                fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                from_subaccount: Some(principal_icp_subaccount(&user_id)),
                to: IcpId::new(&purchase_cycles_bank_data.cycles_bank_canister.unwrap(), &ICP_DEFAULT_SUBACCOUNT),
                created_at_time: None, //Some(IcpTimestamp { timestamp_nanos: time() })
            }
        ).await {
            Ok(transfer_result) => match transfer_result {
                Ok(block_height) => block_height,
                Err(transfer_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::TransferMainderUserCTSICPTransferError(transfer_error)));                    
                }
            },
            Err(transfer_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::TransferMainderUserCTSICPTransferCallError(call_error_as_u32_and_string(transfer_call_error))));
            }
        };
        purchase_cycles_bank_data.transfer_mainder_user_cts_icp_balance = Some(block_height);
    }


    with_mut(&CTS_DATA, |cts_data| { 
        cts_data.users_purchase_cycles_bank.remove(&user_id); 
        put_cb_auth(&mut cts_data.cb_auths, UserAndCB{ user_id, cb_id: purchase_cycles_bank_data.cycles_bank_canister.as_ref().unwrap().clone() });
        set_root_hash(cts_data);
        
    });
    
    
    Ok(PurchaseCyclesBankSuccess {
        cycles_bank_canister_id: purchase_cycles_bank_data.cycles_bank_canister.unwrap()
    })
}





// ----------------------------------------------------------------------------------------------------





#[derive(CandidType, Deserialize, Debug)]
pub enum FindCyclesBankError {
    UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: bool },
    FindUserInTheCBSMapsError(FindUserInTheCBSMapsError),
}

#[update]
pub async fn find_cycles_bank() -> Result<Option<Principal>, FindCyclesBankError> {
    //if localkey::get::(&STOP_CALLS) { trap("Maintenance. try again soon."); }

    let user_id: Principal = caller();
    
    with(&CTS_DATA, |cts_data| { 
        match cts_data.users_purchase_cycles_bank.get(&user_id) {
            None => Ok(()),
            Some(purchase_cycles_bank_data) => { 
                return Err(FindCyclesBankError::UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock });    
            }
        }
    })?;
    
    find_cycles_bank_(&user_id).await.map_err(
        |find_user_in_the_cbsms_error| { 
            FindCyclesBankError::FindUserInTheCBSMapsError(find_user_in_the_cbsms_error) 
        }
    )

}



async fn find_cycles_bank_(user_id: &Principal) -> Result<Option<Principal>, FindUserInTheCBSMapsError> {
    if let Some(opt_cb) = with_mut(&CTS_DATA, |cts_data| { cts_data.cb_cache.check(user_id).cloned() }) {
        return Ok(opt_cb);
    } 
    let opt_cb: Option<Principal> = find_user_in_the_cbs_maps(user_id.clone())
        .await?
        .map(|cbsm_user_data_and_cbsm_id| { 
            cbsm_user_data_and_cbsm_id.0.cycles_bank_canister_id 
        });
    with_mut(&CTS_DATA, |cts_data| { 
        cts_data.cb_cache.put(user_id.clone(), opt_cb.clone());
    });    
    Ok(opt_cb) 
} 




// cb-auths



#[derive(CandidType, Deserialize, Debug)]
pub enum SetCBAuthError {
    CBNotFound,
    FindUserInTheCBSMapsError(FindUserInTheCBSMapsError),
}


#[update]
pub async fn set_cb_auth(opt_user_id: Option<Principal>) -> Result<(), SetCBAuthError> {
    let user_id: Principal = opt_user_id.unwrap_or(caller());
    
    match find_cycles_bank_(&user_id).await.map_err(
        |find_user_in_the_cbsms_error| { 
            SetCBAuthError::FindUserInTheCBSMapsError(find_user_in_the_cbsms_error) 
        }
    )? {
        Some(cb_id) => {
            with_mut(&CTS_DATA, |cts_data| {
                put_cb_auth(&mut cts_data.cb_auths, UserAndCB{user_id, cb_id});
                set_root_hash(cts_data);
            });
            Ok(())
        },
        None => {
            // return an error here (don't trap) bc after an await.
            return Err(SetCBAuthError::CBNotFound);
        }
    }   
}

#[query]
pub fn get_cb_auth(cb_id: Principal) -> Vec<u8> {
    get_cb_auth_(UserAndCB{user_id: caller(), cb_id })
}

#[query]
pub fn get_cb_auth_of_an_sns_control(user_id: Principal, cb_id: Principal) -> Vec<u8> {
    get_cb_auth_(UserAndCB{user_id, cb_id })
}





// ----------------------------------------------------------------------------------------------------



#[derive(CandidType, Deserialize, Clone)]
pub struct TransferIcpData{
    start_time_nanos: u64,
    lock: bool,
    transfer_icp_quest: TransferIcpQuest, 
    cts_transfer_icp_fee: Option<IcpTokens>, //:0.05-xdr with the base on the current-rate.
    icp_transfer_block_height: Option<IcpBlockHeight>,
    cts_fee_taken: bool,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct TransferIcpQuest {
    memo: IcpMemo,
    icp: IcpTokens,
    icp_fee: IcpTokens,
    to: IcpId,
}

#[derive(CandidType, Deserialize)]
pub struct TransferIcpSuccess {
    block_height: IcpBlockHeight,
}




#[derive(CandidType, Deserialize)]
pub enum TransferIcpError{
    UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
    CheckIcpBalanceCallError((u32, String)),
    CTSIsBusy,
    CheckCurrentXdrPerMyriadPerIcpCmcRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    MaxTransfer{ cts_transfer_icp_fee: IcpTokens },
    UserIcpLedgerBalanceTooLow{
        user_icp_ledger_balance: IcpTokens,
        cts_transfer_icp_fee: IcpTokens, // calculate by the current xdr-icp rate 
    },
    IcpTransferCallError((u32, String)),
    IcpTransferError(IcpTransferError),
    MidCallError(TransferIcpMidCallError)
}

#[derive(CandidType, Deserialize)]
pub enum TransferIcpMidCallError{
    CollectCTSFeeIcpTransferCallError((u32, String)),
    CollectCTSFeeIcpTransferError(IcpTransferError)
}

#[update]
pub async fn transfer_icp(q: TransferIcpQuest) -> Result<TransferIcpSuccess, TransferIcpError> {

    let user_id: Principal = caller();
        
    let transfer_icp_data = with_mut(&CTS_DATA, |cts_data| {
        check_if_user_is_in_the_middle_of_a_different_call(cts_data, &user_id).map_err(|e| TransferIcpError::UserIsInTheMiddleOfADifferentCall(e))?;
        if cts_data.users_transfer_icp.len() >= MAX_USERS_TRANSFER_ICP {
            return Err(TransferIcpError::CTSIsBusy);
        }
        let transfer_icp_data: TransferIcpData = TransferIcpData{
            start_time_nanos: time(),
            lock: true,
            transfer_icp_quest: q,
            cts_transfer_icp_fee: None,
            icp_transfer_block_height: None,
            cts_fee_taken: false,
        };
        cts_data.users_transfer_icp.insert(user_id, transfer_icp_data.clone());
        Ok(transfer_icp_data)
    })?;
        
    transfer_icp_(user_id, transfer_icp_data).await
}

#[derive(CandidType, Deserialize)]
pub enum CompleteTransferIcpError{
    UserIsNotInTheMiddleOfATransferIcpCall,
    TransferIcpError(TransferIcpError)
}

#[update]
pub async fn complete_transfer_icp() -> Result<TransferIcpSuccess, CompleteTransferIcpError> {
    
    let user_id: Principal = caller();
    
    complete_transfer_icp_(user_id).await

}

async fn complete_transfer_icp_(user_id: Principal) -> Result<TransferIcpSuccess, CompleteTransferIcpError> {
    
    let transfer_icp_data: TransferIcpData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_transfer_icp.get_mut(&user_id) {
            Some(transfer_icp_data) => {
                if transfer_icp_data.lock == true {
                    return Err(CompleteTransferIcpError::TransferIcpError(TransferIcpError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::TransferIcpCall{ must_call_complete: false })));
                }
                transfer_icp_data.lock = true;
                Ok(transfer_icp_data.clone())
            },
            None => {
                return Err(CompleteTransferIcpError::UserIsNotInTheMiddleOfATransferIcpCall);
            }
        }
    })?;

    transfer_icp_(user_id, transfer_icp_data).await
        .map_err(|transfer_icp_error| { 
            CompleteTransferIcpError::TransferIcpError(transfer_icp_error) 
        })
    
}


async fn transfer_icp_(user_id: Principal, mut transfer_icp_data: TransferIcpData) -> Result<TransferIcpSuccess, TransferIcpError> {

    if transfer_icp_data.cts_transfer_icp_fee.is_none() {

        let (
            check_user_icp_ledger_balance_sponse,
            check_current_xdr_permyriad_per_icp_cmc_rate_sponse,
        ): (
            CallResult<IcpTokens>,
            CheckCurrentXdrPerMyriadPerIcpCmcRateSponse
        ) = futures::future::join(
            check_user_icp_ledger_balance(&user_id), 
            check_current_xdr_permyriad_per_icp_cmc_rate()
        ).await; 
        
        let user_icp_ledger_balance: IcpTokens = match check_user_icp_ledger_balance_sponse {
            Ok(tokens) => tokens,
            Err(check_balance_call_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
                return Err(TransferIcpError::CheckIcpBalanceCallError((check_balance_call_error.0 as u32, check_balance_call_error.1)));
            }
        };
                
        let current_xdr_icp_rate: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate_sponse {
            Ok(rate) => rate,
            Err(check_xdr_icp_rate_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
                return Err(TransferIcpError::CheckCurrentXdrPerMyriadPerIcpCmcRateError(check_xdr_icp_rate_error));
            }
        };
        
        let cts_transfer_icp_fee: IcpTokens = cycles_to_icptokens(CTS_TRANSFER_ICP_FEE, current_xdr_icp_rate);
        
        let icp_balance_quirement: IcpTokens = {
            match transfer_icp_data.transfer_icp_quest.icp.e8s().checked_add(cts_transfer_icp_fee.e8s()) {
                None => return Err(TransferIcpError::MaxTransfer{ cts_transfer_icp_fee }),
                Some(t_e8s) => {
                    match t_e8s.checked_add(transfer_icp_data.transfer_icp_quest.icp_fee.e8s() * 2) {
                        None => return Err(TransferIcpError::MaxTransfer{ cts_transfer_icp_fee }),
                        Some(final_e8s) => {
                            IcpTokens::from_e8s(final_e8s)
                        }
                    }
                }
            }
        };
        
        if user_icp_ledger_balance < icp_balance_quirement  {
            with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
            return Err(TransferIcpError::UserIcpLedgerBalanceTooLow{
                user_icp_ledger_balance,
                cts_transfer_icp_fee,
            });
        }
        
        transfer_icp_data.cts_transfer_icp_fee = Some(cts_transfer_icp_fee);
    }
    
    if transfer_icp_data.icp_transfer_block_height.is_none() {
        
        match icp_transfer(
            MAINNET_LEDGER_CANISTER_ID,
            IcpTransferArgs {
                memo: transfer_icp_data.transfer_icp_quest.memo,
                amount: transfer_icp_data.transfer_icp_quest.icp,
                fee: transfer_icp_data.transfer_icp_quest.icp_fee,
                from_subaccount: Some(principal_icp_subaccount(&user_id)),
                to: transfer_icp_data.transfer_icp_quest.to,
                created_at_time: Some(IcpTimestamp { timestamp_nanos: time() - 1_000_000_000 })
            }
        ).await {
            Ok(transfer_result) => match transfer_result {
                Ok(block_height) => {
                    transfer_icp_data.icp_transfer_block_height = Some(block_height);
                },
                Err(icp_transfer_error) => {
                    with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
                    return Err(TransferIcpError::IcpTransferError(icp_transfer_error));                    
                }
            },
            Err(icp_transfer_call_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
                return Err(TransferIcpError::IcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1)));
            }
        }
        
    }
    
    if transfer_icp_data.cts_fee_taken == false {
        match transfer_user_icp_ledger(&user_id, transfer_icp_data.cts_transfer_icp_fee.unwrap(), transfer_icp_data.transfer_icp_quest.icp_fee, CTS_TRANSFER_ICP_FEE_ICP_MEMO).await {
            Ok(icp_transfer_result) => match icp_transfer_result {
                Ok(_block_height) => {
                    transfer_icp_data.cts_fee_taken = true;
                },
                Err(icp_transfer_error) => {
                    transfer_icp_data.lock = false;
                    with_mut(&CTS_DATA, |cts_data| {
                        if let Some(data) = cts_data.users_transfer_icp.get_mut(&user_id) {
                            *data = transfer_icp_data;
                        }
                    });
                    return Err(TransferIcpError::MidCallError(TransferIcpMidCallError::CollectCTSFeeIcpTransferError(icp_transfer_error)));          
                }
            }, 
            Err(icp_transfer_call_error) => {
                transfer_icp_data.lock = false;
                with_mut(&CTS_DATA, |cts_data| {
                    if let Some(data) = cts_data.users_transfer_icp.get_mut(&user_id) {
                        *data = transfer_icp_data;
                    }
                });
                return Err(TransferIcpError::MidCallError(TransferIcpMidCallError::CollectCTSFeeIcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1))));                
            }               
        }
    }
    
    
    with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
    Ok(TransferIcpSuccess {
        block_height: transfer_icp_data.icp_transfer_block_height.unwrap(),
    })
}






// ------------
// LENGTHEN-MEMBERSHIP






// multi-step, mid-call-data, collect icp and call cbs-map and cycles-bank,
// complete fn



// options are for the memberance of the steps

#[derive(CandidType, Deserialize, Clone)]
pub struct LengthenMembershipMidCallData {
    start_time_nanos: u64,
    lock: bool,
    lengthen_membership_quest: LengthenMembershipQuest, 
    cbsm_user_data_and_cbsm_id: Option<(CBSMUserData, Principal)>,
    posit_ctsfuel_into_the_cycles_bank: bool, // use in the lengthen_membership_cb_cycles_payment method 
    xdr_permyriad_per_icp_rate: Option<u64>,
    cmc_topup_cycles_icp_ledger_transfer_block_height: Option<u64>,
    cmc_topup_cycles: Option<Cycles>,
    collect_icp_block_height: Option<u64>,
    update_cbsm: bool,
    call_cycles_bank_update_membership_length: bool,
}

impl LengthenMembershipMidCallData {
    fn new_lifetime_termination_timestamp_seconds(&self) -> Option<u128>/*none if mid_call_data.cbsm_user_data_and_cbsm_id is none*/ {
        match self.cbsm_user_data_and_cbsm_id {
            None => None,
            Some(ref cbsm_user_data_and_cbsm_id) => {
                Some(
                    core::cmp::max(    
                        cbsm_user_data_and_cbsm_id.0.cycles_bank_lifetime_termination_timestamp_seconds, 
                        self.start_time_nanos as u128 / NANOS_IN_A_SECOND,
                    )  
                    + SECONDS_IN_A_DAY * 365 * self.lengthen_membership_quest.lengthen_years 
                )
            }
        }
    }
}






#[derive(CandidType, Deserialize, Debug)]
pub enum LengthenMembershipError {
    LengthenYearsCannotBeZero,
    UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
    CTSIsBusy,
    MembershipNotFound,
    FindUserInTheCBSMapsError(FindUserInTheCBSMapsError),
    CallerIsNotTheCyclesBankOfTheUser, // for when the lengthen_membership_cb_cycles_payment method
    CheckIcpBalanceCallError(CallError),
    CheckCurrentXdrPerMyriadPerIcpCmcRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    UserIcpLedgerBalanceTooLow{
        membership_cost_per_year_cycles: Cycles,
        current_xdr_permyriad_per_icp_rate: u64, 
        icp_ledger_transfer_fee: IcpTokens,
        user_icp_ledger_balance: IcpTokens,
        /*user_icp_ledger_balance needs cycles_to_icp(membership_cost_per_year_cycles * lengthen_years, current_xdr_permyriad_per_icp_rate) + IcpTokens.from_e8s(1) + icp_ledger_transfer_fee * 2*/
    },
    MidCallError(LengthenMembershipMidCallError),
}


#[derive(CandidType, Deserialize, Debug)]
pub enum LengthenMembershipMidCallError {
    PositCyclesIntoTheCyclesBankCallError(CallError), // for the cb_cycles_payment method
    LedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
    LedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
    CollectIcpTransferCallError(CallError),
    CollectIcpTransferError(IcpTransferError),
    CBSMUpdateUserCallError(CallError),
    CBUpdateMembershipLengthCallError(CallError)
}

#[derive(CandidType, Deserialize)]
pub struct LengthenMembershipSuccess {
    lifetime_termination_timestamp_seconds: u128
}






#[update]
pub async fn lengthen_membership(q: LengthenMembershipQuest) -> Result<LengthenMembershipSuccess, LengthenMembershipError> {
    
    let user_id: Principal = caller(); 
        
    if q.lengthen_years == 0 {
        return Err(LengthenMembershipError::LengthenYearsCannotBeZero);        
    }
    
    let mid_call_data: LengthenMembershipMidCallData = with_mut(&CTS_DATA, |cts_data| {
        check_if_user_is_in_the_middle_of_a_different_call(cts_data, &user_id).map_err(|e| LengthenMembershipError::UserIsInTheMiddleOfADifferentCall(e))?;
        if cts_data.users_lengthen_membership.len() >= MAX_USERS_LENGTHEN_MEMBERSHIP {
            return Err(LengthenMembershipError::CTSIsBusy);
        }
        let lengthen_membership_mid_call_data: LengthenMembershipMidCallData = LengthenMembershipMidCallData{
            start_time_nanos: time(),
            lock: true,
            lengthen_membership_quest: q, 
            cbsm_user_data_and_cbsm_id: None,
            posit_ctsfuel_into_the_cycles_bank: false,
            xdr_permyriad_per_icp_rate: None,
            cmc_topup_cycles_icp_ledger_transfer_block_height: None,
            cmc_topup_cycles: None,
            collect_icp_block_height: None,
            update_cbsm: false,
            call_cycles_bank_update_membership_length: false,
        };
        cts_data.users_lengthen_membership.insert(user_id.clone(), lengthen_membership_mid_call_data.clone());
        Ok(lengthen_membership_mid_call_data) 
    })?; 
    
    

    lengthen_membership_(user_id, mid_call_data).await
    
}


async fn lengthen_membership_(user_id: Principal, mut mid_call_data: LengthenMembershipMidCallData) -> Result<LengthenMembershipSuccess, LengthenMembershipError> {
    

fn lengthen_membership_remove_user(cts_data: &mut CTSData, user: &Principal) {
    cts_data.users_lengthen_membership.remove(user);
}
fn lengthen_membership_unlock_and_write_user(cts_data: &mut CTSData, user: &Principal, mut mid_call_data: LengthenMembershipMidCallData) {
    mid_call_data.lock = false;
    cts_data.users_lengthen_membership.insert(user.clone(), mid_call_data);
}
    

    // look for user in the cbsms
    if mid_call_data.cbsm_user_data_and_cbsm_id.is_none() {
        
        let cbsm_user_data_and_cbsm_id: (CBSMUserData, Principal) = match find_user_in_the_cbs_maps(user_id).await {
            Ok(opt_data) => match opt_data {
                Some(cbsm_user_data_and_cbsm_id) => cbsm_user_data_and_cbsm_id,
                None => {
                    with_mut(&CTS_DATA, |cts_data| { lengthen_membership_remove_user(cts_data, &user_id) });
                    return Err(LengthenMembershipError::MembershipNotFound);
                }
            },
            Err(find_user_in_the_cbsms_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_remove_user(cts_data, &user_id) });
                return Err(LengthenMembershipError::FindUserInTheCBSMapsError(find_user_in_the_cbsms_error));
            }
        };
        
        mid_call_data.cbsm_user_data_and_cbsm_id = Some(cbsm_user_data_and_cbsm_id);
    }
    
    if mid_call_data.xdr_permyriad_per_icp_rate.is_none() {
        let (
            check_user_icp_ledger_balance_sponse,
            check_current_xdr_permyriad_per_icp_cmc_rate_sponse,
        ): (
            CallResult<IcpTokens>,
            CheckCurrentXdrPerMyriadPerIcpCmcRateSponse
        ) = futures::future::join(
            check_user_icp_ledger_balance(&user_id), 
            check_current_xdr_permyriad_per_icp_cmc_rate()
        ).await; 
        
        let user_icp_ledger_balance: IcpTokens = match check_user_icp_ledger_balance_sponse {
            Ok(tokens) => tokens,
            Err(check_balance_call_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_remove_user(cts_data, &user_id) });
                return Err(LengthenMembershipError::CheckIcpBalanceCallError((check_balance_call_error.0 as u32, check_balance_call_error.1)));
            }
        };
                
        let xdr_permyriad_per_icp_rate: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate_sponse {
            Ok(rate) => rate,
            Err(check_xdr_icp_rate_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_remove_user(cts_data, &user_id) });
                return Err(LengthenMembershipError::CheckCurrentXdrPerMyriadPerIcpCmcRateError(check_xdr_icp_rate_error));
            }
        };
        
        let lengthen_membership_total_cost_icp: IcpTokens = {
            IcpTokens::from_e8s(
                cycles_to_icptokens(MEMBERSHIP_COST_CYCLES.saturating_mul(mid_call_data.lengthen_membership_quest.lengthen_years), xdr_permyriad_per_icp_rate)
                .e8s() 
                .saturating_add(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2)
            )
        }; 
        
        if user_icp_ledger_balance < lengthen_membership_total_cost_icp {
            with_mut(&CTS_DATA, |cts_data| { lengthen_membership_remove_user(cts_data, &user_id) });
            return Err(LengthenMembershipError::UserIcpLedgerBalanceTooLow{
                membership_cost_per_year_cycles: MEMBERSHIP_COST_CYCLES,
                current_xdr_permyriad_per_icp_rate: xdr_permyriad_per_icp_rate,
                icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                user_icp_ledger_balance,
            });
        }   
        
        mid_call_data.xdr_permyriad_per_icp_rate = Some(xdr_permyriad_per_icp_rate);
    }
    
    
    
    let user_payment_icp: IcpTokens = {
        cycles_to_icptokens(
            MEMBERSHIP_COST_CYCLES.saturating_mul(mid_call_data.lengthen_membership_quest.lengthen_years), 
            mid_call_data.xdr_permyriad_per_icp_rate.as_ref().unwrap().clone()
        )
    };
    
    // put cycles onto the cycles-bank for the membership-duration
    if mid_call_data.cmc_topup_cycles_icp_ledger_transfer_block_height.is_none() {
        match ledger_topup_cycles_cmc_icp_transfer(
            IcpTokens::from_e8s(user_payment_icp.e8s() / 2),
            ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            Some(principal_icp_subaccount(&user_id)),
            mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.cycles_bank_canister_id,
        ).await {
            Ok(block_height) => {
                mid_call_data.cmc_topup_cycles_icp_ledger_transfer_block_height = Some(block_height);
            },
            Err(ledger_topup_cycles_cmc_icp_transfer_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_unlock_and_write_user(cts_data, &user_id, mid_call_data) });
                return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::LedgerTopupCyclesCmcIcpTransferError(ledger_topup_cycles_cmc_icp_transfer_error)));
            }
        }
    }
        
    if mid_call_data.cmc_topup_cycles.is_none() {
        match ledger_topup_cycles_cmc_notify(
            mid_call_data.cmc_topup_cycles_icp_ledger_transfer_block_height.as_ref().unwrap().clone(),
            mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.cycles_bank_canister_id
        ).await {
            Ok(cycles) => {
                mid_call_data.cmc_topup_cycles = Some(cycles);
            },
            Err(ledger_topup_cycles_cmc_notify_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_unlock_and_write_user(cts_data, &user_id, mid_call_data) });
                return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::LedgerTopupCyclesCmcNotifyError(ledger_topup_cycles_cmc_notify_error)));
            }
        }
    }   
    
    // collect mainder of the user_payment_icp
    if mid_call_data.collect_icp_block_height.is_none() {
        match transfer_user_icp_ledger(
            &user_id,
            IcpTokens::from_e8s(user_payment_icp.e8s() - (user_payment_icp.e8s() / 2)),
            ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            IcpMemo(0),
        ).await {
            Ok(transfer_result) => match transfer_result {
                Ok(block_height) => {
                    mid_call_data.collect_icp_block_height = Some(block_height);
                },
                Err(icp_transfer_error) => {
                    with_mut(&CTS_DATA, |cts_data| { lengthen_membership_unlock_and_write_user(cts_data, &user_id, mid_call_data) });
                    return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::CollectIcpTransferError(icp_transfer_error)));                    
                }
            },
            Err(icp_transfer_call_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_unlock_and_write_user(cts_data, &user_id, mid_call_data) });
                return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::CollectIcpTransferCallError(call_error_as_u32_and_string(icp_transfer_call_error))));
            }
        }
    }
    
    finish_lengthen_membership_update_cycles_bank_and_update_cbsm_(
        user_id, 
        mid_call_data, 
        lengthen_membership_unlock_and_write_user,
        lengthen_membership_remove_user
    ).await
}
    
    
async fn finish_lengthen_membership_update_cycles_bank_and_update_cbsm_(
    user_id: Principal, 
    mut mid_call_data: LengthenMembershipMidCallData,
    unlock_and_write_user: impl Fn(&mut CTSData, &Principal, LengthenMembershipMidCallData) -> (), 
    remove_user: impl Fn(&mut CTSData, &Principal) -> (),
) -> Result<LengthenMembershipSuccess, LengthenMembershipError> {
    // look cycles-bank is with a module or uninstalled/empty
     
    if mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.membership_termination_cb_uninstall_data.is_some() {
        match {
            with(&CTS_DATA, |cts_data| {
                call_raw128(
                    Principal::management_canister(),
                    "install_code",
                    encode_one(
                        ManagementCanisterInstallCodeQuest {
                            mode : ManagementCanisterInstallCodeMode::install,
                            canister_id : mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.cycles_bank_canister_id,
                            wasm_module : cts_data.cycles_bank_canister_code.module(),
                            arg : &encode_one(
                                &CyclesBankInit{
                                    user_id: user_id,
                                    cts_id: ic_cdk::api::id(),
                                    cbsm_id: mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().1,
                                    storage_size_mib: NEW_CYCLES_BANK_STORAGE_SIZE_MiB,                         
                                    lifetime_termination_timestamp_seconds: mid_call_data.new_lifetime_termination_timestamp_seconds().unwrap(),
                                    start_with_user_cycles_balance: mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.membership_termination_cb_uninstall_data.as_ref().unwrap().user_cycles_balance,
                                    sns_control: mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.sns_control,
                                }
                            ).unwrap(),
                        }       
                    ).unwrap(),
                    0,
                )  
            })   
        }.await {
            Ok(_) => {
                mid_call_data.cbsm_user_data_and_cbsm_id.as_mut().unwrap().0.membership_termination_cb_uninstall_data = None;
                mid_call_data.call_cycles_bank_update_membership_length = true; // for the lack of a need to call the cb to update the membership lifetime
            },
            Err(call_error) => {
                with_mut(&CTS_DATA, |cts_data| { unlock_and_write_user(cts_data, &user_id, mid_call_data); });
                return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::CBSMUpdateUserCallError(call_error_as_u32_and_string(call_error))));
            }
        }
    } 
    
    if mid_call_data.call_cycles_bank_update_membership_length == false {
        match call::<(u128,),()>(
            mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.cycles_bank_canister_id,
            "cts_update_lifetime_termination_timestamp_seconds",
            (mid_call_data.new_lifetime_termination_timestamp_seconds().unwrap(),),
        ).await {
            Ok(()) => {
                mid_call_data.call_cycles_bank_update_membership_length = true;
            },
            Err(call_error) => {
                with_mut(&CTS_DATA, |cts_data| { unlock_and_write_user(cts_data, &user_id, mid_call_data); });
                return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::CBUpdateMembershipLengthCallError(call_error_as_u32_and_string(call_error))));
            }
        }
    }
     
                                                                                                
    if mid_call_data.update_cbsm == false {
        match call::<(Principal/*user-id*/, cbs_map::CBSMUserDataUpdateFields), (Result<(), cbs_map::UpdateUserError>,)>(
            mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().1,
            "update_user",
            (
                user_id,
                cbs_map::CBSMUserDataUpdateFields{
                    cycles_bank_lifetime_termination_timestamp_seconds: Some(mid_call_data.new_lifetime_termination_timestamp_seconds().unwrap()),
                    ..Default::default()
                } 
            )
        ).await {
            Ok((update_user_result,)) => match update_user_result {
                Ok(()) => {                        
                    mid_call_data.update_cbsm = true;
                },
                Err(update_user_error) => match update_user_error {
                    cbs_map::UpdateUserError::UserNotFound => {
                        // whaaa
                    } 
                }
            },
            Err(call_error) => {
                with_mut(&CTS_DATA, |cts_data| { unlock_and_write_user(cts_data, &user_id, mid_call_data); });
                return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::CBSMUpdateUserCallError(call_error_as_u32_and_string(call_error))));
            }
        }
    }
    
    
    with_mut(&CTS_DATA, |cts_data| { remove_user(cts_data, &user_id); });
    Ok(LengthenMembershipSuccess{
        lifetime_termination_timestamp_seconds: mid_call_data.new_lifetime_termination_timestamp_seconds().unwrap()
    })    
    
}




#[derive(CandidType, Deserialize)]
pub enum CompleteLengthenMembershipError{
    UserIsNotInTheMiddleOfALengthenMembershipCall,
    LengthenMembershipError(LengthenMembershipError)
}

#[update]
pub async fn complete_lengthen_membership() -> Result<LengthenMembershipSuccess, CompleteLengthenMembershipError> {

    let user_id: Principal = caller(); 
    
    complete_lengthen_membership_(user_id).await

}


async fn complete_lengthen_membership_(user_id: Principal) -> Result<LengthenMembershipSuccess, CompleteLengthenMembershipError> {
    
    let lengthen_membership_mid_call_data: LengthenMembershipMidCallData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_lengthen_membership.get_mut(&user_id) {
            Some(lengthen_membership_mid_call_data) => {
                if lengthen_membership_mid_call_data.lock == true {
                    return Err(CompleteLengthenMembershipError::LengthenMembershipError(LengthenMembershipError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::LengthenMembershipCall{ must_call_complete: false })));
                }
                lengthen_membership_mid_call_data.lock = true;
                Ok(lengthen_membership_mid_call_data.clone())
            },
            None => {
                return Err(CompleteLengthenMembershipError::UserIsNotInTheMiddleOfALengthenMembershipCall);
            }
        }
    })?;

    lengthen_membership_(user_id, lengthen_membership_mid_call_data).await
        .map_err(|lengthen_membership_error| { 
            CompleteLengthenMembershipError::LengthenMembershipError(lengthen_membership_error) 
        })
        
}


// ---------





fn cycles_as_tcycles(c: Cycles) -> u128 {
    c / 1_000_000_000_000
}



#[update]
pub async fn lengthen_membership_cb_cycles_payment(q: LengthenMembershipQuest, user_id: Principal) -> Result<LengthenMembershipSuccess, LengthenMembershipError> {
    let caller_cycles_bank_id: Principal = caller();
    
    let msg_cycles_quirement = MEMBERSHIP_COST_CYCLES.saturating_mul(q.lengthen_years);
    if msg_cycles_available128() < msg_cycles_quirement {
        trap(&format!(
            "Membership cost per year is: {}T cycles. For {} years, that is {}T cycles total. Cycles in the call: {}T", 
            cycles_as_tcycles(MEMBERSHIP_COST_CYCLES),
            q.lengthen_years,
            cycles_as_tcycles(msg_cycles_quirement),
            cycles_as_tcycles(msg_cycles_available128())
        ));
    } 
    
    if q.lengthen_years == 0 {
        return Err(LengthenMembershipError::LengthenYearsCannotBeZero);        
    }
    
    let mut mid_call_data: LengthenMembershipMidCallData = with_mut(&CTS_DATA, |cts_data| {
        check_if_user_is_in_the_middle_of_a_different_call(cts_data, &user_id).map_err(|e| LengthenMembershipError::UserIsInTheMiddleOfADifferentCall(e))?;
        if cts_data.users_lengthen_membership_cb_cycles_payment.len() >= MAX_USERS_LENGTHEN_MEMBERSHIP_CB_CYCLES_PAYMENT {
            return Err(LengthenMembershipError::CTSIsBusy);
        }
        let lengthen_membership_mid_call_data: LengthenMembershipMidCallData = LengthenMembershipMidCallData{
            start_time_nanos: time(),
            lock: true,
            lengthen_membership_quest: q, 
            cbsm_user_data_and_cbsm_id: None,
            posit_ctsfuel_into_the_cycles_bank: false,
            xdr_permyriad_per_icp_rate: None,
            cmc_topup_cycles_icp_ledger_transfer_block_height: None,
            cmc_topup_cycles: None,
            collect_icp_block_height: None,
            update_cbsm: false,
            call_cycles_bank_update_membership_length: false,
        };
        cts_data.users_lengthen_membership_cb_cycles_payment.insert(user_id.clone(), lengthen_membership_mid_call_data.clone());
        Ok(lengthen_membership_mid_call_data) 
    })?;
    
    
    fn lengthen_membership_cb_cycles_payment_remove_user(cts_data: &mut CTSData, user: &Principal) {
        cts_data.users_lengthen_membership_cb_cycles_payment.remove(user);
    }
    
    if mid_call_data.cbsm_user_data_and_cbsm_id.is_none() {
        
        let cbsm_user_data_and_cbsm_id: (CBSMUserData, Principal) = match find_user_in_the_cbs_maps(user_id).await {
            Ok(opt_data) => match opt_data {
                Some(cbsm_user_data_and_cbsm_id) => cbsm_user_data_and_cbsm_id,
                None => {
                    with_mut(&CTS_DATA, |cts_data| { lengthen_membership_cb_cycles_payment_remove_user(cts_data, &user_id) });
                    return Err(LengthenMembershipError::MembershipNotFound);
                }
            },
            Err(find_user_in_the_cbsms_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_cb_cycles_payment_remove_user(cts_data, &user_id) });
                return Err(LengthenMembershipError::FindUserInTheCBSMapsError(find_user_in_the_cbsms_error));
            }
        };
        
        if cbsm_user_data_and_cbsm_id.0.cycles_bank_canister_id != caller_cycles_bank_id {
            with_mut(&CTS_DATA, |cts_data| { lengthen_membership_cb_cycles_payment_remove_user(cts_data, &user_id) });
            return Err(LengthenMembershipError::CallerIsNotTheCyclesBankOfTheUser);
        }
        
        mid_call_data.cbsm_user_data_and_cbsm_id = Some(cbsm_user_data_and_cbsm_id);
    }
    
    
    
    msg_cycles_accept128(msg_cycles_quirement);
    
    lengthen_membership_cb_cycles_payment_(user_id, mid_call_data).await
    
}


async fn lengthen_membership_cb_cycles_payment_(user_id: Principal, mut mid_call_data: LengthenMembershipMidCallData) -> Result<LengthenMembershipSuccess, LengthenMembershipError> {
    

    fn lengthen_membership_cb_cycles_payment_unlock_and_write_user(cts_data: &mut CTSData, user: &Principal, mut mid_call_data: LengthenMembershipMidCallData) {
        mid_call_data.lock = false;
        cts_data.users_lengthen_membership_cb_cycles_payment.insert(user.clone(), mid_call_data);
    }
    fn lengthen_membership_cb_cycles_payment_remove_user(cts_data: &mut CTSData, user: &Principal) {
        cts_data.users_lengthen_membership_cb_cycles_payment.remove(user);
    }
    
    if mid_call_data.posit_ctsfuel_into_the_cycles_bank == false {
        match call_with_payment128::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "deposit_cycles",
            (CanisterIdRecord{ canister_id: mid_call_data.cbsm_user_data_and_cbsm_id.as_ref().unwrap().0.cycles_bank_canister_id, },),
            mid_call_data.lengthen_membership_quest.lengthen_years.saturating_mul(MEMBERSHIP_COST_CYCLES) / 2
        ).await {
            Ok(()) => {
                mid_call_data.posit_ctsfuel_into_the_cycles_bank = true;         
            },
            Err(call_error) => {
                with_mut(&CTS_DATA, |cts_data| { lengthen_membership_cb_cycles_payment_unlock_and_write_user(cts_data, &user_id, mid_call_data) });
                return Err(LengthenMembershipError::MidCallError(LengthenMembershipMidCallError::PositCyclesIntoTheCyclesBankCallError(call_error_as_u32_and_string(call_error))));
            }
        }
    }
    
    finish_lengthen_membership_update_cycles_bank_and_update_cbsm_(
        user_id, 
        mid_call_data,
        lengthen_membership_cb_cycles_payment_unlock_and_write_user,
        lengthen_membership_cb_cycles_payment_remove_user,
    ).await
    
}



#[update]
pub async fn complete_lengthen_membership_cb_cycles_payment() -> Result<LengthenMembershipSuccess, CompleteLengthenMembershipError> {

    let user_id: Principal = caller(); 
    
    complete_lengthen_membership_cb_cycles_payment_(user_id).await

}


async fn complete_lengthen_membership_cb_cycles_payment_(user_id: Principal) -> Result<LengthenMembershipSuccess, CompleteLengthenMembershipError> {
    
    let lengthen_membership_mid_call_data: LengthenMembershipMidCallData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_lengthen_membership_cb_cycles_payment.get_mut(&user_id) {
            Some(lengthen_membership_mid_call_data) => {
                if lengthen_membership_mid_call_data.lock == true {
                    return Err(CompleteLengthenMembershipError::LengthenMembershipError(LengthenMembershipError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::LengthenMembershipCBCyclesPaymentCall{ must_call_complete: false })));
                }
                lengthen_membership_mid_call_data.lock = true;
                Ok(lengthen_membership_mid_call_data.clone())
            },
            None => {
                return Err(CompleteLengthenMembershipError::UserIsNotInTheMiddleOfALengthenMembershipCall);
            }
        }
    })?;

    lengthen_membership_cb_cycles_payment_(user_id, lengthen_membership_mid_call_data).await
        .map_err(|lengthen_membership_error| { 
            CompleteLengthenMembershipError::LengthenMembershipError(lengthen_membership_error) 
        })
        
}





// --------------------------------------------------------------------------
// :CONTROLLER-METHODS.





// ----- USERS_MAP_CANISTERS-METHODS --------------------------



#[update]
pub fn controller_put_umc_code(canister_code: CanisterCode) -> () {
    caller_is_controller_gaurd(&caller());
    
    if sha256(canister_code.module()) != *canister_code.module_hash() {
        trap("Given canister_code.module_hash is different than the manual compute module hash");
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.cbs_map_canister_code = canister_code;
    });
}




// certification? or replication-calls?
#[export_name = "canister_query controller_view_cbsms"]
pub fn controller_view_cbsms() {
    caller_is_controller_gaurd(&caller());
    
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<&Principal>,)>((cts_data.cbs_maps.keys().collect(),));
    });
}



pub type ControllerUpgradeUMCError = (Principal, ControllerUpgradeUMCCallErrorType, (u32, String)); 

#[derive(CandidType, Deserialize)]
pub enum ControllerUpgradeUMCCallErrorType {
    StopCanisterCallError,
    UpgradeCodeCallError,
    StartCanisterCallError
}

#[update]
pub async fn controller_upgrade_cbsms(q: ControllerUpgradeCSQuest) -> Vec<(Principal, UpgradeOutcome)> {
    caller_is_controller_gaurd(&caller());
    
    let cc: CanisterCode = with_mut(&CTS_DATA, |cts_data| {
        if let Some(new_canister_code) = q.new_canister_code {
            new_canister_code.verify_module_hash().unwrap();
            cts_data.cbs_map_canister_code = new_canister_code; 
        }
        cts_data.cbs_map_canister_code.clone()
    });
    
    let cbsms: Vec<Principal> = match q.specific_cs {
        Some(specific_cbsms) => {
            with(&CTS_DATA, |cts_data| {
                for cbsm in specific_cbsms.iter() { 
                    if cts_data.cbs_maps.contains_key(cbsm) == false {
                        trap(&format!("cts cbs_maps does not contain: {}", cbsm));
                    }
                }
            });
            specific_cbsms.into_iter().collect()
        }
        None => {
            with(&CTS_DATA, |cts_data| {
                cts_data.cbs_maps.iter()
                .filter_map(|(cbsm, cbsm_status)| {
                    if &cbsm_status.module_hash != cc.module_hash() {
                        Some(cbsm.clone())
                    } else {
                        None
                    }
                })
                .take(200)
                .collect()
            })
        }
    };
    
    let rs: Vec<(Principal, UpgradeOutcome)> = upgrade_canisters(cbsms, &cc, &q.post_upgrade_quest).await;
    
    with_mut(&CTS_DATA, |cts_data| {
        for (cbsm, uo) in rs.iter() {
            if let Some(ref r) = uo.install_code_result {
                if r.is_ok() {
                    if let Some(cbsm_status) = cts_data.cbs_maps.get_mut(cbsm) {
                        cbsm_status.module_hash = cc.module_hash().clone();
                    } else {
                        ic_cdk::print("check this");
                    } 
                }
            }
        } 
    });
    
    return rs;
    
}









// ----- CYCLES_BANKS-METHODS --------------------------


#[update]
pub fn controller_put_cycles_bank_canister_code(canister_code: CanisterCode) -> () {
    caller_is_controller_gaurd(&caller());
    
    if sha256(canister_code.module()) != *canister_code.module_hash() {
        trap("Given canister_code.module_hash is different than the manual compute module hash");
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.cycles_bank_canister_code = canister_code;
    });
}



pub type ControllerPutUCCodeOntoTheUMCError = (Principal, CallError);

#[update]
pub async fn controller_put_uc_code_onto_the_umcs(opt_umcs: Option<Vec<Principal>>) -> Vec<ControllerPutUCCodeOntoTheUMCError>/*umcs that the put_uc_code call fail. if empty, means every call is success*/ {
    caller_is_controller_gaurd(&caller());
        
    if with(&CTS_DATA, |cts_data| cts_data.cycles_bank_canister_code.module().len() == 0 ) {
        trap("CYCLES_BANK_CODE.module().len() is 0.")
    }
    
    let call_umcs: Vec<Principal> = {
        if let Some(call_umcs) = opt_umcs {
            with(&CTS_DATA, |cts_data| { 
                for call_umc in call_umcs.iter() { 
                    if cts_data.cbs_maps.contains_key(call_umc) == false {
                        trap(&format!("cts cbs_maps does not contain: {:?}", call_umc));
                    }
                }
            });    
            call_umcs
        } else {
            with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.keys().cloned().collect() })
        }
    };    
    
    let cc: CanisterCode = with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.clone() });
    
    async fn call_umc_fn(call_umc: Principal, cc: &CanisterCode) -> Result<(), ControllerPutUCCodeOntoTheUMCError> {
        match call::<(&CanisterCode,), ()>(
            call_umc.clone(),
            "cts_put_user_canister_code",
            (cc,)
        ).await {
            Ok(_) => Ok(()),
            Err(call_error) => Err((call_umc, call_error_as_u32_and_string(call_error))),
        }
    }
    
    let sponses: Vec<Result<(), ControllerPutUCCodeOntoTheUMCError>> = futures::future::join_all(call_umcs.into_iter().map(|umc| call_umc_fn(umc, &cc))).await;
    
    sponses.into_iter().filter_map(
        |call_umc_sponse: Result<(), ControllerPutUCCodeOntoTheUMCError>| {
            match call_umc_sponse {
                Ok(()) => None,
                Err(call_umc_error) => Some(call_umc_error)
            }
        }
    ).collect::<Vec<ControllerPutUCCodeOntoTheUMCError>>()
}



#[update]
pub async fn controller_upgrade_cbsm_cbs_chunk(cbsm: Principal, q: ControllerUpgradeCSQuest) -> Result<Vec<(Principal, UpgradeOutcome)>, CallError> {
    caller_is_controller_gaurd(&caller());
    
    if with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.contains_key(&cbsm) }) == false {
        trap(&format!("cts cbs_maps does not contain {}", cbsm));
    }
    
    if let Some(ref new_cc) = q.new_canister_code {
        new_cc.verify_module_hash().unwrap();
        with_mut(&CTS_DATA, |cts_data| {
            cts_data.cycles_bank_canister_code = new_cc.clone();
        });            
    }
    
    call::<(ControllerUpgradeCSQuest,), (Vec<(Principal, UpgradeOutcome)>,)>(
        cbsm,
        "controller_upgrade_cbs_chunk",
        (q,)        
    )
    .await
    .map(|t| t.0)
    .map_err(call_error_as_u32_and_string)
}






// ----- PURCHASE_CYCLES_BANK-METHODS --------------------------

#[export_name = "canister_query controller_view_users_purchase_cycles_bank"]
pub fn controller_view_users_purchase_cycles_bank() {
    caller_is_controller_gaurd(&caller());
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &PurchaseCyclesBankData)>,)>((cts_data.users_purchase_cycles_bank.iter().collect::<Vec<(&Principal, &PurchaseCyclesBankData)>>(),));
    });
}

// put new user data
#[update]
pub fn controller_put_purchase_cycles_bank_data(new_user_id: Principal, put_data: PurchaseCyclesBankData, override_lock: bool) {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(purchase_cycles_bank_data) = cts_data.users_purchase_cycles_bank.get(&new_user_id) {
            if purchase_cycles_bank_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_purchase_cycles_bank. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_purchase_cycles_bank.insert(new_user_id, put_data);
    });

}
// remove new user
#[update]
pub fn controller_remove_new_user(new_user_id: Principal, override_lock: bool) {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(purchase_cycles_bank_data) = cts_data.users_purchase_cycles_bank.get(&new_user_id) {
            if purchase_cycles_bank_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_purchase_cycles_bank. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_purchase_cycles_bank.remove(&new_user_id);
    });
}


#[update]
pub async fn controller_complete_users_purchase_cycles_bank(opt_complete_users_purchase_cycles_bank_ids: Option<Vec<Principal>>) -> Vec<(Principal, Result<PurchaseCyclesBankSuccess, CompletePurchaseCyclesBankError>)> {
    caller_is_controller_gaurd(&caller());

    let complete_users_purchase_cycles_bank_ids: Vec<Principal> = match opt_complete_users_purchase_cycles_bank_ids {
        Some(complete_users_purchase_cycles_bank_ids) => complete_users_purchase_cycles_bank_ids,
        None => {
            with(&CTS_DATA, |cts_data| { 
                cts_data.users_purchase_cycles_bank.iter()
                .filter(|&(_user_id, purchase_cycles_bank_data): &(&Principal, &PurchaseCyclesBankData)| {
                    purchase_cycles_bank_data.lock == false
                })
                .map(|(user_id, _purchase_cycles_bank_data): (&Principal, &PurchaseCyclesBankData)| {
                    user_id.clone()
                })
                .collect::<Vec<Principal>>() 
            })
        }
    };
    
    let rs: Vec<Result<PurchaseCyclesBankSuccess, CompletePurchaseCyclesBankError>> = futures::future::join_all(
        complete_users_purchase_cycles_bank_ids.iter().map(
            |complete_new_user_id: &Principal| {
                complete_purchase_cycles_bank_(complete_new_user_id.clone())
            }
        ).collect::<Vec<_>>()
    ).await;
    
    complete_users_purchase_cycles_bank_ids.into_iter().zip(rs.into_iter()).collect::<Vec<(Principal, Result<PurchaseCyclesBankSuccess,CompletePurchaseCyclesBankError>)>>()
    
}




// ------ UsersTransferIcp-METHODS -----------------


#[export_name = "canister_query controller_view_users_transfer_icp"]
pub fn controller_view_users_transfer_icp() {
    caller_is_controller_gaurd(&caller());
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &TransferIcpData)>,)>((cts_data.users_transfer_icp.iter().collect::<Vec<(&Principal, &TransferIcpData)>>(),));
    });
}

#[update]
pub fn controller_put_transfer_icp_data(user_id: Principal, put_data: TransferIcpData, override_lock: bool) {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
            if transfer_icp_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_transfer_icp. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_transfer_icp.insert(user_id, put_data);
    });

}

#[update]
pub fn controller_remove_transfer_icp(user_id: Principal, override_lock: bool) {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
            if transfer_icp_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_transfer_icp. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_transfer_icp.remove(&user_id);
    });
}


#[update]
pub async fn controller_complete_users_transfer_icp(opt_complete_users_ids: Option<Vec<Principal>>) -> Vec<(Principal, Result<TransferIcpSuccess, CompleteTransferIcpError>)> {
    caller_is_controller_gaurd(&caller());

    let complete_users_ids: Vec<Principal> = match opt_complete_users_ids {
        Some(complete_users_ids) => complete_users_ids,
        None => {
            with(&CTS_DATA, |cts_data| { 
                cts_data.users_transfer_icp.iter()
                .filter(|&(_user_id, transfer_icp_data): &(&Principal, &TransferIcpData)| {
                    transfer_icp_data.lock == false
                })
                .map(|(user_id, _transfer_icp_data): (&Principal, &TransferIcpData)| {
                    user_id.clone()
                })
                .collect::<Vec<Principal>>()
            })
        }
    };
    
    let rs: Vec<Result<TransferIcpSuccess, CompleteTransferIcpError>> = futures::future::join_all(
        complete_users_ids.iter().map(
            |complete_user_id: &Principal| {
                complete_transfer_icp_(complete_user_id.clone())
            }
        ).collect::<Vec<_>>()
    ).await;
    
    complete_users_ids.into_iter().zip(rs.into_iter()).collect::<Vec<(Principal, Result<TransferIcpSuccess, CompleteTransferIcpError>)>>()

}







// ----- STOP_CALLS-METHODS --------------------------

#[update]
pub fn controller_set_stop_calls_flag(stop_calls_flag: bool) {
    caller_is_controller_gaurd(&caller());
    STOP_CALLS.with(|stop_calls| { stop_calls.set(stop_calls_flag); });
}

#[query]
pub fn controller_see_stop_calls_flag() -> bool {
    caller_is_controller_gaurd(&caller());
    STOP_CALLS.with(|stop_calls| { stop_calls.get() })
}














// ----- CONTROLLER_CALL_CANISTER-METHOD --------------------------

#[derive(CandidType, Deserialize)]
pub struct ControllerCallCanisterQuest {
    pub callee: Principal,
    pub method_name: String,
    pub arg_raw: Vec<u8>,
    pub cycles: Cycles
}

#[update(manual_reply = true)]
pub async fn controller_call_canister() {
    caller_is_controller_gaurd(&caller());
    
    let (q,): (ControllerCallCanisterQuest,) = arg_data::<(ControllerCallCanisterQuest,)>(); 
    
    match call_raw128(
        q.callee,
        &q.method_name,
        &q.arg_raw,
        q.cycles   
    ).await {
        Ok(raw_sponse) => {
            reply::<(Result<Vec<u8>, (u32, String)>,)>((Ok(raw_sponse),));
        }, 
        Err(call_error) => {
            reply::<(Result<Vec<u8>, (u32, String)>,)>((Err((call_error.0 as u32, call_error.1)),));
        }
    }
}







// ----- METRICS --------------------------

#[derive(CandidType, Deserialize)]
pub struct CTSMetrics {
    global_allocator_counter: u64,
    stable_size: u64,
    cycles_balance: u128,
    cbsm_code_hash: Option<[u8; 32]>,
    cycles_bank_canister_code_hash: Option<[u8; 32]>,
    cbsms_count: u64,
    latest_known_cmc_rate: IcpXdrConversionRate,
    users_purchase_cycles_bank_count: u64,
    users_transfer_icp_count: u64,
}


#[query]
pub fn controller_view_metrics() -> CTSMetrics {
    caller_is_controller_gaurd(&caller());
    
    with(&CTS_DATA, |cts_data| {
        CTSMetrics {
            global_allocator_counter: 0,//get_allocated_bytes_count() as u64, disable this for the now.
            stable_size: ic_cdk::api::stable::stable64_size(),
            cycles_balance: ic_cdk::api::canister_balance128(),
            cbsm_code_hash: if cts_data.cbs_map_canister_code.module().len() != 0 { Some(cts_data.cbs_map_canister_code.module_hash().clone()) } else { None },
            cycles_bank_canister_code_hash: if cts_data.cycles_bank_canister_code.module().len() != 0 { Some(cts_data.cycles_bank_canister_code.module_hash().clone()) } else { None },
            cbsms_count: cts_data.cbs_maps.len() as u64,
            latest_known_cmc_rate: LATEST_KNOWN_CMC_RATE.with(|cr| cr.get()),
            users_purchase_cycles_bank_count: cts_data.users_purchase_cycles_bank.len() as u64,
            users_transfer_icp_count: cts_data.users_transfer_icp.len() as u64
        }
    })
}





// ---------------------------- :FRONTCODE. -----------------------------------

#[derive(CandidType, Deserialize)]
pub struct UploadFile {
    pub filename: String,
    pub headers: Vec<(String, String)>,
    pub first_chunk: ByteBuf,
    pub chunks: u32
}

#[update]
pub fn controller_upload_file(q: UploadFile) {
    caller_is_controller_gaurd(&caller());
    
    if q.chunks == 0 {
        trap("there must be at least 1 chunk.");
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if q.chunks == 1 {
            cts_data.frontcode_files_hashes.insert(
                q.filename.clone(), 
                sha256(&q.first_chunk)
            );
            set_root_hash(&cts_data);
        }
        cts_data.frontcode_files.insert(
            q.filename, 
            File{
                headers: q.headers,
                content_chunks: {
                    let mut v: Vec<ByteBuf> = vec![ByteBuf::new(); q.chunks.try_into().unwrap()];
                    v[0] = q.first_chunk;
                    v
                }
            }
        ); 
    });


}

#[update]
pub fn controller_upload_file_chunks(file_path: String, chunk_i: u32, chunk: ByteBuf) -> () {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        match cts_data.frontcode_files.get_mut(&file_path) {
            Some(file) => {
                file.content_chunks[chunk_i as usize] = chunk;
                
                let mut is_upload_complete: bool = true;
                for c in file.content_chunks.iter() {
                    if c.len() == 0 {
                        is_upload_complete = false;
                        break;
                    }
                }
                if is_upload_complete == true {
                    cts_data.frontcode_files_hashes.insert(
                        file_path.clone(), 
                        {
                            let mut hasher: sha2::Sha256 = sha2::Sha256::new();
                            for chunk in file.content_chunks.iter() {
                                hasher.update(chunk);    
                            }
                            hasher.finalize().into()
                        }
                    );
                    set_root_hash(&cts_data);
                }
            },
            None => {
                trap("file not found. call the controller_upload_file method to upload a new file.");
            }
        }
    });
    
    
    
}


#[update]
pub fn controller_clear_files() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.frontcode_files = Files::new();
        cts_data.frontcode_files_hashes = FilesHashes::new();
        set_root_hash(&cts_data);
    });
}

#[update]
pub fn controller_clear_file(filename: String) {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.frontcode_files.remove(&filename);
        cts_data.frontcode_files_hashes.delete(filename.as_bytes());
        set_root_hash(&cts_data);
    });
}



#[query]
pub fn controller_get_file_hashes() -> Vec<(String, [u8; 32])> {
    caller_is_controller_gaurd(&caller());
    
    with(&CTS_DATA, |cts_data| { 
        let mut vec = Vec::<(String, [u8; 32])>::new();
        cts_data.frontcode_files_hashes.for_each(|k,v| {
            vec.push((std::str::from_utf8(k).unwrap().to_string(), *v));
        });
        vec
    })
}



#[export_name = "canister_query http_request"]
pub fn http_request() {
    if STOP_CALLS.with(|stop_calls| { stop_calls.get() }) { trap("Maintenance. try again soon.") }
    
    let (quest,): (HttpRequest,) = arg_data::<(HttpRequest,)>(); 
    
    let file_name: &str = quest.url.split("?").next().unwrap();
    
    with(&CTS_DATA, |cts_data| {
        match cts_data.frontcode_files.get(file_name) {
            None => {
                reply::<(HttpResponse,)>(
                    (HttpResponse {
                        status_code: 404,
                        headers: vec![],
                        body: &ByteBuf::from(vec![]),
                        streaming_strategy: None
                    },)
                );        
            }, 
            Some(file) => {
                let (file_certificate_header_key, file_certificate_header_value): (String, String) = make_file_certificate_header(file_name); 
                let mut headers: Vec<(&str, &str)> = vec![(&file_certificate_header_key, &file_certificate_header_value),];
                headers.extend(file.headers.iter().map(|tuple: &(String, String)| { (&*tuple.0, &*tuple.1) }));
                reply::<(HttpResponse,)>(
                    (HttpResponse {
                        status_code: 200,
                        headers: headers, 
                        body: &file.content_chunks[0],
                        streaming_strategy: if let Some(stream_callback_token) = create_opt_stream_callback_token(file_name, file, 0) {
                            Some(StreamStrategy::Callback{ 
                                callback: StreamCallback(Func{
                                    principal: ic_cdk::api::id(),
                                    method: "http_request_stream_callback".to_string(),
                                }),
                                token: stream_callback_token 
                            })
                        } else {
                            None
                        }
                    },)
                );
            }
        }
    });
    return;
}




//#[query(manual_reply = true)]
#[export_name = "canister_query http_request_stream_callback"]
fn http_request_stream_callback() {
    let (token,): (StreamCallbackTokenBackwards,) = arg_data::<(StreamCallbackTokenBackwards,)>(); 
    
    with(&CTS_DATA, |cts_data| {
        match cts_data.frontcode_files.get(&token.key) {
            None => {
                trap("the file is not found");        
            }, 
            Some(file) => {
                let chunk_i: usize = token.index.0.to_usize().unwrap_or_else(|| { trap("invalid index"); }); 
                reply::<(StreamCallbackHttpResponse,)>((StreamCallbackHttpResponse {
                    body: &file.content_chunks[chunk_i],
                    token: create_opt_stream_callback_token(&token.key, file, chunk_i),
                },));
            }
        }
    })
    
}











