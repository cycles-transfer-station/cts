// icp transfers , 0.05-xdr / 7-cents flat fee

// 10 years save the user's-cycles-balance and icp-balance if the user-canister finishes.  

// convert icp for the cycles as a service and send to a canister with the cycles_transfer-specification . for the users with a cts-user-contract.


// MANAGE-MEMBERSHIP page in the frontcode



//#![allow(unused)] 
#![allow(non_camel_case_types)]

use std::{
    cell::{Cell, RefCell, RefMut}, 
    collections::{HashMap, HashSet},
    future::Future,  
};
use futures::task::Poll;

use cts_lib::{
    types::{
        Cycles,
        CTSFuel,
        CyclesTransfer,
        CyclesTransferMemo,
        XdrPerMyriadPerIcp,
        canister_code::CanisterCode,
        cycles_banks_cache::CBSCache,
        management_canister::{
            ManagementCanisterInstallCodeMode,
            ManagementCanisterInstallCodeQuest,
            ManagementCanisterCanisterSettings,
            ManagementCanisterOptionalCanisterSettings,
            ManagementCanisterCanisterStatusRecord,
            ManagementCanisterCanisterStatusVariant,
            CanisterIdRecord,
            ChangeCanisterSettingsRecord,
            
        },
        cts::{
            CyclesBankLifetimeTerminationQuest
        },
        cbs_map::{
            CBSMUserData,
            CBSMUpgradeCBError,
            CBSMUpgradeCBCallErrorType
        },
        cycles_bank::{
            CyclesBankInit,
        },
        cycles_transferrer::{
            CyclesTransferrerCanisterInit,
        },
    },
    consts::{
        MANAGEMENT_CANISTER_ID,
        MiB,
        WASM_PAGE_SIZE_BYTES,
        NETWORK_CANISTER_CREATION_FEE_CYCLES,
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES,
        ICP_LEDGER_CREATE_CANISTER_MEMO,
    },
    tools::{
        sha256,
        localkey::{
            self,
            refcell::{
                with, 
                with_mut,
            },
            cell::{
                get,
                set
            }
        },
        thirty_bytes_as_principal,
        principal_icp_subaccount,
        cycles_to_icptokens
    },
    ic_cdk::{
        self,
        api::{
            trap,
            caller, 
            time,
            id,
            canister_balance128,
            performance_counter,
            call::{
                arg_data,
                arg_data_raw,
                arg_data_raw_size,
                call_raw128,
                call,
                call_with_payment128,
                CallResult,
                RejectionCode,
                msg_cycles_refunded128,
                msg_cycles_available128,
                msg_cycles_accept128,
                reject,
                reply,
                reply_raw
            },
            stable::{
                stable64_grow,
                stable64_read,
                stable64_size,
                stable64_write,
                stable_bytes
            }
        },
        export::{
            Principal,
            candid::{
                self,
                CandidType,
                Deserialize,
                utils::{
                    encode_one, 
                    decode_one
                },
            },
        },
    },
    ic_cdk_macros::{
        update, 
        query, 
        init, 
        pre_upgrade, 
        post_upgrade
    },
    ic_ledger_types::{
        IcpMemo,
        IcpId,
        IcpIdSub,
        IcpTokens,
        IcpBlockHeight,
        IcpTimestamp,
        ICP_DEFAULT_SUBACCOUNT,
        ICP_LEDGER_TRANSFER_DEFAULT_FEE,
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        MAINNET_LEDGER_CANISTER_ID, 
        icp_transfer,
        IcpTransferArgs, 
        IcpTransferResult, 
        IcpTransferError,
        icp_account_balance,
        IcpAccountBalanceArgs
    },
    global_allocator_counter::get_allocated_bytes_count
};


#[cfg(test)]
mod t;

mod tools;
use tools::{
    check_user_icp_ledger_balance,
    main_cts_icp_id,
    CheckCurrentXdrPerMyriadPerIcpCmcRateError,
    CheckCurrentXdrPerMyriadPerIcpCmcRateSponse,
    check_current_xdr_permyriad_per_icp_cmc_rate,
    get_new_canister,
    GetNewCanisterError,
    IcpXdrConversionRate,
    take_user_icp_ledger,
    CmcNotifyError,
    CmcNotifyCreateCanisterQuest,
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
use frontcode::{File, Files, FilesHashes, HttpRequest, HttpResponse, set_root_hash, make_file_certificate_header};



#[derive(CandidType, Deserialize)]
pub struct CTSData {
    controllers: Vec<Principal>,
    cycles_market_id: Principal,
    cycles_bank_canister_code: CanisterCode,
    cbs_map_canister_code: CanisterCode,
    cycles_transferrer_canister_code: CanisterCode,
    frontcode_files: Files,
    frontcode_files_hashes: Vec<(String, [u8; 32])>, // field is [only] use for the upgrades.
    cbs_maps: Vec<Principal>,
    create_new_cbs_map_lock: bool,
    cycles_transferrer_canisters: Vec<Principal>,
    cycles_transferrer_canisters_round_robin_counter: u32,
    canisters_for_the_use: HashSet<Principal>,
    cycles_bank_purchases: HashMap<Principal, PurchaseCyclesBankData>,
    users_burn_icp_mint_cycles: HashMap<Principal, UserBurnIcpMintCyclesData>,
    users_transfer_icp: HashMap<Principal, UserTransferIcpData>

}
impl CTSData {
    fn new() -> Self {
        Self {
            controllers: Vec::new(),
            cycles_market_id: Principal::from_slice(&[]),
            cycles_bank_canister_code: CanisterCode::new(Vec::new()),
            cbs_map_canister_code: CanisterCode::new(Vec::new()),
            cycles_transferrer_canister_code: CanisterCode::new(Vec::new()),
            frontcode_files: Files::new(),
            frontcode_files_hashes: Vec::new(), // field is [only] use for the upgrades.
            cbs_maps: Vec::new(),
            create_new_cbs_map_lock: false,
            cycles_transferrer_canisters: Vec::new(),
            cycles_transferrer_canisters_round_robin_counter: 0,
            canisters_for_the_use: HashSet::new(),
            cycles_bank_purchases: HashMap::new(),
            users_burn_icp_mint_cycles: HashMap::new(),
            users_transfer_icp: HashMap::new()
        }
    }
}


    

pub const NEW_USER_CONTRACT_COST_CYCLES: Cycles = 10_000_000_000_000; //10T-cycles for a new-user-contract. lifetime: 1-year, storage-size: 50mib/*100mib-canister-memory-allocation*/, start-with-the-ctsfuel: 5T-cycles. 
pub const NEW_USER_CONTRACT_LIFETIME_DURATION_SECONDS: u64 = 1*60*60*24*365; // 1-year.
pub const NEW_USER_CONTRACT_CTSFUEL: CTSFuel = 5_000_000_000_000; // 5T-cycles.
pub const NEW_USER_CONTRACT_STORAGE_SIZE_MiB: u64 = 50; // 50-mib
pub const NEW_USER_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB: u64 = NEW_USER_CONTRACT_STORAGE_SIZE_MiB * 2;
pub const NEW_USER_CANISTER_BACKUP_CYCLES: Cycles = 1_400_000_000_000;
pub const NEW_USER_CANISTER_CREATION_CYCLES: Cycles = {
    NETWORK_CANISTER_CREATION_FEE_CYCLES
    + (
        NEW_USER_CONTRACT_LIFETIME_DURATION_SECONDS as u128 
        * NEW_USER_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB as u128 
        * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES as u128 
        / 1024 /*network mib storage per second*/ )
    + NEW_USER_CONTRACT_CTSFUEL
    + NEW_USER_CANISTER_BACKUP_CYCLES
};

pub const MAX_CYCLES_BANK_PURCHASES: usize = 5000; // the max number of entries in the NEW_USERS-hashmap at the same-time
pub const MAX_CBS_MAPS: usize = 4; // can be 30-million at 1-gb, or 3-million at 0.1-gb,

pub const MAX_USERS_TRANSFER_ICP: usize = 2000;
pub const CTS_TRANSFER_ICP_FEE: Cycles = 50_000_000_000; // taken as the icptokens by the conversion-rate

const MAX_USERS_BURN_ICP_MINT_CYCLES: usize = 1000;
const MINIMUM_USER_BURN_ICP_MINT_CYCLES: IcpTokens = IcpTokens::from_e8s(3000000); // 0.03 icp
const USER_BURN_ICP_MINT_CYCLES_FEE: Cycles = 50_000_000_000; //  user gets cmc-cycles minus this fee


pub const MINIMUM_CTS_CYCLES_TRANSFER_IN_CYCLES: Cycles = 5_000_000_000;


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;


thread_local! {
    
    pub static CTS_DATA: RefCell<CTSData> = RefCell::new(CTSData::new());
    
    // not save through the upgrades
    pub static FRONTCODE_FILES_HASHES: RefCell<FilesHashes> = RefCell::new(FilesHashes::new()); // is with the save through the upgrades by the frontcode_files_hashes field on the CTSData
    pub static LATEST_KNOWN_CMC_RATE: Cell<IcpXdrConversionRate> = Cell::new(IcpXdrConversionRate{ xdr_permyriad_per_icp: 0, timestamp_seconds: 0 });
    static     CYCLES_BANKS_CACHE: RefCell<CBSCache> = RefCell::new(CBSCache::new(1400));
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_CTS_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    
}



// -------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CTSInit {
    controllers: Vec<Principal>,
    cycles_market_id: Principal
} 

#[init]
fn init(cts_init: CTSInit) {
    with_mut(&CTS_DATA, |cts_data| { 
        cts_data.controllers = cts_init.controllers; 
        cts_data.cycles_market_id = cts_init.cycles_market_id;
    });
} 


// -------------------------------------------------------------


fn create_cts_data_candid_bytes() -> Vec<u8> {
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.frontcode_files_hashes = with(&FRONTCODE_FILES_HASHES, |frontcode_files_hashes| { 
            frontcode_files_hashes.iter().map(
                |(name, hash)| { (name.clone(), hash.clone()) }
            ).collect::<Vec<(String, [u8; 32])>>() 
        });
    });

    let mut cts_data_candid_bytes: Vec<u8> = with(&CTS_DATA, |cts_data| { encode_one(cts_data).unwrap() });
    cts_data_candid_bytes.shrink_to_fit();
    cts_data_candid_bytes
}

fn re_store_cts_data_candid_bytes(cts_data_candid_bytes: Vec<u8>) {
    
    let mut cts_data: CTSData = match decode_one::<CTSData>(&cts_data_candid_bytes) {
        Ok(cts_data) => cts_data,
        Err(_) => {
            trap("error decode of the CTSData");
            /*
            let old_cts_data: OldCTSData = decode_one::<OldCTSData>(&cts_data_candid_bytes).unwrap();
            let cts_data: CTSData = CTSData{
                controllers: old_cts_data.controllers
                ........
            };
            cts_data
            */
        }
    };

    std::mem::drop(cts_data_candid_bytes);
    
    with_mut(&FRONTCODE_FILES_HASHES, |frontcode_files_hashes| {
        *frontcode_files_hashes = FilesHashes::from_iter(cts_data.frontcode_files_hashes.drain(..));
        set_root_hash(frontcode_files_hashes);
    });
    
    with_mut(&CTS_DATA, |ctsd| {
        *ctsd = cts_data;    
    });
    
}


#[pre_upgrade]
fn pre_upgrade() {
    
    let cts_upgrade_data_candid_bytes: Vec<u8> = create_cts_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*len of the cts_upgrade_data_candid_bytes*/ + cts_upgrade_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
    }
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((cts_upgrade_data_candid_bytes.len() as u64).to_be_bytes()));
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &cts_upgrade_data_candid_bytes);
    
}

#[post_upgrade]
fn post_upgrade() {
    let mut cts_upgrade_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut cts_upgrade_data_candid_bytes_len_u64_be_bytes);
    let cts_upgrade_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(cts_upgrade_data_candid_bytes_len_u64_be_bytes); 
    
    let mut cts_upgrade_data_candid_bytes: Vec<u8> = vec![0; cts_upgrade_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut cts_upgrade_data_candid_bytes);
    
    re_store_cts_data_candid_bytes(cts_upgrade_data_candid_bytes);
    
} 


// test this!
#[no_mangle]
pub fn canister_inspect_message() {
    // caution: this function is only called for ingress messages 
    use ic_cdk::api::call::{method_name,accept_message};
    
    if caller() == Principal::anonymous() 
        && !["see_fees"].contains(&&method_name()[..])
        {
        trap("caller cannot be anonymous for this method.");
    }
    
    // check the size of the arg_data_raw_size()

    if &method_name()[..] == "cycles_transfer" {
        trap("caller must be a canister for this method.");
    }
    
    if method_name()[..].starts_with("controller") {
        if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
            trap("Caller must be a controller for this method.");
        }
    }

    accept_message();
}







// ----------------------------------------------------------------------------------------


fn cycles_market_id() -> Principal {
    with(&CTS_DATA, |cts_data| { cts_data.cycles_market_id })
}





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
    cts_user_contract_cost_cycles: Cycles,
    cts_transfer_icp_fee: Cycles,
    user_burn_icp_mint_cycles_fee: Cycles
    
    
    
}

#[query]
pub fn see_fees() -> Fees {
    Fees {
        cts_user_contract_cost_cycles: NEW_USER_CONTRACT_COST_CYCLES,
        cts_transfer_icp_fee: CTS_TRANSFER_ICP_FEE,
        user_burn_icp_mint_cycles_fee: USER_BURN_ICP_MINT_CYCLES_FEE
        
        
    }
}











// save the fees in the purchase_cycles_bank_data so the fees cant change while creating a new user

#[derive(Clone, CandidType, Deserialize)]
pub struct PurchaseCyclesBankData {
    start_time_nanos: u64,
    lock: bool,    
    purchase_cycles_bank_quest: PurchaseCyclesBankQuest,
    // the options and bools are for the memberance of the steps
    current_xdr_icp_rate: Option<XdrPerMyriadPerIcp>,
    look_if_user_is_in_the_cbs_maps: bool,
    referral_user_canister_id: Option<Principal>, // use if a referral
    create_user_canister_block_height: Option<IcpBlockHeight>,
    user_canister: Option<Principal>,
    users_map_canister: Option<Principal>,
    user_canister_uninstall_code: bool,
    user_canister_install_code: bool,
    user_canister_status_record: Option<ManagementCanisterCanisterStatusRecord>,
    collect_icp: bool,
    collect_cycles_cmc_icp_transfer_block_height: Option<IcpBlockHeight>,
    collect_cycles_cmc_notify_cycles: Option<Cycles>,
    referral_user_referral_payment_cycles_transfer: bool,
    user_referral_payment_cycles_transfer: bool
    
}



#[derive(CandidType, Deserialize)]
pub enum PurchaseCyclesBankMidCallError{
    CBSMapsFindUserCallFails(Vec<(Principal, (u32, String))>),
    PutNewUserIntoACBSMError(PutNewUserIntoACBSMError),
    CreateCyclesBankCanisterIcpTransferError(IcpTransferError),
    CreateCyclesBankCanisterIcpTransferCallError((u32, String)),
    CreateCyclesBankCanisterCmcNotifyError(CmcNotifyError),
    CreateCyclesBankCanisterCmcNotifyCallError((u32, String)),
    CyclesBankUninstallCodeCallError((u32, String)),
    CyclesBankCodeNotFound,
    CyclesBankInstallCodeCallError((u32, String)),
    CyclesBankCanisterStatusCallError((u32, String)),
    CyclesBankModuleVerificationError,
    CyclesBankStartCanisterCallError((u32, String)),
    CyclesBankUpdateSettingsCallError((u32, String)),
    CollectCyclesLedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
    CollectCyclesLedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
    ReferralUserReferralPaymentCyclesTransferCallError((u32, String)),
    UserReferralPaymentCyclesTransferCallError((u32, String)),
    CollectIcpTransferError(IcpTransferError),
    CollectIcpTransferCallError((u32, String)),
    
}


#[derive(CandidType, Deserialize)]
pub enum PurchaseCyclesBankError{
    ReferralUserCannotBeTheCaller,
    CheckIcpBalanceCallError((u32, String)),
    CheckCurrentXdrPerMyriadPerIcpCmcRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    UserIcpLedgerBalanceTooLow{
        cts_user_contract_cost_icp: IcpTokens,
        user_icp_ledger_balance: IcpTokens,
        icp_ledger_transfer_fee: IcpTokens
    },
    UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: bool },
    UserIsInTheMiddleOfAUserTransferIcpCall{ must_call_complete: bool },
    UserIsInTheMiddleOfAUserBurnIcpMintCyclesCall{ must_call_complete: bool },
    CTSIsBusy,
    FoundCyclesBank(Principal),
    ReferralUserNotFound,
    CreateCyclesBankCanisterCmcNotifyError(CmcNotifyError),
    MidCallError(PurchaseCyclesBankMidCallError),    // call complete_purchase_cycles_bank on this sponse
}


#[derive(CandidType, Deserialize, Clone, PartialEq, Eq)]
pub struct PurchaseCyclesBankQuest {
    opt_referral_user_id: Option<Principal>,
}


#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesBankSuccess {
    cycles_bank_canister_id: Principal,
}


fn write_purchase_cycles_bank_data(user_id: &Principal, purchase_cycles_bank_data: PurchaseCyclesBankData) {
    with_mut(&CTS_DATA, |cts_data| {
        match cts_data.cycles_bank_purchases.get_mut(user_id) {
            Some(nud) => { *nud = purchase_cycles_bank_data; },
            None => {}
        }
    });
}

// for the now a user must pay with the icp.
#[update]
pub async fn purchase_cycles_bank(q: PurchaseCyclesBankQuest) -> Result<PurchaseCyclesBankSuccess, PurchaseCyclesBankError> {

    let user_id: Principal = caller();

    if let Some(ref referral_user_id) = q.opt_referral_user_id {
        if *referral_user_id == user_id {
            return Err(PurchaseCyclesBankError::ReferralUserCannotBeTheCaller);
        }
    }

    let purchase_cycles_bank_data: PurchaseCyclesBankData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.cycles_bank_purchases.get(&user_id) {
            Some(purchase_cycles_bank_data) => {
                return Err(PurchaseCyclesBankError::UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock });
            },
            None => {
                if get(&STOP_CALLS) { trap("Maintenance. try soon."); }
                if let Some(user_burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
                    return Err(PurchaseCyclesBankError::UserIsInTheMiddleOfAUserBurnIcpMintCyclesCall{ must_call_complete: !user_burn_icp_mint_cycles_data.lock });
                }
                if let Some(user_transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
                    return Err(PurchaseCyclesBankError::UserIsInTheMiddleOfAUserTransferIcpCall{ must_call_complete: !user_transfer_icp_data.lock });
                }
                if cts_data.cycles_bank_purchases.len() >= MAX_CYCLES_BANK_PURCHASES {
                    return Err(PurchaseCyclesBankError::CTSIsBusy);
                }
                let purchase_cycles_bank_data: PurchaseCyclesBankData = PurchaseCyclesBankData{
                    start_time_nanos: time(),
                    lock: true,
                    purchase_cycles_bank_quest: q,
                    // the options and bools are for the memberance of the steps
                    current_xdr_icp_rate: None,
                    look_if_user_is_in_the_cbs_maps: false,
                    referral_user_canister_id: None,
                    create_user_canister_block_height: None,
                    user_canister: None,
                    users_map_canister: None,
                    user_canister_uninstall_code: false,
                    user_canister_install_code: false,
                    user_canister_status_record: None,
                    collect_icp: false,
                    collect_cycles_cmc_icp_transfer_block_height: None,
                    collect_cycles_cmc_notify_cycles: None,
                    referral_user_referral_payment_cycles_transfer: false,
                    user_referral_payment_cycles_transfer: false
                };
                cts_data.cycles_bank_purchases.insert(user_id, purchase_cycles_bank_data.clone());
                Ok(purchase_cycles_bank_data)
            }
        }
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
        match cts_data.cycles_bank_purchases.get_mut(&user_id) {
            Some(purchase_cycles_bank_data) => {
                if purchase_cycles_bank_data.lock == true {
                    return Err(CompletePurchaseCyclesBankError::PurchaseCyclesBankError(PurchaseCyclesBankError::UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: false }));
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
                with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_bank_purchases.remove(&user_id); });
                return Err(PurchaseCyclesBankError::CheckIcpBalanceCallError((check_balance_call_error.0 as u32, check_balance_call_error.1)));
            }
        };
                
        let current_xdr_icp_rate: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate_sponse {
            Ok(rate) => rate,
            Err(check_xdr_icp_rate_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_bank_purchases.remove(&user_id); });
                return Err(PurchaseCyclesBankError::CheckCurrentXdrPerMyriadPerIcpCmcRateError(check_xdr_icp_rate_error));
            }
        };
        
        let current_membership_cost_icp: IcpTokens = cycles_to_icptokens(NEW_USER_CONTRACT_COST_CYCLES, current_xdr_icp_rate); 
        
        if user_icp_ledger_balance < current_membership_cost_icp + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) {
            with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_bank_purchases.remove(&user_id); });
            return Err(PurchaseCyclesBankError::UserIcpLedgerBalanceTooLow{
                cts_user_contract_cost_icp: current_membership_cost_icp,
                user_icp_ledger_balance,
                icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE
            });
        }   
        
        purchase_cycles_bank_data.current_xdr_icp_rate = Some(current_xdr_icp_rate);
    }
    
    
    if purchase_cycles_bank_data.look_if_user_is_in_the_cbs_maps == false {
        // check in the list of the users-whos cycles-balance is save but without a user-canister 
        
        match find_user_canister_of_the_specific_user(user_id).await {
            Ok(opt_user_canister_id) => match opt_user_canister_id {
                Some(user_canister_id) => {
                    with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_bank_purchases.remove(&user_id); });
                    return Err(PurchaseCyclesBankError::FoundCyclesBank(user_canister_id));
                },
                None => {
                    purchase_cycles_bank_data.look_if_user_is_in_the_cbs_maps = true;
                }
            },
            Err(find_user_in_the_users_map_canisters_error) => match find_user_in_the_users_map_canisters_error {
                FindUserInTheCBSMapsError::CBSMapsFindUserCallFails(umc_call_errors) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CBSMapsFindUserCallFails(umc_call_errors)));
                }
            }
        }
        
    }
    
    if purchase_cycles_bank_data.purchase_cycles_bank_quest.opt_referral_user_id.is_some() {
    
        if purchase_cycles_bank_data.referral_user_canister_id.is_none() {
        
            match find_user_canister_of_the_specific_user(purchase_cycles_bank_data.purchase_cycles_bank_quest.opt_referral_user_id.as_ref().unwrap().clone()).await {
                Ok(opt_user_canister_id) => match opt_user_canister_id {
                    Some(user_canister_id) => {
                        purchase_cycles_bank_data.referral_user_canister_id = Some(user_canister_id);
                    },
                    None => {
                        with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_bank_purchases.remove(&user_id); });
                        return Err(PurchaseCyclesBankError::ReferralUserNotFound);
                    }
                },
                Err(find_user_in_the_users_map_canisters_error) => match find_user_in_the_users_map_canisters_error {
                    FindUserInTheCBSMapsError::CBSMapsFindUserCallFails(umc_call_errors) => {
                        purchase_cycles_bank_data.lock = false;
                        write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                        return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CBSMapsFindUserCallFails(umc_call_errors)));
                    }
                }
            }
            
        }
        
    }
    

    if purchase_cycles_bank_data.create_user_canister_block_height.is_none() {
        let create_user_canister_block_height: IcpBlockHeight = match icp_transfer(
            MAINNET_LEDGER_CANISTER_ID,
            IcpTransferArgs {
                memo: ICP_LEDGER_CREATE_CANISTER_MEMO,
                amount: cycles_to_icptokens(NEW_USER_CANISTER_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()),
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
    
        purchase_cycles_bank_data.create_user_canister_block_height = Some(create_user_canister_block_height);
    }


    if purchase_cycles_bank_data.user_canister.is_none() {
    
        let user_canister: Principal = match call::<(CmcNotifyCreateCanisterQuest,), (Result<Principal, CmcNotifyError>,)>(
            MAINNET_CYCLES_MINTING_CANISTER_ID,
            "notify_create_canister",
            (CmcNotifyCreateCanisterQuest {
                controller: id(),
                block_index: purchase_cycles_bank_data.create_user_canister_block_height.unwrap()
            },)
        ).await {
            Ok((notify_result,)) => match notify_result {
                Ok(new_canister_id) => new_canister_id,
                Err(cmc_notify_error) => {
                    // match on the cmc_notify_error, if it failed bc of the cmc icp transfer block height expired, remove the user from the NEW_USERS map.     
                    match cmc_notify_error {
                        CmcNotifyError::TransactionTooOld(_) | CmcNotifyError::Refunded{ .. } => {
                            with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_bank_purchases.remove(&user_id); });
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
        
        purchase_cycles_bank_data.user_canister = Some(user_canister);
        with_mut(&CYCLES_BANKS_CACHE, |uc_cache| { uc_cache.put(user_id, Some(user_canister)); });
        purchase_cycles_bank_data.user_canister_uninstall_code = true; // because a fresh cmc canister is empty 
    }
        
 

    if purchase_cycles_bank_data.user_canister_uninstall_code == false {
        
        match call::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "uninstall_code",
            (CanisterIdRecord { canister_id: purchase_cycles_bank_data.user_canister.unwrap() },),
        ).await {
            Ok(_) => {},
            Err(uninstall_code_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankUninstallCodeCallError((uninstall_code_call_error.0 as u32, uninstall_code_call_error.1))));
            }
        }
        
        purchase_cycles_bank_data.user_canister_uninstall_code = true;
    }


    if purchase_cycles_bank_data.user_canister_install_code == false {
    
        if with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module().len() == 0 }) {
            purchase_cycles_bank_data.lock = false;
            write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
            return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankCodeNotFound));
        }

        match call::<(ManagementCanisterInstallCodeQuest,), ()>(
            MANAGEMENT_CANISTER_ID,
            "install_code",
            (ManagementCanisterInstallCodeQuest {
                mode : ManagementCanisterInstallCodeMode::install,
                canister_id : purchase_cycles_bank_data.user_canister.unwrap(),
                wasm_module : unsafe{&*with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module() as *const Vec<u8> })},
                arg : &encode_one(&CyclesBankInit{ 
                    cts_id: id(), 
                    cycles_market_id: cycles_market_id(),
                    user_id: user_id,
                    storage_size_mib: NEW_USER_CONTRACT_STORAGE_SIZE_MiB,                         
                    lifetime_termination_timestamp_seconds: time()/1_000_000_000 + NEW_USER_CONTRACT_LIFETIME_DURATION_SECONDS,
                    cycles_transferrer_canisters: with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canisters.clone() })
                }).unwrap()
            },),
        ).await {
            Ok(()) => {},
            Err(put_code_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankInstallCodeCallError((put_code_call_error.0 as u32, put_code_call_error.1))));
            }
        }
        
        purchase_cycles_bank_data.user_canister_install_code = true;
    }
    
    if purchase_cycles_bank_data.user_canister_status_record.is_none() {
        
        let canister_status_record: ManagementCanisterCanisterStatusRecord = match call(
            MANAGEMENT_CANISTER_ID,
            "canister_status",
            (CanisterIdRecord { canister_id: purchase_cycles_bank_data.user_canister.unwrap() },),
        ).await {
            Ok((canister_status_record,)) => canister_status_record,
            Err(canister_status_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankCanisterStatusCallError((canister_status_call_error.0 as u32, canister_status_call_error.1))));
            }
        };
        
        purchase_cycles_bank_data.user_canister_status_record = Some(canister_status_record);
    }
        
    // no async in this if-block so no PurchaseCyclesBankData field needed. can make it for the optimization though
    if with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module().len() == 0 }) {
        purchase_cycles_bank_data.lock = false;
        write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
        return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankCodeNotFound));
    }
    if purchase_cycles_bank_data.user_canister_status_record.as_ref().unwrap().module_hash.is_none() || purchase_cycles_bank_data.user_canister_status_record.as_ref().unwrap().module_hash.as_ref().unwrap().clone() != with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module_hash().clone() }) {
        // go back a couple of steps
        purchase_cycles_bank_data.user_canister_uninstall_code = false;                                  
        purchase_cycles_bank_data.user_canister_install_code = false;
        purchase_cycles_bank_data.user_canister_status_record = None;
        purchase_cycles_bank_data.lock = false;
        write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
        return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankModuleVerificationError));
    }
    

    if purchase_cycles_bank_data.user_canister_status_record.as_ref().unwrap().status != ManagementCanisterCanisterStatusVariant::running {
    
        match call::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "start_canister",
            (CanisterIdRecord { canister_id: purchase_cycles_bank_data.user_canister.unwrap() },)
        ).await {
            Ok(_) => {
                purchase_cycles_bank_data.user_canister_status_record.as_mut().unwrap().status = ManagementCanisterCanisterStatusVariant::running; 
            },
            Err(start_canister_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankStartCanisterCallError((start_canister_call_error.0 as u32, start_canister_call_error.1))));
            }
        }
        
    }

    
    if purchase_cycles_bank_data.users_map_canister.is_none() {
        
        let users_map_canister_id: Principal = match put_new_user_into_a_cbsm(
            user_id, 
            CBSMUserData{
                cycles_bank_canister_id: purchase_cycles_bank_data.user_canister.as_ref().unwrap().clone(),
                cycles_bank_latest_known_module_hash: purchase_cycles_bank_data.user_canister_status_record.as_ref().unwrap().module_hash.as_ref().unwrap().clone()
            }
        ).await {
            Ok(umcid) => umcid,
            Err(put_new_user_into_a_users_map_canister_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::PutNewUserIntoACBSMError(put_new_user_into_a_users_map_canister_error)));
            }
        };
        
        purchase_cycles_bank_data.users_map_canister = Some(users_map_canister_id);
    }



    //update the controller to clude the users_map_canister
    
    let put_user_canister_settings: ManagementCanisterCanisterSettings = ManagementCanisterCanisterSettings{
        controllers : vec![
            id(), 
            purchase_cycles_bank_data.users_map_canister.as_ref().unwrap().clone(),
            purchase_cycles_bank_data.user_canister.as_ref().unwrap().clone(),
        ],
        compute_allocation : 0,
        memory_allocation : NEW_USER_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB as u128 * MiB as u128,
        freezing_threshold : 2592000 
    };
    
    if purchase_cycles_bank_data.user_canister_status_record.as_ref().unwrap().settings != put_user_canister_settings {
                
        match call::<(ChangeCanisterSettingsRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "update_settings",
            (ChangeCanisterSettingsRecord{
                canister_id: purchase_cycles_bank_data.user_canister.as_ref().unwrap().clone(),
                settings: ManagementCanisterOptionalCanisterSettings{
                    controllers : Some(put_user_canister_settings.controllers.clone()),
                    compute_allocation : Some(put_user_canister_settings.compute_allocation),
                    memory_allocation : Some(put_user_canister_settings.memory_allocation),
                    freezing_threshold : Some(put_user_canister_settings.freezing_threshold),
                }
            },)
        ).await {
            Ok(()) => {
                purchase_cycles_bank_data.user_canister_status_record.as_mut().unwrap().settings = put_user_canister_settings;
            },
            Err(update_settings_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankUpdateSettingsCallError((update_settings_call_error.0 as u32, update_settings_call_error.1))));
            }
        }
    }
    
    
    // hand out the referral-bonuses if there is.
    if purchase_cycles_bank_data.purchase_cycles_bank_quest.opt_referral_user_id.is_some() {
        
        if purchase_cycles_bank_data.collect_cycles_cmc_icp_transfer_block_height.is_none() {
            match ledger_topup_cycles_cmc_icp_transfer(
                cycles_to_icptokens(NEW_USER_CONTRACT_COST_CYCLES - NEW_USER_CANISTER_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()), 
                Some(principal_icp_subaccount(&user_id)), 
                id()
            ).await {
                Ok(block_height) => {
                    purchase_cycles_bank_data.collect_cycles_cmc_icp_transfer_block_height = Some(block_height);
                },
                Err(ledger_topup_cycles_cmc_icp_transfer_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CollectCyclesLedgerTopupCyclesCmcIcpTransferError(ledger_topup_cycles_cmc_icp_transfer_error)));
                }
            }
        }
        
        if purchase_cycles_bank_data.collect_cycles_cmc_notify_cycles.is_none() {
            match ledger_topup_cycles_cmc_notify(purchase_cycles_bank_data.collect_cycles_cmc_icp_transfer_block_height.unwrap(), id()).await {
                Ok(topup_cycles) => {
                    purchase_cycles_bank_data.collect_cycles_cmc_notify_cycles = Some(topup_cycles); 
                }, 
                Err(ledger_topup_cycles_cmc_notify_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CollectCyclesLedgerTopupCyclesCmcNotifyError(ledger_topup_cycles_cmc_notify_error)));
                }
            }
        }
        
        if purchase_cycles_bank_data.referral_user_referral_payment_cycles_transfer == false {
            match call_with_payment128::<(CyclesTransfer,), ()>(
                purchase_cycles_bank_data.referral_user_canister_id.as_ref().unwrap().clone(),        
                "cycles_transfer",
                (CyclesTransfer{
                    memo: CyclesTransferMemo::Blob(b"CTS-REFERRAL-PAYMENT".to_vec())
                },),
                1_000_000_000_000
            ).await {
                Ok(()) => {
                    purchase_cycles_bank_data.referral_user_referral_payment_cycles_transfer = true;
                }, 
                Err(referral_user_referral_payment_cycles_transfer_call_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::ReferralUserReferralPaymentCyclesTransferCallError((referral_user_referral_payment_cycles_transfer_call_error.0 as u32, referral_user_referral_payment_cycles_transfer_call_error.1))));
                }
            }
        }
        
        if purchase_cycles_bank_data.user_referral_payment_cycles_transfer == false {
            match call_with_payment128::<(CyclesTransfer,), ()>(
                purchase_cycles_bank_data.user_canister.as_ref().unwrap().clone(),
                "cycles_transfer",
                (CyclesTransfer{
                    memo: CyclesTransferMemo::Blob(b"CTS-REFERRAL-PAYMENT".to_vec())
                },),
                1_000_000_000_000
            ).await {
                Ok(()) => {
                    purchase_cycles_bank_data.user_referral_payment_cycles_transfer = true;
                }, 
                Err(user_referral_payment_cycles_transfer_call_error) => {
                    purchase_cycles_bank_data.lock = false;
                    write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                    return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::UserReferralPaymentCyclesTransferCallError((user_referral_payment_cycles_transfer_call_error.0 as u32, user_referral_payment_cycles_transfer_call_error.1))));
                }
            }
        }
        
    } else {
        
        if purchase_cycles_bank_data.collect_icp == false {
            match take_user_icp_ledger(&user_id, cycles_to_icptokens(NEW_USER_CONTRACT_COST_CYCLES - NEW_USER_CANISTER_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap())).await {
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
    
    }
    


    with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_bank_purchases.remove(&user_id); });
    
    Ok(PurchaseCyclesBankSuccess {
        cycles_bank_canister_id: purchase_cycles_bank_data.user_canister.unwrap()
    })
}





// ----------------------------------------------------------------------------------------------------





#[derive(CandidType, Deserialize)]
pub enum FindCyclesBankError {
    UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: bool },
    FindUserInTheCBSMapsError(FindUserInTheCBSMapsError),
}

#[update]
pub async fn find_cycles_bank() -> Result<Option<Principal>, FindCyclesBankError> {
    //if localkey::get::(&STOP_CALLS) { trap("Maintenance. try again soon."); }

    let user_id: Principal = caller();
    
    with(&CTS_DATA, |cts_data| { 
        match cts_data.cycles_bank_purchases.get(&user_id) {
            None => Ok(()),
            Some(purchase_cycles_bank_data) => { 
                return Err(FindCyclesBankError::UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock });    
            }
        }
    })?;
    
    find_user_canister_of_the_specific_user(user_id).await.map_err(
        |find_user_in_the_users_map_canisters_error| { 
            FindCyclesBankError::FindUserInTheCBSMapsError(find_user_in_the_users_map_canisters_error) 
        }
    )

}



async fn find_user_canister_of_the_specific_user(user_id: Principal) -> Result<Option<Principal>, FindUserInTheCBSMapsError> {
    if let Some(opt_user_canister_id) = with_mut(&CYCLES_BANKS_CACHE, |uc_cache| { uc_cache.check(user_id) }) {
        return Ok(opt_user_canister_id);
    } 
    find_user_in_the_cbs_maps(user_id).await.map(
        |opt_umc_user_data_and_umc_id| {
            let opt_user_canister_id: Option<Principal> = opt_umc_user_data_and_umc_id.map(|(umc_user_data, _umc_id)| { umc_user_data.cycles_bank_canister_id });
            with_mut(&CYCLES_BANKS_CACHE, |uc_cache| {
                uc_cache.put(user_id, opt_user_canister_id);
            });    
            opt_user_canister_id
        }
    )   
} 






// ----------------------------------------------------------------------------------------------------




// options are for the memberance of the steps

#[derive(CandidType, Deserialize, Clone)]
pub struct UserBurnIcpMintCyclesData {
    start_time_nanos: u64,
    lock: bool,
    user_burn_icp_mint_cycles_quest: UserBurnIcpMintCyclesQuest, 
    user_burn_icp_mint_cycles_fee: Cycles,
    user_canister_id: Option<Principal>,
    cmc_icp_transfer_block_height: Option<IcpBlockHeight>,
    cmc_cycles: Option<Cycles>,
    call_user_canister_cycles_transfer_refund: Option<Cycles>,
    call_management_canister_posit_cycles: bool
}


#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct UserBurnIcpMintCyclesQuest {
    burn_icp: IcpTokens,    
}

#[derive(CandidType, Deserialize)]
pub enum UserBurnIcpMintCyclesError {
    UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: bool },
    UserIsInTheMiddleOfAUserTransferIcpCall{ must_call_complete: bool },
    UserIsInTheMiddleOfAUserBurnIcpMintCyclesCall{ must_call_complete: bool },
    MinimumUserBurnIcpMintCycles{minimum_user_burn_icp_mint_cycles: IcpTokens},
    IcpCheckBalanceCallError((u32, String)),
    UserIcpBalanceTooLow{user_icp_balance: IcpTokens, icp_ledger_transfer_fee: IcpTokens},
    FindUserInTheCBSMapsError(FindUserInTheCBSMapsError),
    CyclesBankNotFound,
    CTSIsBusy,
    LedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
    MidCallError(UserBurnIcpMintCyclesMidCallError) // on this error, call with the same-parameters for the completion of this call. 
}


#[derive(CandidType, Deserialize)]
pub enum UserBurnIcpMintCyclesMidCallError {
    LedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
    CallCyclesBankCyclesTransferCandidEncodeError(String),
    CallCyclesBankCallPerformError(u32),
    ManagementCanisterPositCyclesCallError((u32, String))
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct UserBurnIcpMintCyclesSuccess {
    mint_cycles_for_the_user: Cycles,
    cts_fee_taken: Cycles
}


#[update]
pub async fn user_burn_icp_mint_cycles(q: UserBurnIcpMintCyclesQuest) -> Result<UserBurnIcpMintCyclesSuccess, UserBurnIcpMintCyclesError> {

    let user_id: Principal = caller(); 

    if q.burn_icp < MINIMUM_USER_BURN_ICP_MINT_CYCLES {
        return Err(UserBurnIcpMintCyclesError::MinimumUserBurnIcpMintCycles{
            minimum_user_burn_icp_mint_cycles: MINIMUM_USER_BURN_ICP_MINT_CYCLES
        });
    }
    
    let user_burn_icp_mint_cycles_data: UserBurnIcpMintCyclesData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_burn_icp_mint_cycles.get(&user_id) {
            Some(user_burn_icp_mint_cycles_data) => {
                return Err(UserBurnIcpMintCyclesError::UserIsInTheMiddleOfAUserBurnIcpMintCyclesCall{ must_call_complete: !user_burn_icp_mint_cycles_data.lock });
            },
            None => {
                if get(&STOP_CALLS) { trap("Maintenance. try soon."); }
                if let Some(purchase_cycles_bank_data) = cts_data.cycles_bank_purchases.get(&user_id) {
                    return Err(UserBurnIcpMintCyclesError::UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock });
                }
                if let Some(user_transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
                    return Err(UserBurnIcpMintCyclesError::UserIsInTheMiddleOfAUserTransferIcpCall{ must_call_complete: !user_transfer_icp_data.lock });
                }
                if cts_data.users_burn_icp_mint_cycles.len() >= MAX_USERS_BURN_ICP_MINT_CYCLES {
                    return Err(UserBurnIcpMintCyclesError::CTSIsBusy);
                }
                let user_burn_icp_mint_cycles_data: UserBurnIcpMintCyclesData = UserBurnIcpMintCyclesData{
                    start_time_nanos: time(),
                    lock: true,
                    user_burn_icp_mint_cycles_quest: q, 
                    user_burn_icp_mint_cycles_fee: USER_BURN_ICP_MINT_CYCLES_FEE,
                    user_canister_id: None,
                    cmc_icp_transfer_block_height: None,
                    cmc_cycles: None,
                    call_user_canister_cycles_transfer_refund: None,
                    call_management_canister_posit_cycles: false
                };
                cts_data.users_burn_icp_mint_cycles.insert(user_id, user_burn_icp_mint_cycles_data.clone());
                Ok(user_burn_icp_mint_cycles_data)
            }
        }
    })?;

    user_burn_icp_mint_cycles_(user_id, user_burn_icp_mint_cycles_data).await
}


#[derive(CandidType, Deserialize)]
pub enum CompleteUserBurnIcpMintCyclesError{
    UserNotFoundInTheOngoingUsersBurnIcpMintCyclesMap,
    UserBurnIcpMintCyclesError(UserBurnIcpMintCyclesError)
}

#[update]
pub async fn complete_user_burn_icp_mint_cycles() -> Result<UserBurnIcpMintCyclesSuccess, CompleteUserBurnIcpMintCyclesError> {

    let user_id: Principal = caller(); 
    
    complete_user_burn_icp_mint_cycles_(user_id).await

}


async fn complete_user_burn_icp_mint_cycles_(user_id: Principal) -> Result<UserBurnIcpMintCyclesSuccess, CompleteUserBurnIcpMintCyclesError> {
    
    let user_burn_icp_mint_cycles_data: UserBurnIcpMintCyclesData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
            Some(user_burn_icp_mint_cycles_data) => {
                if user_burn_icp_mint_cycles_data.lock == true {
                    return Err(CompleteUserBurnIcpMintCyclesError::UserBurnIcpMintCyclesError(UserBurnIcpMintCyclesError::UserIsInTheMiddleOfAUserBurnIcpMintCyclesCall{ must_call_complete: false }));
                }
                user_burn_icp_mint_cycles_data.lock = true;
                Ok(user_burn_icp_mint_cycles_data.clone())
            },
            None => {
                return Err(CompleteUserBurnIcpMintCyclesError::UserNotFoundInTheOngoingUsersBurnIcpMintCyclesMap);
            }
        }
    })?;

    user_burn_icp_mint_cycles_(user_id, user_burn_icp_mint_cycles_data).await
        .map_err(|user_burn_icp_mint_cycles_error| { 
            CompleteUserBurnIcpMintCyclesError::UserBurnIcpMintCyclesError(user_burn_icp_mint_cycles_error) 
        })
        
}



async fn user_burn_icp_mint_cycles_(user_id: Principal, mut user_burn_icp_mint_cycles_data: UserBurnIcpMintCyclesData) -> Result<UserBurnIcpMintCyclesSuccess, UserBurnIcpMintCyclesError> {
    
    if user_burn_icp_mint_cycles_data.user_canister_id.is_none() {
            
        let user_icp_balance: IcpTokens = match check_user_icp_ledger_balance(&user_id).await {
            Ok(icp_tokens) => icp_tokens,
            Err(icp_check_balance_call_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
                return Err(UserBurnIcpMintCyclesError::IcpCheckBalanceCallError((icp_check_balance_call_error.0 as u32, icp_check_balance_call_error.1)));
            }
        };
        
        if user_icp_balance < user_burn_icp_mint_cycles_data.user_burn_icp_mint_cycles_quest.burn_icp + ICP_LEDGER_TRANSFER_DEFAULT_FEE {
            with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
            return Err(UserBurnIcpMintCyclesError::UserIcpBalanceTooLow{
                user_icp_balance,
                icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE
            });
        }
        
        let user_canister_id: Principal = match find_user_canister_of_the_specific_user(user_id).await {
            Ok(opt_user_canister_id) => match opt_user_canister_id {
                None => {
                    with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
                    return Err(UserBurnIcpMintCyclesError::CyclesBankNotFound);
                },
                Some(user_canister_id) => user_canister_id
            },
            Err(find_user_in_the_users_map_canisters_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
                return Err(UserBurnIcpMintCyclesError::FindUserInTheCBSMapsError(find_user_in_the_users_map_canisters_error));
            }
        };
        
        user_burn_icp_mint_cycles_data.user_canister_id = Some(user_canister_id);
    }     
    
    
    // this is after the put into the state bc if this is success the block height must be save in the state
    if user_burn_icp_mint_cycles_data.cmc_icp_transfer_block_height.is_none() {
        match ledger_topup_cycles_cmc_icp_transfer(user_burn_icp_mint_cycles_data.user_burn_icp_mint_cycles_quest.burn_icp, Some(principal_icp_subaccount(&user_id)), id()).await {
            Ok(block_height) => { user_burn_icp_mint_cycles_data.cmc_icp_transfer_block_height = Some(block_height); },
            Err(ledger_topup_cycles_cmc_icp_transfer_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
                return Err(UserBurnIcpMintCyclesError::LedgerTopupCyclesCmcIcpTransferError(ledger_topup_cycles_cmc_icp_transfer_error));
            }
        }
    }
    
    if user_burn_icp_mint_cycles_data.cmc_cycles.is_none() {
        match ledger_topup_cycles_cmc_notify(user_burn_icp_mint_cycles_data.cmc_icp_transfer_block_height.unwrap(), id()).await {
            Ok(cmc_cycles) => { user_burn_icp_mint_cycles_data.cmc_cycles = Some(cmc_cycles); },
            Err(ledger_topup_cycles_cmc_notify_error) => {
                user_burn_icp_mint_cycles_data.lock = false;
                with_mut(&CTS_DATA, |cts_data| {
                    match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                        Some(data) => { *data = user_burn_icp_mint_cycles_data; },
                        None => {}
                    }
                });
                return Err(UserBurnIcpMintCyclesError::MidCallError(UserBurnIcpMintCyclesMidCallError::LedgerTopupCyclesCmcNotifyError(ledger_topup_cycles_cmc_notify_error)));
            }
        }
    }
    
    let cycles_for_the_user_canister: Cycles = user_burn_icp_mint_cycles_data.cmc_cycles.unwrap().checked_sub(user_burn_icp_mint_cycles_data.user_burn_icp_mint_cycles_fee).unwrap_or(user_burn_icp_mint_cycles_data.cmc_cycles.unwrap());
    if user_burn_icp_mint_cycles_data.call_user_canister_cycles_transfer_refund.is_none() {
        let mut cycles_transfer_call_future = call_raw128(
            user_burn_icp_mint_cycles_data.user_canister_id.unwrap(),
            "cycles_transfer",
            &match encode_one(CyclesTransfer{
                memo: CyclesTransferMemo::Blob(b"CTS-BURN-ICP-MINT-CYCLES".to_vec())
            }) { 
                Ok(b)=>b, 
                Err(candid_error)=>{
                    user_burn_icp_mint_cycles_data.lock = false;
                    with_mut(&CTS_DATA, |cts_data| {
                        match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                            Some(data) => { *data = user_burn_icp_mint_cycles_data; },
                            None => {}
                        }
                    });
                    return Err(UserBurnIcpMintCyclesError::MidCallError(UserBurnIcpMintCyclesMidCallError::CallCyclesBankCyclesTransferCandidEncodeError(format!("{}", candid_error))));          
                } 
            },
            cycles_for_the_user_canister
        );
        
        if let Poll::Ready(call_result_with_an_error) = futures::poll!(&mut cycles_transfer_call_future) {
            let call_error: (RejectionCode, String) = call_result_with_an_error.unwrap_err();
            user_burn_icp_mint_cycles_data.lock = false;
            with_mut(&CTS_DATA, |cts_data| {
                match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                    Some(data) => { *data = user_burn_icp_mint_cycles_data; },
                    None => {}
                }
            });
            return Err(UserBurnIcpMintCyclesError::MidCallError(UserBurnIcpMintCyclesMidCallError::CallCyclesBankCallPerformError(call_error.0 as u32)));    
        }
        
        cycles_transfer_call_future.await; 
        user_burn_icp_mint_cycles_data.call_user_canister_cycles_transfer_refund = Some(msg_cycles_refunded128());
    }
    
    if user_burn_icp_mint_cycles_data.call_user_canister_cycles_transfer_refund.unwrap() != 0 
    && user_burn_icp_mint_cycles_data.call_management_canister_posit_cycles == false {
        match call_with_payment128::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "deposit_cycles",
            (CanisterIdRecord{
                canister_id: user_burn_icp_mint_cycles_data.user_canister_id.unwrap()
            },),
            user_burn_icp_mint_cycles_data.call_user_canister_cycles_transfer_refund.unwrap()
        ).await {
            Ok(_) => {
                user_burn_icp_mint_cycles_data.call_management_canister_posit_cycles = true;
            },
            Err(call_error) => {
                user_burn_icp_mint_cycles_data.lock = false;
                with_mut(&CTS_DATA, |cts_data| {
                    match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                        Some(data) => { *data = user_burn_icp_mint_cycles_data; },
                        None => {}
                    }
                });
                return Err(UserBurnIcpMintCyclesError::MidCallError(UserBurnIcpMintCyclesMidCallError::ManagementCanisterPositCyclesCallError((call_error.0 as u32, call_error.1))));    
            }
        }
    
    }
    
    with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
    Ok(UserBurnIcpMintCyclesSuccess{
        mint_cycles_for_the_user: cycles_for_the_user_canister,
        cts_fee_taken: match user_burn_icp_mint_cycles_data.cmc_cycles.unwrap().checked_sub(user_burn_icp_mint_cycles_data.user_burn_icp_mint_cycles_fee) {
            Some(_) => user_burn_icp_mint_cycles_data.user_burn_icp_mint_cycles_fee,
            None => 0
        }
    })
    
}



// ---------------------------------------

#[derive(CandidType, Deserialize, Clone)]
pub struct UserTransferIcpData{
    start_time_nanos: u64,
    lock: bool,
    user_transfer_icp_quest: UserTransferIcpQuest, 
    cts_transfer_icp_fee: Option<IcpTokens>, //:0.05-xdr with the base on the current-rate.
    icp_transfer_block_height: Option<IcpBlockHeight>,
    cts_fee_taken: bool,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct UserTransferIcpQuest {
    memo: IcpMemo,
    icp: IcpTokens,
    to: IcpId,
}

#[derive(CandidType, Deserialize)]
pub enum UserTransferIcpError{
    UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: bool },
    UserIsInTheMiddleOfAUserTransferIcpCall{ must_call_complete: bool },
    UserIsInTheMiddleOfAUserBurnIcpMintCyclesCall{ must_call_complete: bool },
    CheckIcpBalanceCallError((u32, String)),
    CTSIsBusy,
    CheckCurrentXdrPerMyriadPerIcpCmcRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    UserIcpLedgerBalanceTooLow{
        user_icp_ledger_balance: IcpTokens,
        icp_ledger_transfer_fee: IcpTokens,
        cts_transfer_icp_fee: IcpTokens, // calculate by the current xdr-icp rate 
    },
    IcpTransferCallError((u32, String)),
    IcpTransferError(IcpTransferError),
    MidCallError(UserTransferIcpMidCallError)
}

#[derive(CandidType, Deserialize)]
pub enum UserTransferIcpMidCallError{
    CollectCTSFeeIcpTransferCallError((u32, String)),
    CollectCTSFeeIcpTransferError(IcpTransferError)
}

#[update]
pub async fn user_transfer_icp(q: UserTransferIcpQuest) -> Result<IcpBlockHeight, UserTransferIcpError> {

    let user_id: Principal = caller();
        
    let user_transfer_icp_data = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_transfer_icp.get(&user_id) {
            Some(user_transfer_icp_data) => {
                return Err(UserTransferIcpError::UserIsInTheMiddleOfAUserTransferIcpCall{ must_call_complete: !user_transfer_icp_data.lock });
            },
            None => {
                if localkey::cell::get(&STOP_CALLS) { trap("maintenance, try soon.") }
                if let Some(purchase_cycles_bank_data) = cts_data.cycles_bank_purchases.get(&user_id) {
                    return Err(UserTransferIcpError::UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock });
                }
                if let Some(user_burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
                    return Err(UserTransferIcpError::UserIsInTheMiddleOfAUserBurnIcpMintCyclesCall{ must_call_complete: !user_burn_icp_mint_cycles_data.lock });   
                }
                if cts_data.users_transfer_icp.len() >= MAX_USERS_TRANSFER_ICP {
                    return Err(UserTransferIcpError::CTSIsBusy);
                }
                let user_transfer_icp_data: UserTransferIcpData = UserTransferIcpData{
                    start_time_nanos: time(),
                    lock: true,
                    user_transfer_icp_quest: q,
                    cts_transfer_icp_fee: None,
                    icp_transfer_block_height: None,
                    cts_fee_taken: false,
                };
                cts_data.users_transfer_icp.insert(user_id, user_transfer_icp_data.clone());
                Ok(user_transfer_icp_data)
            }
        }
    })?;
        
    user_transfer_icp_(user_id, user_transfer_icp_data).await
}

#[derive(CandidType, Deserialize)]
pub enum CompleteUserTransferIcpError{
    UserNotFoundInTheUsersTransferIcpMap,
    UserTransferIcpError(UserTransferIcpError)
}

#[update]
pub async fn complete_user_transfer_icp() -> Result<IcpBlockHeight, CompleteUserTransferIcpError> {
    
    let user_id: Principal = caller();
    
    complete_user_transfer_icp_(user_id).await

}

async fn complete_user_transfer_icp_(user_id: Principal) -> Result<IcpBlockHeight, CompleteUserTransferIcpError> {
    
    let user_transfer_icp_data: UserTransferIcpData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_transfer_icp.get_mut(&user_id) {
            Some(user_transfer_icp_data) => {
                if user_transfer_icp_data.lock == true {
                    return Err(CompleteUserTransferIcpError::UserTransferIcpError(UserTransferIcpError::UserIsInTheMiddleOfAUserTransferIcpCall{ must_call_complete: false }));
                }
                user_transfer_icp_data.lock = true;
                Ok(user_transfer_icp_data.clone())
            },
            None => {
                return Err(CompleteUserTransferIcpError::UserNotFoundInTheUsersTransferIcpMap);
            }
        }
    })?;

    user_transfer_icp_(user_id, user_transfer_icp_data).await
        .map_err(|user_transfer_icp_error| { 
            CompleteUserTransferIcpError::UserTransferIcpError(user_transfer_icp_error) 
        })
    
}


async fn user_transfer_icp_(user_id: Principal, mut user_transfer_icp_data: UserTransferIcpData) -> Result<IcpBlockHeight, UserTransferIcpError> {

    if user_transfer_icp_data.cts_transfer_icp_fee.is_none() {

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
                return Err(UserTransferIcpError::CheckIcpBalanceCallError((check_balance_call_error.0 as u32, check_balance_call_error.1)));
            }
        };
                
        let current_xdr_icp_rate: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate_sponse {
            Ok(rate) => rate,
            Err(check_xdr_icp_rate_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
                return Err(UserTransferIcpError::CheckCurrentXdrPerMyriadPerIcpCmcRateError(check_xdr_icp_rate_error));
            }
        };
        
        let cts_transfer_icp_fee: IcpTokens = cycles_to_icptokens(CTS_TRANSFER_ICP_FEE, current_xdr_icp_rate);
        
        if user_icp_ledger_balance < user_transfer_icp_data.user_transfer_icp_quest.icp + cts_transfer_icp_fee + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) {
            with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
            return Err(UserTransferIcpError::UserIcpLedgerBalanceTooLow{
                user_icp_ledger_balance,
                icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                cts_transfer_icp_fee,
            });
        }
        
        user_transfer_icp_data.cts_transfer_icp_fee = Some(cts_transfer_icp_fee);
    }
    
    if user_transfer_icp_data.icp_transfer_block_height.is_none() {
        
        match icp_transfer(
            MAINNET_LEDGER_CANISTER_ID,
            IcpTransferArgs {
                memo: user_transfer_icp_data.user_transfer_icp_quest.memo,
                amount: user_transfer_icp_data.user_transfer_icp_quest.icp,
                fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                from_subaccount: Some(principal_icp_subaccount(&user_id)),
                to: user_transfer_icp_data.user_transfer_icp_quest.to,
                created_at_time: Some(IcpTimestamp { timestamp_nanos: time() - 1_000_000_000 })
            }
        ).await {
            Ok(transfer_result) => match transfer_result {
                Ok(block_height) => {
                    user_transfer_icp_data.icp_transfer_block_height = Some(block_height);
                },
                Err(icp_transfer_error) => {
                    with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
                    return Err(UserTransferIcpError::IcpTransferError(icp_transfer_error));                    
                }
            },
            Err(icp_transfer_call_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
                return Err(UserTransferIcpError::IcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1)));
            }
        }
        
    }
    
    if user_transfer_icp_data.cts_fee_taken == false {
        match take_user_icp_ledger(&user_id, user_transfer_icp_data.cts_transfer_icp_fee.unwrap()).await {
            Ok(icp_transfer_result) => match icp_transfer_result {
                Ok(_block_height) => {
                    user_transfer_icp_data.cts_fee_taken = true;
                },
                Err(icp_transfer_error) => {
                    user_transfer_icp_data.lock = false;
                    with_mut(&CTS_DATA, |cts_data| {
                        if let Some(data) = cts_data.users_transfer_icp.get_mut(&user_id) {
                            *data = user_transfer_icp_data;
                        }
                    });
                    return Err(UserTransferIcpError::MidCallError(UserTransferIcpMidCallError::CollectCTSFeeIcpTransferError(icp_transfer_error)));          
                }
            }, 
            Err(icp_transfer_call_error) => {
                user_transfer_icp_data.lock = false;
                with_mut(&CTS_DATA, |cts_data| {
                    if let Some(data) = cts_data.users_transfer_icp.get_mut(&user_id) {
                        *data = user_transfer_icp_data;
                    }
                });
                return Err(UserTransferIcpError::MidCallError(UserTransferIcpMidCallError::CollectCTSFeeIcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1))));                
            }               
        }
    }
    
    
    with_mut(&CTS_DATA, |cts_data| { cts_data.users_transfer_icp.remove(&user_id); });
    Ok(user_transfer_icp_data.icp_transfer_block_height.unwrap())
}


















// --------------------------------------------------------------------------
// :CONTROLLER-METHODS.





// ----- USERS_MAP_CANISTERS-METHODS --------------------------



#[update]
pub fn controller_put_umc_code(canister_code: CanisterCode) -> () {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    if sha256(canister_code.module()) != *canister_code.module_hash() {
        trap("Given canister_code.module_hash is different than the manual compute module hash");
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.cbs_map_canister_code = canister_code;
    });
}




// certification? or replication-calls?
#[export_name = "canister_query controller_see_users_map_canisters"]
pub fn controller_see_users_map_canisters() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(&Vec<Principal>,)>((&(cts_data.cbs_maps),));
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
pub async fn controller_upgrade_umcs(opt_upgrade_umcs: Option<Vec<Principal>>, post_upgrade_arg: Vec<u8>) -> Vec<ControllerUpgradeUMCError>/*umcs that upgrade call-fail*/ {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    if with(&CTS_DATA, |cts_data| cts_data.cbs_map_canister_code.module().len() == 0 ) {
        trap("USERS_MAP_CANISTER_CODE.module().len() is 0.")
    }
    
    let upgrade_umcs: Vec<Principal> = {
        if let Some(upgrade_umcs) = opt_upgrade_umcs {
            with(&CTS_DATA, |cts_data| { 
                upgrade_umcs.iter().for_each(|upgrade_umc| {
                    if cts_data.cbs_maps.contains(&upgrade_umc) == false {
                        trap(&format!("cts cbs_maps does not contain: {:?}", upgrade_umc));
                    }
                });
            });    
            upgrade_umcs
        } else {
            with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.clone() })
        }
    };     
    
    let sponses: Vec<Result<(), ControllerUpgradeUMCError>> = futures::future::join_all(
        upgrade_umcs.iter().map(|umc_id| {
            async {
            
                match call::<(CanisterIdRecord,), ()>(
                    MANAGEMENT_CANISTER_ID,
                    "stop_canister",
                    (CanisterIdRecord{ canister_id: *umc_id/*copy*/ },)
                ).await {
                    Ok(_) => {},
                    Err(stop_canister_call_error) => {
                        return Err((*umc_id/*copy*/, ControllerUpgradeUMCCallErrorType::StopCanisterCallError, (stop_canister_call_error.0 as u32, stop_canister_call_error.1))); 
                    }
                }
            
                match call_raw128(
                    MANAGEMENT_CANISTER_ID,
                    "install_code",
                    &encode_one(&ManagementCanisterInstallCodeQuest{
                        mode : ManagementCanisterInstallCodeMode::upgrade,
                        canister_id : *umc_id/*copy*/,
                        wasm_module : unsafe {&*with(&CTS_DATA, |cts_data| { cts_data.cbs_map_canister_code.module() as *const Vec<u8> })},
                        arg : &post_upgrade_arg,
                    }).unwrap(),
                    0
                ).await {
                    Ok(_) => {},
                    Err(upgrade_code_call_error) => {
                        return Err((*umc_id/*copy*/, ControllerUpgradeUMCCallErrorType::UpgradeCodeCallError, (upgrade_code_call_error.0 as u32, upgrade_code_call_error.1)));
                    }
                }

                match call::<(CanisterIdRecord,), ()>(
                    MANAGEMENT_CANISTER_ID,
                    "start_canister",
                    (CanisterIdRecord{ canister_id: *umc_id/*copy*/ },)
                ).await {
                    Ok(_) => {},
                    Err(start_canister_call_error) => {
                        return Err((umc_id.clone(), ControllerUpgradeUMCCallErrorType::StartCanisterCallError, (start_canister_call_error.0 as u32, start_canister_call_error.1))); 
                    }
                }
                
                Ok(())
            }
        }).collect::<Vec<_>>()
    ).await;
    
    
    sponses.into_iter().filter_map(
        |upgrade_umc_sponse: Result<(), ControllerUpgradeUMCError>| {
            match upgrade_umc_sponse {
                Ok(_) => None,
                Err(upgrade_umc_error) => Some(upgrade_umc_error)
            }
        }
    ).collect::<Vec<ControllerUpgradeUMCError>>()
    
}




// ----- USER_CANISTERS-METHODS --------------------------


#[update]
pub fn controller_put_user_canister_code(canister_code: CanisterCode) -> () {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    if sha256(canister_code.module()) != *canister_code.module_hash() {
        trap("Given canister_code.module_hash is different than the manual compute module hash");
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.cycles_bank_canister_code = canister_code;
    });
}



pub type ControllerPutUCCodeOntoTheUMCError = (Principal, (u32, String));

#[update]
pub async fn controller_put_uc_code_onto_the_umcs(opt_umcs: Option<Vec<Principal>>) -> Vec<ControllerPutUCCodeOntoTheUMCError>/*umcs that the put_uc_code call fail*/ {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
        
    if with(&CTS_DATA, |cts_data| cts_data.cycles_bank_canister_code.module().len() == 0 ) {
        trap("USER_CANISTER_CODE.module().len() is 0.")
    }
    
    let call_umcs: Vec<Principal> = {
        if let Some(call_umcs) = opt_umcs {
            with(&CTS_DATA, |cts_data| { 
                call_umcs.iter().for_each(|call_umc| {
                    if cts_data.cbs_maps.contains(&call_umc) == false {
                        trap(&format!("cts cbs_maps does not contain: {:?}", call_umc));
                    }
                });
            });    
            call_umcs
        } else {
            with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.clone() })
        }
    };    
    
    let sponses: Vec<Result<(), ControllerPutUCCodeOntoTheUMCError>> = futures::future::join_all(
        call_umcs.iter().map(|call_umc| {
            async {
                match call::<(&CanisterCode,), ()>(
                    *call_umc,
                    "cts_put_user_canister_code",
                    (unsafe{&*with(&CTS_DATA, |cts_data| { &(cts_data.cycles_bank_canister_code) as *const CanisterCode })},)
                ).await {
                    Ok(_) => {},
                    Err(call_error) => {
                        return Err((call_umc.clone(), (call_error.0 as u32, call_error.1)));
                    }
                }
                
                Ok(())
            }
        }).collect::<Vec<_>>()
    ).await;
    
    sponses.into_iter().filter_map(
        |call_umc_sponse: Result<(), ControllerPutUCCodeOntoTheUMCError>| {
            match call_umc_sponse {
                Ok(()) => None,
                Err(call_umc_error) => Some(call_umc_error)
            }
        }
    ).collect::<Vec<ControllerPutUCCodeOntoTheUMCError>>()
}



#[derive(CandidType, Deserialize)]
pub enum ControllerUpgradeUCSOnAUMCError {
    CTSUpgradeUCSCallError((u32, String))
}



#[update]
pub async fn controller_upgrade_ucs_on_a_umc(umc: Principal, opt_upgrade_ucs: Option<Vec<Principal>>, post_upgrade_arg: Vec<u8>) -> Result<Option<Vec<CBSMUpgradeCBError>>, ControllerUpgradeUCSOnAUMCError> {       // /*:chunk-0 of the ucs that upgrade-fail*/ 
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    if with(&CTS_DATA, |cts_data| { cts_data.cbs_maps.contains(&umc) }) == false {
        trap(&format!("cts cbs_maps does not contain: {:?}", umc));
    }
    
    match call::<(Option<Vec<Principal>>, Vec<u8>/*post-upgrade-arg*/), (Option<Vec<CBSMUpgradeCBError>>,)>(
        umc,
        "cts_upgrade_ucs_chunk",
        (opt_upgrade_ucs, post_upgrade_arg)
    ).await {
        Ok((opt_uc_upgrade_fails,)) => Ok(opt_uc_upgrade_fails),
        Err(call_error) => Err(ControllerUpgradeUCSOnAUMCError::CTSUpgradeUCSCallError((call_error.0 as u32, call_error.1)))
    }

}






// ----- CYCLES_TRANSFERRER_CANISTERS-METHODS --------------------------


#[update]
pub fn controller_put_ctc_code(canister_code: CanisterCode) -> () {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    if sha256(canister_code.module()) != *canister_code.module_hash() {
        trap("Given canister_code.module_hash is different than the manual compute module hash");
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.cycles_transferrer_canister_code = canister_code;
    });
}




#[export_name = "canister_query controller_see_cycles_transferrer_canisters"]
pub fn controller_see_cycles_transferrer_canisters() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(&Vec<Principal>,)>((&(cts_data.cycles_transferrer_canisters),));
    });
}




#[update]
pub fn controller_put_cycles_transferrer_canisters(mut put_cycles_transferrer_canisters: Vec<Principal>) {
    
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        for put_cycles_transferrer_canister in put_cycles_transferrer_canisters.iter() {
            if cts_data.cycles_transferrer_canisters.contains(put_cycles_transferrer_canister) {
                trap(&format!("{:?} already in the cycles_transferrer_canisters list", put_cycles_transferrer_canister));
            }
        }
        cts_data.cycles_transferrer_canisters.append(&mut put_cycles_transferrer_canisters);
    });
}



#[derive(CandidType, Deserialize)]
pub enum ControllerCreateNewCyclesTransferrerCanisterError {
    GetNewCanisterError(GetNewCanisterError),
    CyclesTransferrerCanisterCodeNotFound,
    InstallCodeCallError((u32, String))
}


#[update]
pub async fn controller_create_new_cycles_transferrer_canister() -> Result<Principal, ControllerCreateNewCyclesTransferrerCanisterError> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    let new_cycles_transferrer_canister_id: Principal = match get_new_canister(
        None,
        3_000_000_000_000/*7_000_000_000_000*/
    ).await {
        Ok(new_canister_id) => new_canister_id,
        Err(get_new_canister_error) => return Err(ControllerCreateNewCyclesTransferrerCanisterError::GetNewCanisterError(get_new_canister_error))
    };
    
    // on errors after here make sure to put the new_canister into the NEW_CANISTERS list
    
    // install code
    if with(&CTS_DATA, |cts_data| cts_data.cycles_transferrer_canister_code.module().len() == 0 ) {
        with_mut(&CTS_DATA, |cts_data| { cts_data.canisters_for_the_use.insert(new_cycles_transferrer_canister_id); });
        return Err(ControllerCreateNewCyclesTransferrerCanisterError::CyclesTransferrerCanisterCodeNotFound);
    }
    
    match call::<(ManagementCanisterInstallCodeQuest,), ()>(
        MANAGEMENT_CANISTER_ID,
        "install_code",
        (ManagementCanisterInstallCodeQuest{
            mode : ManagementCanisterInstallCodeMode::install,
            canister_id : new_cycles_transferrer_canister_id,
            wasm_module : unsafe{&*with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canister_code.module() as *const Vec<u8> })},
            arg : &encode_one(&CyclesTransferrerCanisterInit{
                cts_id: id()
            }).unwrap() // unwrap or return Err(candiderror); 
        },)
    ).await {
        Ok(_) => {
            with_mut(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canisters.push(new_cycles_transferrer_canister_id); }); 
            Ok(new_cycles_transferrer_canister_id)    
        },
        Err(install_code_call_error) => {
            with_mut(&CTS_DATA, |cts_data| { cts_data.canisters_for_the_use.insert(new_cycles_transferrer_canister_id); });
            return Err(ControllerCreateNewCyclesTransferrerCanisterError::InstallCodeCallError((install_code_call_error.0 as u32, install_code_call_error.1)));
        }
    }
      
} 



/*
#[update]
pub fn controller_take_away_cycles_transferrer_canisters(take_away_ctcs: Vec<Principal>) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with_mut(&CYCLES_TRANSFERRER_CANISTERS, |ctcs| {
        for take_away_ctc in take_away_ctcs.iter() {
            match ctcs.binary_search(take_away_ctc) {
                Ok(take_away_ctc_i) => {
                    ctcs.remove(take_away_ctc_i);
                },
                Err(_) => {
                    trap(&format!("{:?} is not one of the cycles_transferrer canisters in the CTS", take_away_ctc)); // rollback 
                }
            }
        }
    });    
}
*/

/*

#[update]
pub async fn controller_see_cycles_transferrer_canister_re_try_cycles_transferrer_user_transfer_cycles_callbacks(cycles_transferrer_canister_id: Principal) -> Result<Vec<ReTryCyclesTransferrerUserTransferCyclesCallback>, (u32, String)> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    if with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canisters.contains(&cycles_transferrer_canister_id) == false }) {
        trap(&format!("cts cycles_transferrer_canisters does not contain: {:?}", cycles_transferrer_canister_id));
    }
    
    match call::<(), (Vec<ReTryCyclesTransferrerUserTransferCyclesCallback>,)>(
        cycles_transferrer_canister_id,
        "cts_see_re_try_cycles_transferrer_user_transfer_cycles_callbacks",
        ()
    ).await {
        Ok((re_try_cycles_transferrer_user_transfer_cycles_callbacks,)) => Ok(re_try_cycles_transferrer_user_transfer_cycles_callbacks),
        Err(call_error) => Err((call_error.0 as u32, call_error.1))
    }

}


#[update]
pub async fn controller_do_cycles_transferrer_canister_re_try_cycles_transferrer_user_transfer_cycles_callbacks(cycles_transferrer_canister_id: Principal) -> Result<Vec<ReTryCyclesTransferrerUserTransferCyclesCallback>, (u32, String)> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    if with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canisters.contains(&cycles_transferrer_canister_id) == false }) {
        trap(&format!("cts cycles_transferrer_canisters does not contain: {:?}", cycles_transferrer_canister_id))
    }
    
    match call::<(), (Vec<ReTryCyclesTransferrerUserTransferCyclesCallback>,)>(
        cycles_transferrer_canister_id,
        "cts_re_try_cycles_transferrer_user_transfer_cycles_callbacks",
        ()
    ).await {
        Ok((re_try_cycles_transferrer_user_transfer_cycles_callbacks,)) => Ok(re_try_cycles_transferrer_user_transfer_cycles_callbacks),
        Err(call_error) => Err((call_error.0 as u32, call_error.1))
    }


}


#[update]
pub async fn controller_drain_cycles_transferrer_canister_re_try_cycles_transferrer_user_transfer_cycles_callbacks(cycles_transferrer_canister_id: Principal) -> Result<Vec<ReTryCyclesTransferrerUserTransferCyclesCallback>, (u32, String)> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    if with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canisters.contains(&cycles_transferrer_canister_id) == false }) {
        trap(&format!("cts cycles_transferrer_canisters does not contain: {:?}", cycles_transferrer_canister_id));
    }
    
    match call::<(), (Vec<ReTryCyclesTransferrerUserTransferCyclesCallback>,)>(
        cycles_transferrer_canister_id,
        "cts_drain_re_try_cycles_transferrer_user_transfer_cycles_callbacks",
        ()
    ).await {
        Ok((re_try_cycles_transferrer_user_transfer_cycles_callbacks,)) => Ok(re_try_cycles_transferrer_user_transfer_cycles_callbacks),
        Err(call_error) => Err((call_error.0 as u32, call_error.1))
    }

}



*/




pub type ControllerUpgradeCTCError = (Principal, ControllerUpgradeCTCCallErrorType, (u32, String)); 

#[derive(CandidType, Deserialize)]
pub enum ControllerUpgradeCTCCallErrorType {
    StopCanisterCallError,
    UpgradeCodeCallError,
    StartCanisterCallError
}



// we upgrade the ctcs one at a time because if one of them takes too long to stop, we dont want to wait for it to come back, we will stop_calls on the cycles_transferrer, wait an hour, uninstall, and reinstall
#[update]
pub async fn controller_upgrade_ctc(upgrade_ctc: Principal, post_upgrade_arg: Vec<u8>) -> Result<(), ControllerUpgradeCTCError> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }

    if with(&CTS_DATA, |cts_data| cts_data.cycles_transferrer_canister_code.module().len() == 0 ) {
        trap("CYCLES_TRANSFERRER_CANISTER_CODE.module().len() is 0.")
    }
    
    if with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canisters.contains(&upgrade_ctc) == false }) {
        trap(&format!("cts cycles_transferrer_canisters does not contain: {:?}", upgrade_ctc));
    }
       
    match call::<(CanisterIdRecord,), ()>(
        MANAGEMENT_CANISTER_ID,
        "stop_canister",
        (CanisterIdRecord{ canister_id: upgrade_ctc },)
    ).await {
        Ok(_) => {},
        Err(stop_canister_call_error) => {
                
            // set stop_calls_flag , wait an hour, then re-try the [re]maining re_try-cycles_transferrer_user_transfer_cycles_callbacks till 0 left, then uninstall the canister and install . 

            
            return Err((upgrade_ctc, ControllerUpgradeCTCCallErrorType::StopCanisterCallError, (stop_canister_call_error.0 as u32, stop_canister_call_error.1))); 
        }
    }

    match call_raw128(
        MANAGEMENT_CANISTER_ID,
        "install_code",
        &encode_one(&ManagementCanisterInstallCodeQuest{
            mode : ManagementCanisterInstallCodeMode::upgrade,
            canister_id : upgrade_ctc,
            wasm_module : unsafe{&*with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canister_code.module() as *const Vec<u8> })},
            arg : &post_upgrade_arg,
        }).unwrap(),
        0
    ).await {
        Ok(_) => {},
        Err(upgrade_code_call_error) => {
            return Err((upgrade_ctc, ControllerUpgradeCTCCallErrorType::UpgradeCodeCallError, (upgrade_code_call_error.0 as u32, upgrade_code_call_error.1)));
        }
    }

    match call::<(CanisterIdRecord,), ()>(
        MANAGEMENT_CANISTER_ID,
        "start_canister",
        (CanisterIdRecord{ canister_id: upgrade_ctc },)
    ).await {
        Ok(_) => {},
        Err(start_canister_call_error) => {
            return Err((upgrade_ctc, ControllerUpgradeCTCCallErrorType::StartCanisterCallError, (start_canister_call_error.0 as u32, start_canister_call_error.1))); 
        }
    }
    
    Ok(())
    
}










// ----- NEW_USERS-METHODS --------------------------

#[export_name = "canister_query controller_see_cycles_bank_purchases"]
pub fn controller_see_cycles_bank_purchases() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &PurchaseCyclesBankData)>,)>((cts_data.cycles_bank_purchases.iter().collect::<Vec<(&Principal, &PurchaseCyclesBankData)>>(),));
    });
}

// put new user data
#[update]
pub fn controller_put_purchase_cycles_bank_data(new_user_id: Principal, put_data: PurchaseCyclesBankData, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(purchase_cycles_bank_data) = cts_data.cycles_bank_purchases.get(&new_user_id) {
            if purchase_cycles_bank_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the cycles_bank_purchases. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.cycles_bank_purchases.insert(new_user_id, put_data);
    });

}
// remove new user
#[update]
pub fn controller_remove_new_user(new_user_id: Principal, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(purchase_cycles_bank_data) = cts_data.cycles_bank_purchases.get(&new_user_id) {
            if purchase_cycles_bank_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the cycles_bank_purchases. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.cycles_bank_purchases.remove(&new_user_id);
    });
}


#[update]
pub async fn controller_complete_cycles_bank_purchases(opt_complete_cycles_bank_purchases_ids: Option<Vec<Principal>>) -> Vec<(Principal, Result<PurchaseCyclesBankSuccess, CompletePurchaseCyclesBankError>)> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }

    let complete_cycles_bank_purchases_ids: Vec<Principal> = match opt_complete_cycles_bank_purchases_ids {
        Some(complete_cycles_bank_purchases_ids) => complete_cycles_bank_purchases_ids,
        None => {
            with(&CTS_DATA, |cts_data| { 
                cts_data.cycles_bank_purchases.iter()
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
        complete_cycles_bank_purchases_ids.iter().map(
            |complete_new_user_id: &Principal| {
                complete_purchase_cycles_bank_(complete_new_user_id.clone())
            }
        ).collect::<Vec<_>>()
    ).await;
    
    complete_cycles_bank_purchases_ids.into_iter().zip(rs.into_iter()).collect::<Vec<(Principal, Result<PurchaseCyclesBankSuccess,CompletePurchaseCyclesBankError>)>>()
    
}




// ------ UserBurnIcpMintCycles-METHODS -----------------


#[export_name = "canister_query controller_see_users_burn_icp_mint_cycles"]
pub fn controller_see_users_burn_icp_mint_cycles() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &UserBurnIcpMintCyclesData)>,)>((cts_data.users_burn_icp_mint_cycles.iter().collect::<Vec<(&Principal, &UserBurnIcpMintCyclesData)>>(),));
    });
}

#[update]
pub fn controller_put_user_burn_icp_mint_cycles_data(user_id: Principal, put_data: UserBurnIcpMintCyclesData, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(user_burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
            if user_burn_icp_mint_cycles_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_burn_icp_mint_cycles. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_burn_icp_mint_cycles.insert(user_id, put_data);
    });

}

#[update]
pub fn controller_remove_user_burn_icp_mint_cycles(user_id: Principal, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(user_burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
            if user_burn_icp_mint_cycles_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_burn_icp_mint_cycles. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_burn_icp_mint_cycles.remove(&user_id);
    });
}


#[update]
pub async fn controller_complete_users_burn_icp_mint_cycles(opt_complete_users_ids: Option<Vec<Principal>>) -> Vec<(Principal, Result<UserBurnIcpMintCyclesSuccess, CompleteUserBurnIcpMintCyclesError>)> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }

    let complete_users_ids: Vec<Principal> = match opt_complete_users_ids {
        Some(complete_users_ids) => complete_users_ids,
        None => {
            with(&CTS_DATA, |cts_data| { 
                cts_data.users_burn_icp_mint_cycles.iter()
                .filter(|&(_user_id, user_burn_icp_mint_cycles_data): &(&Principal, &UserBurnIcpMintCyclesData)| {
                    user_burn_icp_mint_cycles_data.lock == false
                })
                .map(|(user_id, _user_burn_icp_mint_cycles_data): (&Principal, &UserBurnIcpMintCyclesData)| {
                    user_id.clone()
                })
                .collect::<Vec<Principal>>()
            })
        }
    };
    
    let rs: Vec<Result<UserBurnIcpMintCyclesSuccess, CompleteUserBurnIcpMintCyclesError>> = futures::future::join_all(
        complete_users_ids.iter().map(
            |complete_user_id: &Principal| {
                complete_user_burn_icp_mint_cycles_(complete_user_id.clone())
            }
        ).collect::<Vec<_>>()
    ).await;
    
    complete_users_ids.into_iter().zip(rs.into_iter()).collect::<Vec<(Principal, Result<UserBurnIcpMintCyclesSuccess, CompleteUserBurnIcpMintCyclesError>)>>()

}



// ------ UsersTransferIcp-METHODS -----------------


#[export_name = "canister_query controller_see_users_transfer_icp"]
pub fn controller_see_users_transfer_icp() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &UserTransferIcpData)>,)>((cts_data.users_transfer_icp.iter().collect::<Vec<(&Principal, &UserTransferIcpData)>>(),));
    });
}

#[update]
pub fn controller_put_user_transfer_icp_data(user_id: Principal, put_data: UserTransferIcpData, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(user_transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
            if user_transfer_icp_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_transfer_icp. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_transfer_icp.insert(user_id, put_data);
    });

}

#[update]
pub fn controller_remove_user_transfer_icp(user_id: Principal, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(user_transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
            if user_transfer_icp_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_transfer_icp. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_transfer_icp.remove(&user_id);
    });
}


#[update]
pub async fn controller_complete_users_transfer_icp(opt_complete_users_ids: Option<Vec<Principal>>) -> Vec<(Principal, Result<IcpBlockHeight, CompleteUserTransferIcpError>)> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }

    let complete_users_ids: Vec<Principal> = match opt_complete_users_ids {
        Some(complete_users_ids) => complete_users_ids,
        None => {
            with(&CTS_DATA, |cts_data| { 
                cts_data.users_transfer_icp.iter()
                .filter(|&(_user_id, user_transfer_icp_data): &(&Principal, &UserTransferIcpData)| {
                    user_transfer_icp_data.lock == false
                })
                .map(|(user_id, _user_transfer_icp_data): (&Principal, &UserTransferIcpData)| {
                    user_id.clone()
                })
                .collect::<Vec<Principal>>()
            })
        }
    };
    
    let rs: Vec<Result<IcpBlockHeight, CompleteUserTransferIcpError>> = futures::future::join_all(
        complete_users_ids.iter().map(
            |complete_user_id: &Principal| {
                complete_user_transfer_icp_(complete_user_id.clone())
            }
        ).collect::<Vec<_>>()
    ).await;
    
    complete_users_ids.into_iter().zip(rs.into_iter()).collect::<Vec<(Principal, Result<IcpBlockHeight, CompleteUserTransferIcpError>)>>()

}







// ----- NEW_CANISTERS-METHODS --------------------------

#[update]
pub fn controller_put_canisters_for_the_use(canisters_for_the_use: Vec<Principal>) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with_mut(&CTS_DATA, |cts_data| {
        for canister_for_the_use in canisters_for_the_use.into_iter() {
            cts_data.canisters_for_the_use.insert(canister_for_the_use);
        }
    });
}

#[export_name = "canister_query controller_see_canisters_for_the_use"]
pub fn controller_see_canisters_for_the_use() -> () {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(&HashSet<Principal>,)>((&(cts_data.canisters_for_the_use),));
    });

}







// ----- STOP_CALLS-METHODS --------------------------

#[update]
pub fn controller_set_stop_calls_flag(stop_calls_flag: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.set(stop_calls_flag); });
}

#[query]
pub fn controller_see_stop_calls_flag() -> bool {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.get() })
}







// ----- STATE_SNAPSHOT_CTS_DATA_CANDID_BYTES-METHODS --------------------------

#[update]
pub fn controller_create_state_snapshot() -> u64/*len of the state_snapshot_candid_bytes*/ {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTS_DATA_CANDID_BYTES, |state_snapshot_cts_data_candid_bytes| {
        *state_snapshot_cts_data_candid_bytes = create_cts_data_candid_bytes();
        state_snapshot_cts_data_candid_bytes.len() as u64
    })
}


// chunk_size = 1mib
#[export_name = "canister_query controller_download_state_snapshot"]
pub fn controller_download_state_snapshot() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    let chunk_size: usize = 1024*1024;
    with(&STATE_SNAPSHOT_CTS_DATA_CANDID_BYTES, |state_snapshot_cts_data_candid_bytes| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_cts_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    });
}



#[update]
pub fn controller_clear_state_snapshot() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTS_DATA_CANDID_BYTES, |state_snapshot_cts_data_candid_bytes| {
        *state_snapshot_cts_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn controller_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTS_DATA_CANDID_BYTES, |state_snapshot_cts_data_candid_bytes| {
        state_snapshot_cts_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn controller_re_store_cts_data_out_of_the_state_snapshot() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    re_store_cts_data_candid_bytes(
        with_mut(&STATE_SNAPSHOT_CTS_DATA_CANDID_BYTES, |state_snapshot_cts_data_candid_bytes| {
            let mut v: Vec<u8> = Vec::new();
            v.append(state_snapshot_cts_data_candid_bytes);  // moves the bytes out of the state_snapshot vec
            v
        })
    );

}




// ----- SET_&_SEE_CTS_CONTROLLERS-METHODS --------------------------

#[query(manual_reply = true)]
pub fn controller_see_controllers() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| { 
        reply::<(&Vec<Principal>,)>((&(cts_data.controllers),)); 
    })
}


#[update]
pub fn controller_set_new_controller(set_controller: Principal) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with_mut(&CTS_DATA, |cts_data| { cts_data.controllers.push(set_controller); });
}









// ----- CONTROLLER_CALL_CANISTER-METHOD --------------------------

#[derive(CandidType, Deserialize)]
pub struct ControllerCallCanisterQuest {
    callee: Principal,
    method_name: String,
    arg_raw: Vec<u8>,
    cycles: Cycles
}

#[update(manual_reply = true)]
pub async fn controller_call_canister() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
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
    canisters_for_the_use_count: u64,
    users_map_canister_code_hash: Option<[u8; 32]>,
    user_canister_code_hash: Option<[u8; 32]>,
    cycles_transferrer_canister_code_hash: Option<[u8; 32]>,
    users_map_canisters_count: u64,
    cycles_transferrer_canisters_count: u64,
    latest_known_cmc_rate: IcpXdrConversionRate,
    cycles_bank_purchases_count: u64,
    users_burn_icp_mint_cycles_count: u64,
}


#[query]
pub fn controller_see_metrics() -> CTSMetrics {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with(&CTS_DATA, |cts_data| {
        CTSMetrics {
            global_allocator_counter: get_allocated_bytes_count() as u64,
            stable_size: ic_cdk::api::stable::stable64_size(),
            cycles_balance: ic_cdk::api::canister_balance128(),
            canisters_for_the_use_count: cts_data.canisters_for_the_use.len() as u64,
            users_map_canister_code_hash: if cts_data.cbs_map_canister_code.module().len() != 0 { Some(cts_data.cbs_map_canister_code.module_hash().clone()) } else { None },
            user_canister_code_hash: if cts_data.cycles_bank_canister_code.module().len() != 0 { Some(cts_data.cycles_bank_canister_code.module_hash().clone()) } else { None },
            cycles_transferrer_canister_code_hash: if cts_data.cycles_transferrer_canister_code.module().len() != 0 { Some(cts_data.cycles_transferrer_canister_code.module_hash().clone()) } else { None },
            users_map_canisters_count: cts_data.cbs_maps.len() as u64,
            cycles_transferrer_canisters_count: cts_data.cycles_transferrer_canisters.len() as u64,
            latest_known_cmc_rate: LATEST_KNOWN_CMC_RATE.with(|cr| cr.get()),
            cycles_bank_purchases_count: cts_data.cycles_bank_purchases.len() as u64,
            users_burn_icp_mint_cycles_count: cts_data.users_burn_icp_mint_cycles.len() as u64
            
        }
    })
}





// ---------------------------- :FRONTCODE. -----------------------------------


#[update]
pub fn controller_upload_frontcode_file_chunks(file_path: String, file: File) -> () {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&FRONTCODE_FILES_HASHES, |ffhs| {
        ffhs.insert(file_path.clone(), sha256(&file.content));
        set_root_hash(ffhs);
    });
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.frontcode_files.insert(file_path, file); 
    });
}


#[update]
pub fn controller_clear_frontcode_files() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    
    with_mut(&CTS_DATA, |cts_data| {
        cts_data.frontcode_files = Files::new();
    });

    with_mut(&FRONTCODE_FILES_HASHES, |ffhs| {
        *ffhs = FilesHashes::new();
        set_root_hash(ffhs);
    });
}


#[query]
pub fn controller_get_file_hashes() -> Vec<(String, [u8; 32])> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with(&FRONTCODE_FILES_HASHES, |file_hashes| { 
        let mut vec = Vec::<(String, [u8; 32])>::new();
        file_hashes.for_each(|k,v| {
            vec.push((std::str::from_utf8(k).unwrap().to_string(), *v));
        });
        vec
    })
}



#[export_name = "canister_query http_request"]
pub fn http_request() {
    if STOP_CALLS.with(|stop_calls| { stop_calls.get() }) { trap("Maintenance. try again soon.") }
    
    let (quest,): (HttpRequest,) = arg_data::<(HttpRequest,)>(); 
    
    let file_name: String = quest.url;
    
    with(&CTS_DATA, |cts_data| {
        match cts_data.frontcode_files.get(&file_name) {
            None => {
                reply::<(HttpResponse,)>(
                    (HttpResponse {
                        status_code: 404,
                        headers: vec![],
                        body: &vec![],
                        streaming_strategy: None
                    },)
                );        
            }, 
            Some(file) => {
                reply::<(HttpResponse,)>(
                    (HttpResponse {
                        status_code: 200,
                        headers: vec![
                            make_file_certificate_header(&file_name), 
                            ("content-type".to_string(), file.content_type.clone()),
                            ("content-encoding".to_string(), file.content_encoding.clone())
                        ],
                        body: &file.content,//.to_vec(),
                        streaming_strategy: None
                    },)
                );
            }
        }
    });
    return;
}













 // ---- FOR THE TESTS --------------

#[query]
pub fn see_caller() -> Principal {
    caller()
} 







