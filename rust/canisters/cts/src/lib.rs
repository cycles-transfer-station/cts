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
    self,
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
            CBSMUpgradeCBErrorKind
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
        CTS_TRANSFER_ICP_FEE_ICP_MEMO,
        CTS_PURCHASE_CYCLES_BANK_COLLECT_PAYMENT_ICP_MEMO

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
    transfer_user_icp_ledger,
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
    cycles_market_cmcaller: Principal,
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
    users_purchase_cycles_bank: HashMap<Principal, PurchaseCyclesBankData>,
    users_burn_icp_mint_cycles: HashMap<Principal, BurnIcpMintCyclesData>,
    users_transfer_icp: HashMap<Principal, TransferIcpData>

}
impl CTSData {
    fn new() -> Self {
        Self {
            controllers: Vec::new(),
            cycles_market_id: Principal::from_slice(&[]),
            cycles_market_cmcaller: Principal::from_slice(&[]),
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
            users_purchase_cycles_bank: HashMap::new(),
            users_burn_icp_mint_cycles: HashMap::new(),
            users_transfer_icp: HashMap::new()
        }
    }
}


    

pub const NEW_CYCLES_BANK_COST_CYCLES: Cycles = 15_000_000_000_000; //15T-cycles for a new-cycles_bank. lifetime: 1-year, storage-size: 50mib/*160mib-canister-memory-allocation*/, start-with-the-ctsfuel: 5T-cycles. 
pub const NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS: u128 = 1*60*60*24*365; // 1-year.
pub const NEW_CYCLES_BANK_CTSFUEL: CTSFuel = 5_000_000_000_000; // 5T-cycles.
pub const NEW_CYCLES_BANK_STORAGE_SIZE_MiB: u128 = 50; // 50-mib
pub const NEW_CYCLES_BANK_NETWORK_MEMORY_ALLOCATION_MiB: u128 = cts_lib::tools::cb_storage_size_mib_as_cb_network_memory_allocation_mib(NEW_CYCLES_BANK_STORAGE_SIZE_MiB);
pub const NEW_CYCLES_BANK_BACKUP_CYCLES: Cycles = 1_400_000_000_000;
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

pub const MAX_USERS_PURCHASE_CYCLES_BANK: usize = 5000; // the max number of entries in the NEW_USERS-hashmap at the same-time
pub const MAX_CBS_MAPS: usize = 4; // can be 30-million at 1-gb, or 3-million at 0.1-gb,

pub const MAX_USERS_TRANSFER_ICP: usize = 2000;
pub const CTS_TRANSFER_ICP_FEE: Cycles = 30_000_000_000; // taken as the icptokens by the conversion-rate

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
    cycles_market_id: Principal,
    cycles_market_cmcaller: Principal,
} 

#[init]
fn init(cts_init: CTSInit) {
    with_mut(&CTS_DATA, |cts_data| { 
        cts_data.controllers = cts_init.controllers; 
        cts_data.cycles_market_id = cts_init.cycles_market_id;
        cts_data.cycles_market_cmcaller = cts_init.cycles_market_cmcaller;
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
    
    // ------
    
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



#[derive(CandidType, Deserialize)]
pub enum UserIsInTheMiddleOfADifferentCall {
    PurchaseCyclesBankCall{ must_call_complete: bool },
    BurnIcpMintCyclesCall{ must_call_complete: bool },
    TransferIcpCall{ must_call_complete: bool }
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
    cycles_bank_cost_cycles: Cycles,
    cts_transfer_icp_fee: Cycles,
    burn_icp_mint_cycles_fee: Cycles
    
    
    
}

#[query]
pub fn see_fees() -> Fees {
    Fees {
        cycles_bank_cost_cycles: NEW_CYCLES_BANK_COST_CYCLES,
        cts_transfer_icp_fee: CTS_TRANSFER_ICP_FEE,
        burn_icp_mint_cycles_fee: USER_BURN_ICP_MINT_CYCLES_FEE
        
        
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
    look_if_user_is_in_the_cbs_maps: bool,
    referral_cycles_bank_canister_id: Option<Principal>, // use if a referral
    create_cycles_bank_canister_block_height: Option<IcpBlockHeight>,
    cycles_bank_canister: Option<Principal>,
    cbs_map: Option<Principal>,
    cycles_bank_canister_uninstall_code: bool,
    cycles_bank_canister_install_code: bool,
    cycles_bank_canister_status_record: Option<ManagementCanisterCanisterStatusRecord>,
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
        cycles_bank_cost_icp: IcpTokens,
        user_icp_ledger_balance: IcpTokens,
        icp_ledger_transfer_fee: IcpTokens
    },
    UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
    CTSIsBusy,
    FoundCyclesBank(Principal),
    ReferralUserCyclesBankNotFound,
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

    if let Some(ref referral_user_id) = q.opt_referral_user_id {
        if *referral_user_id == user_id {
            return Err(PurchaseCyclesBankError::ReferralUserCannotBeTheCaller);
        }
    }

    let purchase_cycles_bank_data: PurchaseCyclesBankData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_purchase_cycles_bank.get(&user_id) {
            Some(purchase_cycles_bank_data) => {
                return Err(PurchaseCyclesBankError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::PurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock }));
            },
            None => {
                if get(&STOP_CALLS) { trap("Maintenance. try soon."); }
                if let Some(burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
                    return Err(PurchaseCyclesBankError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::BurnIcpMintCyclesCall{ must_call_complete: !burn_icp_mint_cycles_data.lock }));
                }
                if let Some(transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
                    return Err(PurchaseCyclesBankError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::TransferIcpCall{ must_call_complete: !transfer_icp_data.lock }));
                }
                if cts_data.users_purchase_cycles_bank.len() >= MAX_USERS_PURCHASE_CYCLES_BANK {
                    return Err(PurchaseCyclesBankError::CTSIsBusy);
                }
                let purchase_cycles_bank_data: PurchaseCyclesBankData = PurchaseCyclesBankData{
                    start_time_nanos: time() as u128,
                    lock: true,
                    purchase_cycles_bank_quest: q,
                    // the options and bools are for the memberance of the steps
                    current_xdr_icp_rate: None,
                    look_if_user_is_in_the_cbs_maps: false,
                    referral_cycles_bank_canister_id: None,
                    create_cycles_bank_canister_block_height: None,
                    cycles_bank_canister: None,
                    cbs_map: None,
                    cycles_bank_canister_uninstall_code: false,
                    cycles_bank_canister_install_code: false,
                    cycles_bank_canister_status_record: None,
                    collect_icp: false,
                    collect_cycles_cmc_icp_transfer_block_height: None,
                    collect_cycles_cmc_notify_cycles: None,
                    referral_user_referral_payment_cycles_transfer: false,
                    user_referral_payment_cycles_transfer: false
                };
                cts_data.users_purchase_cycles_bank.insert(user_id, purchase_cycles_bank_data.clone());
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
        
        let current_membership_cost_icp: IcpTokens = cycles_to_icptokens(NEW_CYCLES_BANK_COST_CYCLES, current_xdr_icp_rate); 
        
        if user_icp_ledger_balance < current_membership_cost_icp + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) {
            with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
            return Err(PurchaseCyclesBankError::UserIcpLedgerBalanceTooLow{
                cycles_bank_cost_icp: current_membership_cost_icp,
                user_icp_ledger_balance,
                icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE
            });
        }   
        
        purchase_cycles_bank_data.current_xdr_icp_rate = Some(current_xdr_icp_rate);
    }
    
    
    if purchase_cycles_bank_data.look_if_user_is_in_the_cbs_maps == false {
        // check in the list of the users-whos cycles-balance is save but without a user-canister 
        
        match find_cycles_bank_canister_of_the_specific_user(user_id).await {
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
    
    if purchase_cycles_bank_data.purchase_cycles_bank_quest.opt_referral_user_id.is_some() {
    
        if purchase_cycles_bank_data.referral_cycles_bank_canister_id.is_none() {
        
            match find_cycles_bank_canister_of_the_specific_user(purchase_cycles_bank_data.purchase_cycles_bank_quest.opt_referral_user_id.as_ref().unwrap().clone()).await {
                Ok(opt_cycles_bank_canister_id) => match opt_cycles_bank_canister_id {
                    Some(cycles_bank_canister_id) => {
                        purchase_cycles_bank_data.referral_cycles_bank_canister_id = Some(cycles_bank_canister_id);
                    },
                    None => {
                        with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
                        return Err(PurchaseCyclesBankError::ReferralUserCyclesBankNotFound);
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
        
    }
    

    if purchase_cycles_bank_data.create_cycles_bank_canister_block_height.is_none() {
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
    
        purchase_cycles_bank_data.create_cycles_bank_canister_block_height = Some(create_cycles_bank_canister_block_height);
    }


    if purchase_cycles_bank_data.cycles_bank_canister.is_none() {
    
        let cycles_bank_canister: Principal = match call::<(CmcNotifyCreateCanisterQuest,), (Result<Principal, CmcNotifyError>,)>(
            MAINNET_CYCLES_MINTING_CANISTER_ID,
            "notify_create_canister",
            (CmcNotifyCreateCanisterQuest {
                controller: id(),
                block_index: purchase_cycles_bank_data.create_cycles_bank_canister_block_height.unwrap()
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
        
        purchase_cycles_bank_data.cycles_bank_canister = Some(cycles_bank_canister);
        with_mut(&CYCLES_BANKS_CACHE, |cbs_cache| { cbs_cache.put(user_id, Some(cycles_bank_canister)); });
        purchase_cycles_bank_data.cycles_bank_canister_uninstall_code = true; // because a fresh cmc canister is empty 
    }
    
    if purchase_cycles_bank_data.cbs_map.is_none() {
        
        let cbs_map: Principal = match put_new_user_into_a_cbsm(
            user_id, 
            CBSMUserData{
                cycles_bank_canister_id: purchase_cycles_bank_data.cycles_bank_canister.as_ref().unwrap().clone(),
                cycles_bank_latest_known_module_hash: [0u8; 32],
                cycles_bank_lifetime_termination_timestamp_seconds: purchase_cycles_bank_data.start_time_nanos/1_000_000_000 + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS
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
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankUninstallCodeCallError((uninstall_code_call_error.0 as u32, uninstall_code_call_error.1))));
            }
        }
        
        purchase_cycles_bank_data.cycles_bank_canister_uninstall_code = true;
    }


    if purchase_cycles_bank_data.cycles_bank_canister_install_code == false {
    
        if with(&CTS_DATA, |cts_data| { cts_data.cycles_bank_canister_code.module().len() == 0 }) {
            purchase_cycles_bank_data.lock = false;
            write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
            return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankCodeNotFound));
        }

        match with(&CTS_DATA, |cts_data| {
            call_raw128( //::<(ManagementCanisterInstallCodeQuest,), ()>(
                MANAGEMENT_CANISTER_ID,
                "install_code",
                &encode_one(&ManagementCanisterInstallCodeQuest {
                    mode : ManagementCanisterInstallCodeMode::install,
                    canister_id : purchase_cycles_bank_data.cycles_bank_canister.unwrap(),
                    wasm_module : cts_data.cycles_bank_canister_code.module(),
                    arg : &encode_one(&CyclesBankInit{ 
                        cts_id: id(), 
                        cbsm_id: purchase_cycles_bank_data.cbs_map.unwrap(),
                        cycles_market_id: cts_data.cycles_market_id,
                        cycles_market_cmcaller: cts_data.cycles_market_cmcaller,
                        user_id: user_id,
                        storage_size_mib: NEW_CYCLES_BANK_STORAGE_SIZE_MiB,                         
                        lifetime_termination_timestamp_seconds: purchase_cycles_bank_data.start_time_nanos/1_000_000_000 + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS,
                        cycles_transferrer_canisters: with(&CTS_DATA, |cts_data| { cts_data.cycles_transferrer_canisters.clone() })
                    }).unwrap()
                }).unwrap(),
                0
            )
        }).await {
            Ok(_) => {},
            Err(put_code_call_error) => {
                purchase_cycles_bank_data.lock = false;
                write_purchase_cycles_bank_data(&user_id, purchase_cycles_bank_data);
                return Err(PurchaseCyclesBankError::MidCallError(PurchaseCyclesBankMidCallError::CyclesBankInstallCodeCallError((put_code_call_error.0 as u32, put_code_call_error.1))));
            }
        }
        
        purchase_cycles_bank_data.cycles_bank_canister_install_code = true;
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
        purchase_cycles_bank_data.cycles_bank_canister_install_code = false;
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
        memory_allocation : NEW_CYCLES_BANK_NETWORK_MEMORY_ALLOCATION_MiB as u128 * MiB as u128,
        freezing_threshold : 2592000 * 3 
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
    
    
    // hand out the referral-bonuses if there is.
    if purchase_cycles_bank_data.purchase_cycles_bank_quest.opt_referral_user_id.is_some() {
        
        if purchase_cycles_bank_data.collect_cycles_cmc_icp_transfer_block_height.is_none() {
            match ledger_topup_cycles_cmc_icp_transfer(
                cycles_to_icptokens(NEW_CYCLES_BANK_COST_CYCLES - NEW_CYCLES_BANK_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()), 
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
                purchase_cycles_bank_data.referral_cycles_bank_canister_id.as_ref().unwrap().clone(),        
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
                purchase_cycles_bank_data.cycles_bank_canister.as_ref().unwrap().clone(),
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
            match transfer_user_icp_ledger(&user_id, cycles_to_icptokens(NEW_CYCLES_BANK_COST_CYCLES - NEW_CYCLES_BANK_CREATION_CYCLES, purchase_cycles_bank_data.current_xdr_icp_rate.unwrap()), ICP_LEDGER_TRANSFER_DEFAULT_FEE, CTS_PURCHASE_CYCLES_BANK_COLLECT_PAYMENT_ICP_MEMO).await {
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
    


    with_mut(&CTS_DATA, |cts_data| { cts_data.users_purchase_cycles_bank.remove(&user_id); });
    
    Ok(PurchaseCyclesBankSuccess {
        cycles_bank_canister_id: purchase_cycles_bank_data.cycles_bank_canister.unwrap()
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
        match cts_data.users_purchase_cycles_bank.get(&user_id) {
            None => Ok(()),
            Some(purchase_cycles_bank_data) => { 
                return Err(FindCyclesBankError::UserIsInTheMiddleOfAPurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock });    
            }
        }
    })?;
    
    find_cycles_bank_canister_of_the_specific_user(user_id).await.map_err(
        |find_user_in_the_cbsms_error| { 
            FindCyclesBankError::FindUserInTheCBSMapsError(find_user_in_the_cbsms_error) 
        }
    )

}



async fn find_cycles_bank_canister_of_the_specific_user(user_id: Principal) -> Result<Option<Principal>, FindUserInTheCBSMapsError> {
    if let Some(opt_cycles_bank_canister_id) = with_mut(&CYCLES_BANKS_CACHE, |uc_cache| { uc_cache.check(user_id) }) {
        return Ok(opt_cycles_bank_canister_id);
    } 
    find_user_in_the_cbs_maps(user_id).await.map(
        |opt_umc_user_data_and_umc_id| {
            let opt_cycles_bank_canister_id: Option<Principal> = opt_umc_user_data_and_umc_id.map(|(umc_user_data, _umc_id)| { umc_user_data.cycles_bank_canister_id });
            with_mut(&CYCLES_BANKS_CACHE, |uc_cache| {
                uc_cache.put(user_id, opt_cycles_bank_canister_id);
            });    
            opt_cycles_bank_canister_id
        }
    )   
} 






// ----------------------------------------------------------------------------------------------------




// options are for the memberance of the steps

#[derive(CandidType, Deserialize, Clone)]
pub struct BurnIcpMintCyclesData {
    start_time_nanos: u64,
    lock: bool,
    burn_icp_mint_cycles_quest: BurnIcpMintCyclesQuest, 
    burn_icp_mint_cycles_fee: Cycles,
    cycles_bank_canister_id: Option<Principal>,
    cmc_icp_transfer_block_height: Option<IcpBlockHeight>,
    cmc_cycles: Option<Cycles>,
    call_cycles_bank_canister_cycles_transfer_refund: Option<Cycles>,
    call_management_canister_posit_cycles: bool
}


#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct BurnIcpMintCyclesQuest {
    burn_icp: IcpTokens,    
}

#[derive(CandidType, Deserialize)]
pub enum BurnIcpMintCyclesError {
    UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
    MinimumBurnIcpMintCycles{minimum_burn_icp_mint_cycles: IcpTokens},
    FindUserInTheCBSMapsError(FindUserInTheCBSMapsError),
    CyclesBankNotFound,
    CTSIsBusy,
    LedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
    MidCallError(BurnIcpMintCyclesMidCallError)
}


#[derive(CandidType, Deserialize)]
pub enum BurnIcpMintCyclesMidCallError {
    LedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
    CallCyclesBankCyclesTransferCandidEncodeError(String),
    CallCyclesBankCallPerformError(u32),
    ManagementCanisterPositCyclesCallError((u32, String))
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct BurnIcpMintCyclesSuccess {
    mint_cycles_for_the_user: Cycles,
    cts_fee_taken: Cycles
}


#[update]
pub async fn burn_icp_mint_cycles(q: BurnIcpMintCyclesQuest) -> Result<BurnIcpMintCyclesSuccess, BurnIcpMintCyclesError> {

    let user_id: Principal = caller(); 

    if q.burn_icp < MINIMUM_USER_BURN_ICP_MINT_CYCLES {
        return Err(BurnIcpMintCyclesError::MinimumBurnIcpMintCycles{
            minimum_burn_icp_mint_cycles: MINIMUM_USER_BURN_ICP_MINT_CYCLES
        });
    }
    
    let burn_icp_mint_cycles_data: BurnIcpMintCyclesData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_burn_icp_mint_cycles.get(&user_id) {
            Some(burn_icp_mint_cycles_data) => {
                return Err(BurnIcpMintCyclesError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::BurnIcpMintCyclesCall{ must_call_complete: !burn_icp_mint_cycles_data.lock }));
            },
            None => {
                if get(&STOP_CALLS) { trap("Maintenance. try soon."); }
                if let Some(purchase_cycles_bank_data) = cts_data.users_purchase_cycles_bank.get(&user_id) {
                    return Err(BurnIcpMintCyclesError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::PurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock }));
                }
                if let Some(transfer_icp_data) = cts_data.users_transfer_icp.get(&user_id) {
                    return Err(BurnIcpMintCyclesError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::TransferIcpCall{ must_call_complete: !transfer_icp_data.lock }));
                }
                if cts_data.users_burn_icp_mint_cycles.len() >= MAX_USERS_BURN_ICP_MINT_CYCLES {
                    return Err(BurnIcpMintCyclesError::CTSIsBusy);
                }
                let burn_icp_mint_cycles_data: BurnIcpMintCyclesData = BurnIcpMintCyclesData{
                    start_time_nanos: time(),
                    lock: true,
                    burn_icp_mint_cycles_quest: q, 
                    burn_icp_mint_cycles_fee: USER_BURN_ICP_MINT_CYCLES_FEE,
                    cycles_bank_canister_id: None,
                    cmc_icp_transfer_block_height: None,
                    cmc_cycles: None,
                    call_cycles_bank_canister_cycles_transfer_refund: None,
                    call_management_canister_posit_cycles: false
                };
                cts_data.users_burn_icp_mint_cycles.insert(user_id, burn_icp_mint_cycles_data.clone());
                Ok(burn_icp_mint_cycles_data)
            }
        }
    })?;

    burn_icp_mint_cycles_(user_id, burn_icp_mint_cycles_data).await
}


#[derive(CandidType, Deserialize)]
pub enum CompleteBurnIcpMintCyclesError{
    UserIsNotInTheMiddleOfABurnIcpMintCyclesCall,
    BurnIcpMintCyclesError(BurnIcpMintCyclesError)
}

#[update]
pub async fn complete_burn_icp_mint_cycles() -> Result<BurnIcpMintCyclesSuccess, CompleteBurnIcpMintCyclesError> {

    let user_id: Principal = caller(); 
    
    complete_burn_icp_mint_cycles_(user_id).await

}


async fn complete_burn_icp_mint_cycles_(user_id: Principal) -> Result<BurnIcpMintCyclesSuccess, CompleteBurnIcpMintCyclesError> {
    
    let burn_icp_mint_cycles_data: BurnIcpMintCyclesData = with_mut(&CTS_DATA, |cts_data| {
        match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
            Some(burn_icp_mint_cycles_data) => {
                if burn_icp_mint_cycles_data.lock == true {
                    return Err(CompleteBurnIcpMintCyclesError::BurnIcpMintCyclesError(BurnIcpMintCyclesError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::BurnIcpMintCyclesCall{ must_call_complete: false })));
                }
                burn_icp_mint_cycles_data.lock = true;
                Ok(burn_icp_mint_cycles_data.clone())
            },
            None => {
                return Err(CompleteBurnIcpMintCyclesError::UserIsNotInTheMiddleOfABurnIcpMintCyclesCall);
            }
        }
    })?;

    burn_icp_mint_cycles_(user_id, burn_icp_mint_cycles_data).await
        .map_err(|burn_icp_mint_cycles_error| { 
            CompleteBurnIcpMintCyclesError::BurnIcpMintCyclesError(burn_icp_mint_cycles_error) 
        })
        
}



async fn burn_icp_mint_cycles_(user_id: Principal, mut burn_icp_mint_cycles_data: BurnIcpMintCyclesData) -> Result<BurnIcpMintCyclesSuccess, BurnIcpMintCyclesError> {
    
    if burn_icp_mint_cycles_data.cycles_bank_canister_id.is_none() {
        
        let cycles_bank_canister_id: Principal = match find_cycles_bank_canister_of_the_specific_user(user_id).await {
            Ok(opt_cycles_bank_canister_id) => match opt_cycles_bank_canister_id {
                None => {
                    with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
                    return Err(BurnIcpMintCyclesError::CyclesBankNotFound);
                },
                Some(cycles_bank_canister_id) => cycles_bank_canister_id
            },
            Err(find_user_in_the_cbsms_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
                return Err(BurnIcpMintCyclesError::FindUserInTheCBSMapsError(find_user_in_the_cbsms_error));
            }
        };
        
        burn_icp_mint_cycles_data.cycles_bank_canister_id = Some(cycles_bank_canister_id);
    }     
    
    
    // this is after the put into the state bc if this is success the block height must be save in the state
    if burn_icp_mint_cycles_data.cmc_icp_transfer_block_height.is_none() {
        match ledger_topup_cycles_cmc_icp_transfer(burn_icp_mint_cycles_data.burn_icp_mint_cycles_quest.burn_icp, Some(principal_icp_subaccount(&user_id)), id()).await {
            Ok(block_height) => { burn_icp_mint_cycles_data.cmc_icp_transfer_block_height = Some(block_height); },
            Err(ledger_topup_cycles_cmc_icp_transfer_error) => {
                with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
                return Err(BurnIcpMintCyclesError::LedgerTopupCyclesCmcIcpTransferError(ledger_topup_cycles_cmc_icp_transfer_error));
            }
        }
    }
    
    if burn_icp_mint_cycles_data.cmc_cycles.is_none() {
        match ledger_topup_cycles_cmc_notify(burn_icp_mint_cycles_data.cmc_icp_transfer_block_height.unwrap(), id()).await {
            Ok(cmc_cycles) => { burn_icp_mint_cycles_data.cmc_cycles = Some(cmc_cycles); },
            Err(ledger_topup_cycles_cmc_notify_error) => {
                burn_icp_mint_cycles_data.lock = false;
                with_mut(&CTS_DATA, |cts_data| {
                    match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                        Some(data) => { *data = burn_icp_mint_cycles_data; },
                        None => {}
                    }
                });
                return Err(BurnIcpMintCyclesError::MidCallError(BurnIcpMintCyclesMidCallError::LedgerTopupCyclesCmcNotifyError(ledger_topup_cycles_cmc_notify_error)));
            }
        }
    }
    
    let cycles_for_the_cycles_bank_canister: Cycles = burn_icp_mint_cycles_data.cmc_cycles.unwrap().checked_sub(burn_icp_mint_cycles_data.burn_icp_mint_cycles_fee).unwrap_or(burn_icp_mint_cycles_data.cmc_cycles.unwrap());
    if burn_icp_mint_cycles_data.call_cycles_bank_canister_cycles_transfer_refund.is_none() {
        let mut cycles_transfer_call_future = call_raw128(
            burn_icp_mint_cycles_data.cycles_bank_canister_id.unwrap(),
            "cycles_transfer",
            &match encode_one(CyclesTransfer{
                memo: CyclesTransferMemo::Blob(b"CTS-BURN-ICP-MINT-CYCLES".to_vec())
            }) {
                Ok(b)=>b, 
                Err(candid_error)=>{
                    burn_icp_mint_cycles_data.lock = false;
                    with_mut(&CTS_DATA, |cts_data| {
                        match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                            Some(data) => { *data = burn_icp_mint_cycles_data; },
                            None => {}
                        }
                    });
                    return Err(BurnIcpMintCyclesError::MidCallError(BurnIcpMintCyclesMidCallError::CallCyclesBankCyclesTransferCandidEncodeError(format!("{}", candid_error))));          
                } 
            },
            cycles_for_the_cycles_bank_canister
        );
        
        if let Poll::Ready(call_result_with_an_error) = futures::poll!(&mut cycles_transfer_call_future) {
            let call_error: (RejectionCode, String) = call_result_with_an_error.unwrap_err();
            burn_icp_mint_cycles_data.lock = false;
            with_mut(&CTS_DATA, |cts_data| {
                match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                    Some(data) => { *data = burn_icp_mint_cycles_data; },
                    None => {}
                }
            });
            return Err(BurnIcpMintCyclesError::MidCallError(BurnIcpMintCyclesMidCallError::CallCyclesBankCallPerformError(call_error.0 as u32)));    
        }
        
        cycles_transfer_call_future.await; 
        burn_icp_mint_cycles_data.call_cycles_bank_canister_cycles_transfer_refund = Some(msg_cycles_refunded128());
    }
    
    if burn_icp_mint_cycles_data.call_cycles_bank_canister_cycles_transfer_refund.unwrap() != 0 
    && burn_icp_mint_cycles_data.call_management_canister_posit_cycles == false {
        match call_with_payment128::<(CanisterIdRecord,), ()>(
            MANAGEMENT_CANISTER_ID,
            "deposit_cycles",
            (CanisterIdRecord{
                canister_id: burn_icp_mint_cycles_data.cycles_bank_canister_id.unwrap()
            },),
            burn_icp_mint_cycles_data.call_cycles_bank_canister_cycles_transfer_refund.unwrap()
        ).await {
            Ok(_) => {
                burn_icp_mint_cycles_data.call_management_canister_posit_cycles = true;
            },
            Err(call_error) => {
                burn_icp_mint_cycles_data.lock = false;
                with_mut(&CTS_DATA, |cts_data| {
                    match cts_data.users_burn_icp_mint_cycles.get_mut(&user_id) {
                        Some(data) => { *data = burn_icp_mint_cycles_data; },
                        None => {}
                    }
                });
                return Err(BurnIcpMintCyclesError::MidCallError(BurnIcpMintCyclesMidCallError::ManagementCanisterPositCyclesCallError((call_error.0 as u32, call_error.1))));    
            }
        }
    
    }
    
    with_mut(&CTS_DATA, |cts_data| { cts_data.users_burn_icp_mint_cycles.remove(&user_id); });
    Ok(BurnIcpMintCyclesSuccess{
        mint_cycles_for_the_user: cycles_for_the_cycles_bank_canister,
        cts_fee_taken: match burn_icp_mint_cycles_data.cmc_cycles.unwrap().checked_sub(burn_icp_mint_cycles_data.burn_icp_mint_cycles_fee) {
            Some(_) => burn_icp_mint_cycles_data.burn_icp_mint_cycles_fee,
            None => 0
        }
    })
    
}



// ---------------------------------------

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
        match cts_data.users_transfer_icp.get(&user_id) {
            Some(transfer_icp_data) => {
                return Err(TransferIcpError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::TransferIcpCall{ must_call_complete: !transfer_icp_data.lock }));
            },
            None => {
                if localkey::cell::get(&STOP_CALLS) { trap("maintenance, try soon.") }
                if let Some(purchase_cycles_bank_data) = cts_data.users_purchase_cycles_bank.get(&user_id) {
                    return Err(TransferIcpError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::PurchaseCyclesBankCall{ must_call_complete: !purchase_cycles_bank_data.lock }));
                }
                if let Some(burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
                    return Err(TransferIcpError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::BurnIcpMintCyclesCall{ must_call_complete: !burn_icp_mint_cycles_data.lock }));   
                }
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
            }
        }
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
#[export_name = "canister_query controller_see_cbsms"]
pub fn controller_see_cbsms() {
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




// ----- CYCLES_BANKS-METHODS --------------------------


#[update]
pub fn controller_put_cycles_bank_canister_code(canister_code: CanisterCode) -> () {
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
        trap("CYCLES_BANK_CODE.module().len() is 0.")
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










// ----- PURCHASE_CYCLES_BANK-METHODS --------------------------

#[export_name = "canister_query controller_see_users_purchase_cycles_bank"]
pub fn controller_see_users_purchase_cycles_bank() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &PurchaseCyclesBankData)>,)>((cts_data.users_purchase_cycles_bank.iter().collect::<Vec<(&Principal, &PurchaseCyclesBankData)>>(),));
    });
}

// put new user data
#[update]
pub fn controller_put_purchase_cycles_bank_data(new_user_id: Principal, put_data: PurchaseCyclesBankData, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
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
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
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
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }

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




// ------ BurnIcpMintCycles-METHODS -----------------


#[export_name = "canister_query controller_see_users_burn_icp_mint_cycles"]
pub fn controller_see_users_burn_icp_mint_cycles() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &BurnIcpMintCyclesData)>,)>((cts_data.users_burn_icp_mint_cycles.iter().collect::<Vec<(&Principal, &BurnIcpMintCyclesData)>>(),));
    });
}

#[update]
pub fn controller_put_burn_icp_mint_cycles_data(user_id: Principal, put_data: BurnIcpMintCyclesData, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
            if burn_icp_mint_cycles_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_burn_icp_mint_cycles. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_burn_icp_mint_cycles.insert(user_id, put_data);
    });

}

#[update]
pub fn controller_remove_burn_icp_mint_cycles(user_id: Principal, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
    with_mut(&CTS_DATA, |cts_data| {
        if let Some(burn_icp_mint_cycles_data) = cts_data.users_burn_icp_mint_cycles.get(&user_id) {
            if burn_icp_mint_cycles_data.lock == true {
                if override_lock == false {
                    trap("user is with the lock == true in the users_burn_icp_mint_cycles. set the override_lock flag if want override.")
                }
            }
        }
        cts_data.users_burn_icp_mint_cycles.remove(&user_id);
    });
}


#[update]
pub async fn controller_complete_users_burn_icp_mint_cycles(opt_complete_users_ids: Option<Vec<Principal>>) -> Vec<(Principal, Result<BurnIcpMintCyclesSuccess, CompleteBurnIcpMintCyclesError>)> {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }

    let complete_users_ids: Vec<Principal> = match opt_complete_users_ids {
        Some(complete_users_ids) => complete_users_ids,
        None => {
            with(&CTS_DATA, |cts_data| { 
                cts_data.users_burn_icp_mint_cycles.iter()
                .filter(|&(_user_id, burn_icp_mint_cycles_data): &(&Principal, &BurnIcpMintCyclesData)| {
                    burn_icp_mint_cycles_data.lock == false
                })
                .map(|(user_id, _burn_icp_mint_cycles_data): (&Principal, &BurnIcpMintCyclesData)| {
                    user_id.clone()
                })
                .collect::<Vec<Principal>>()
            })
        }
    };
    
    let rs: Vec<Result<BurnIcpMintCyclesSuccess, CompleteBurnIcpMintCyclesError>> = futures::future::join_all(
        complete_users_ids.iter().map(
            |complete_user_id: &Principal| {
                complete_burn_icp_mint_cycles_(complete_user_id.clone())
            }
        ).collect::<Vec<_>>()
    ).await;
    
    complete_users_ids.into_iter().zip(rs.into_iter()).collect::<Vec<(Principal, Result<BurnIcpMintCyclesSuccess, CompleteBurnIcpMintCyclesError>)>>()

}



// ------ UsersTransferIcp-METHODS -----------------


#[export_name = "canister_query controller_see_users_transfer_icp"]
pub fn controller_see_users_transfer_icp() {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    with(&CTS_DATA, |cts_data| {
        ic_cdk::api::call::reply::<(Vec<(&Principal, &TransferIcpData)>,)>((cts_data.users_transfer_icp.iter().collect::<Vec<(&Principal, &TransferIcpData)>>(),));
    });
}

#[update]
pub fn controller_put_transfer_icp_data(user_id: Principal, put_data: TransferIcpData, override_lock: bool) {
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
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
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }
    
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
    if with(&CTS_DATA, |cts_data| { cts_data.controllers.contains(&caller()) }) == false {
        trap("Caller must be a controller for this method.")
    }

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
    cbsm_code_hash: Option<[u8; 32]>,
    cycles_bank_canister_code_hash: Option<[u8; 32]>,
    cycles_transferrer_canister_code_hash: Option<[u8; 32]>,
    cbsms_count: u64,
    cycles_transferrer_canisters_count: u64,
    latest_known_cmc_rate: IcpXdrConversionRate,
    users_purchase_cycles_bank_count: u64,
    users_burn_icp_mint_cycles_count: u64,
    users_transfer_icp_count: u64,
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
            cbsm_code_hash: if cts_data.cbs_map_canister_code.module().len() != 0 { Some(cts_data.cbs_map_canister_code.module_hash().clone()) } else { None },
            cycles_bank_canister_code_hash: if cts_data.cycles_bank_canister_code.module().len() != 0 { Some(cts_data.cycles_bank_canister_code.module_hash().clone()) } else { None },
            cycles_transferrer_canister_code_hash: if cts_data.cycles_transferrer_canister_code.module().len() != 0 { Some(cts_data.cycles_transferrer_canister_code.module_hash().clone()) } else { None },
            cbsms_count: cts_data.cbs_maps.len() as u64,
            cycles_transferrer_canisters_count: cts_data.cycles_transferrer_canisters.len() as u64,
            latest_known_cmc_rate: LATEST_KNOWN_CMC_RATE.with(|cr| cr.get()),
            users_purchase_cycles_bank_count: cts_data.users_purchase_cycles_bank.len() as u64,
            users_burn_icp_mint_cycles_count: cts_data.users_burn_icp_mint_cycles.len() as u64,
            users_transfer_icp_count: cts_data.users_transfer_icp.len() as u64
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







