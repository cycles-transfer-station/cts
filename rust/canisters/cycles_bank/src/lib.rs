use std::{
    cell::{RefCell,Cell},
    collections::{HashMap, HashSet}
};
use cts_lib::{
    self,
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            canister_balance128,
            call::{
                msg_cycles_accept128,
                msg_cycles_available128,
                msg_cycles_refunded128,
                RejectionCode,
                reject,
                reply,
                CallResult,
                arg_data,
                arg_data_raw_size,
                call,
                call_with_payment128,
                call_raw128
            },
            stable::{
                stable64_read,
                stable64_write
            }
        },
        export::{
            Principal,
            candid::{
                CandidType,
                Deserialize,
                Nat,
                utils::{
                    encode_one,
                    decode_one
                }
            },
        },
        update, 
        query, 
        init, 
        pre_upgrade, 
        post_upgrade
    },
    management_canister,
    ic_ledger_types::{
        MAINNET_LEDGER_CANISTER_ID
    },
    types::{
        Cycles,
        CTSFuel,
        CyclesTransfer,
        CyclesTransferMemo,
        cycles_transferrer,
        cycles_bank::{
            CyclesBankInit,
            LengthenLifetimeQuest
        },
        cycles_market::{
            icrc1token_trade_contract as cm_icrc1token_trade_contract,
            cm_main::Icrc1TokenTradeContract,
        }
    },
    consts::{
        MiB,
        GiB,
        NANOS_IN_A_SECOND,
        SECONDS_IN_A_DAY,
        WASM_PAGE_SIZE_BYTES,
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES,
        MANAGEMENT_CANISTER_ID,
    },
    tools::{
        time_nanos,
        time_seconds,
        localkey::{
            self,
            refcell::{
                with, 
                with_mut,
            }
        },
        tokens_transform_cycles,
    },
    icrc::{Tokens,IcrcId, BlockId},
    global_allocator_counter::get_allocated_bytes_count,
    stable_memory_tools::{self, MemoryId},
};

use serde::Serialize;

// -------------------------------------------------------------------------


#[derive(CandidType, Serialize, Deserialize)]
struct CyclesTransferIn {
    id: u128,
    by_the_canister: Principal,
    cycles: Cycles,
    cycles_transfer_memo: CyclesTransferMemo,       // save max 32-bytes of the memo, of a Blob or of a Text
    timestamp_nanos: u128
}

#[derive(CandidType, Serialize, Deserialize)]
struct CyclesTransferOut {
    id: u128,
    for_the_canister: Principal,
    cycles_sent: Cycles,
    cycles_refunded: Option<Cycles>,   // option cause this field is only filled in the callback and that might not come back because of the callee holding-back the callback cross-upgrades. // if/when a user deletes some CyclesTransferPurchaseLogs, let the user set a special flag to delete the still-not-come-back-user_transfer_cycles by default unset.
    cycles_transfer_memo: CyclesTransferMemo,                           // save max 32-bytes of the memo, of a Blob or of a Text
    timestamp_nanos: u128, // time sent
    opt_cycles_transfer_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)>, // None means the cycles_transfer-call replied. // save max 20-bytes of the string
    fee_paid: u64 // cycles_transferrer_fee
}

// --------

#[derive(CandidType, Serialize, Deserialize)]
struct CMCyclesPosition{
    id: cm_icrc1token_trade_contract::PositionId,   
    create_cycles_position_quest: cm_icrc1token_trade_contract::CreateCyclesPositionQuest,
    create_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMTokenPosition{
    id: cm_icrc1token_trade_contract::PositionId,   
    create_token_position_quest: cm_icrc1token_trade_contract::CreateTokenPositionQuest,
    create_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMCyclesPositionPurchase{
    cycles_position_id: cm_icrc1token_trade_contract::PositionId,
    cycles_position_cycles_per_token_rate: cm_icrc1token_trade_contract::CyclesPerToken,
    cycles_position_positor: Principal,
    id: cm_icrc1token_trade_contract::PurchaseId,
    cycles: Cycles,
    purchase_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMTokenPositionPurchase{
    token_position_id: cm_icrc1token_trade_contract::PositionId,
    token_position_cycles_per_token_rate: cm_icrc1token_trade_contract::CyclesPerToken,
    token_position_positor: Principal,
    id: cm_icrc1token_trade_contract::PurchaseId,
    tokens: Tokens,
    purchase_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMTokenTransferOut{
    tokens: Tokens,
    token_ledger_transfer_fee: Tokens,
    to: IcrcId,
    block_height: Nat,
    timestamp_nanos: u128,
    transfer_token_balance_fee: u64
}

#[derive(Serialize, Deserialize)]
struct CMCallsOut {
    cm_cycles_positions: Vec<CMCyclesPosition>,
    cm_token_positions: Vec<CMTokenPosition>,
    cm_cycles_positions_purchases: Vec<CMCyclesPositionPurchase>,
    cm_token_positions_purchases: Vec<CMTokenPositionPurchase>,    
    cm_token_transfers_out: Vec<CMTokenTransferOut>,
}
impl CMCallsOut {
    fn new() -> Self {
        Self {
            cm_cycles_positions: Vec::new(),
            cm_token_positions: Vec::new(),
            cm_cycles_positions_purchases: Vec::new(),
            cm_token_positions_purchases: Vec::new(),    
            cm_token_transfers_out: Vec::new(),
        }
    }
}


#[derive(CandidType, Serialize, Deserialize)]
struct CMMessageCyclesPositionPurchasePositorLog{
    timestamp_nanos: u128,
    cm_message_cycles_position_purchase_positor_quest: cm_icrc1token_trade_contract::CMCyclesPositionPurchasePositorMessageQuest 
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMMessageCyclesPositionPurchasePurchaserLog{
    timestamp_nanos: u128,
    cycles_purchase: Cycles,
    cm_message_cycles_position_purchase_purchaser_quest: cm_icrc1token_trade_contract::CMCyclesPositionPurchasePurchaserMessageQuest
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMMessageTokenPositionPurchasePositorLog{
    timestamp_nanos: u128,
    cycles_payment: Cycles,
    cm_message_token_position_purchase_positor_quest: cm_icrc1token_trade_contract::CMTokenPositionPurchasePositorMessageQuest
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMMessageTokenPositionPurchasePurchaserLog{
    timestamp_nanos: u128,
    cm_message_token_position_purchase_purchaser_quest: cm_icrc1token_trade_contract::CMTokenPositionPurchasePurchaserMessageQuest
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMMessageVoidCyclesPositionPositorLog{
    timestamp_nanos: u128,
    void_cycles: Cycles,
    cm_message_void_cycles_position_positor_quest: cm_icrc1token_trade_contract::CMVoidCyclesPositionPositorMessageQuest
}

#[derive(CandidType, Serialize, Deserialize)]
struct CMMessageVoidTokenPositionPositorLog{
    timestamp_nanos: u128,
    cm_message_void_token_position_positor_quest: cm_icrc1token_trade_contract::CMVoidTokenPositionPositorMessageQuest
}

#[derive(Serialize, Deserialize)]
struct CMMessageLogs{
    cm_message_cycles_position_purchase_positor_logs: Vec<CMMessageCyclesPositionPurchasePositorLog>,
    cm_message_cycles_position_purchase_purchaser_logs: Vec<CMMessageCyclesPositionPurchasePurchaserLog>,
    cm_message_token_position_purchase_positor_logs: Vec<CMMessageTokenPositionPurchasePositorLog>,
    cm_message_token_position_purchase_purchaser_logs: Vec<CMMessageTokenPositionPurchasePurchaserLog>,
    cm_message_void_cycles_position_positor_logs: Vec<CMMessageVoidCyclesPositionPositorLog>,
    cm_message_void_token_position_positor_logs: Vec<CMMessageVoidTokenPositionPositorLog>,    
}
impl CMMessageLogs {
    fn new() -> Self {
        Self{
            cm_message_cycles_position_purchase_positor_logs: Vec::new(),
            cm_message_cycles_position_purchase_purchaser_logs: Vec::new(),
            cm_message_token_position_purchase_positor_logs: Vec::new(),
            cm_message_token_position_purchase_purchaser_logs: Vec::new(),
            cm_message_void_cycles_position_positor_logs: Vec::new(),
            cm_message_void_token_position_positor_logs: Vec::new(),                
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CMTradeContractLogs {
    cm_calls_out: CMCallsOut,
    cm_message_logs: CMMessageLogs,
}
impl CMTradeContractLogs {
    fn new() -> Self {
        Self {
            cm_calls_out: CMCallsOut::new(),
            cm_message_logs: CMMessageLogs::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct UserData {
    cycles_balance: Cycles,
    cycles_transfers_in: Vec<CyclesTransferIn>,
    cycles_transfers_out: Vec<CyclesTransferOut>,
    cm_trade_contracts: HashMap<Icrc1TokenTradeContract, CMTradeContractLogs>,
}

impl UserData {
    fn new() -> Self {
        Self {
            cycles_balance: 0u128,
            cycles_transfers_in: Vec::new(),
            cycles_transfers_out: Vec::new(),
            cm_trade_contracts: HashMap::new(),
        }
    }
}



#[derive(Serialize, Deserialize)]
struct CBData {
    user_canister_creation_timestamp_nanos: u128,
    cts_id: Principal,
    cbsm_id: Principal,
    user_id: Principal,
    storage_size_mib: u128,
    lifetime_termination_timestamp_seconds: u128,
    cycles_transferrer_canisters: Vec<Principal>,
    user_data: UserData,
    cycles_transfers_id_counter: u128,
}

impl CBData {
    fn new() -> Self {
        Self {
            user_canister_creation_timestamp_nanos: 0,
            cts_id: Principal::from_slice(&[]),
            cbsm_id: Principal::from_slice(&[]),
            user_id: Principal::from_slice(&[]),
            storage_size_mib: 0,       // memory-allocation/2 // is with the set in the canister_init // in the mib // starting at a 50mib-storage with a 1-year-user_canister_lifetime with a 5T-cycles-ctsfuel-balance at a cost: 10T-CYCLES   // this value is half of the user-canister-memory_allocation. for the upgrades.  
            lifetime_termination_timestamp_seconds: 0,
            cycles_transferrer_canisters: Vec::new(),
            user_data: UserData::new(),
            cycles_transfers_id_counter: 0,        
        }
    }
}

// ------ old cb data -----------

use cts_lib::{
    types::{
        XdrPerMyriadPerIcp,
    },
    ic_ledger_types::{
        IcpTokens,
        IcpId,
        IcpBlockHeight,
    }
};
use cts_lib::types::cycles_market::icrc1token_trade_contract::{PositionId, PurchaseId};

#[derive(CandidType, Deserialize)]
struct OldCMCyclesPosition{
    id: u128,   
    cycles: Cycles,
    minimum_purchase: Cycles,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    create_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct OldCMIcpPosition{
    id: u128,   
    icp: IcpTokens,
    minimum_purchase: IcpTokens,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    create_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct OldCMCyclesPositionPurchase{
    cycles_position_id: u128,
    cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    cycles_position_positor: Principal,
    id: u128,
    cycles: Cycles,
    purchase_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct OldCMIcpPositionPurchase{
    icp_position_id: u128,
    icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    icp_position_positor: Principal,
    id: u128,
    icp: IcpTokens,
    purchase_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct OldCMIcpTransferOut{
    icp: IcpTokens,
    icp_fee: IcpTokens,
    to: IcpId,
    block_height: u128,
    timestamp_nanos: u128,
    transfer_icp_balance_fee: u64
}



#[derive(CandidType, Deserialize)]
struct OldCMCallsOut {
    cm_cycles_positions: Vec<OldCMCyclesPosition>,
    cm_icp_positions: Vec<OldCMIcpPosition>,
    cm_cycles_positions_purchases: Vec<OldCMCyclesPositionPurchase>,
    cm_icp_positions_purchases: Vec<OldCMIcpPositionPurchase>,    
    cm_icp_transfers_out: Vec<OldCMIcpTransferOut>,
}


#[derive(CandidType, Deserialize)]
struct OldCMMessageCyclesPositionPurchasePositorLog{
    timestamp_nanos: u128,
    cm_message_cycles_position_purchase_positor_quest: OldCMCyclesPositionPurchasePositorMessageQuest 
}

#[derive(CandidType, Deserialize)]
struct OldCMMessageCyclesPositionPurchasePurchaserLog{
    timestamp_nanos: u128,
    cycles_purchase: Cycles,
    cm_message_cycles_position_purchase_purchaser_quest: OldCMCyclesPositionPurchasePurchaserMessageQuest
}

#[derive(CandidType, Deserialize)]
struct OldCMMessageIcpPositionPurchasePositorLog{
    timestamp_nanos: u128,
    cycles_payment: Cycles,
    cm_message_icp_position_purchase_positor_quest: OldCMIcpPositionPurchasePositorMessageQuest
}

#[derive(CandidType, Deserialize)]
struct OldCMMessageIcpPositionPurchasePurchaserLog{
    timestamp_nanos: u128,
    cm_message_icp_position_purchase_purchaser_quest: OldCMIcpPositionPurchasePurchaserMessageQuest
}

#[derive(CandidType, Deserialize)]
struct OldCMMessageVoidCyclesPositionPositorLog{
    timestamp_nanos: u128,
    void_cycles: Cycles,
    cm_message_void_cycles_position_positor_quest: OldCMVoidCyclesPositionPositorMessageQuest
}

#[derive(CandidType, Deserialize)]
struct OldCMMessageVoidIcpPositionPositorLog{
    timestamp_nanos: u128,
    cm_message_void_icp_position_positor_quest: OldCMVoidIcpPositionPositorMessageQuest
}

#[derive(CandidType, Deserialize)]
pub struct OldCMVoidCyclesPositionPositorMessageQuest {
    pub position_id: PositionId,
    // cycles in the call
    pub timestamp_nanos: u128
}

#[derive(CandidType, Deserialize)]
pub struct OldCMVoidIcpPositionPositorMessageQuest {
    pub position_id: PositionId,
    pub void_icp: IcpTokens,
    pub timestamp_nanos: u128
}

#[derive(CandidType, Deserialize)]
pub struct OldCMCyclesPositionPurchasePositorMessageQuest {
    pub cycles_position_id: PositionId,
    pub purchase_id: PurchaseId,
    pub purchaser: Principal,
    pub purchase_timestamp_nanos: u128,
    pub cycles_purchase: Cycles,
    pub cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    pub icp_payment: IcpTokens,
    pub icp_transfer_block_height: IcpBlockHeight,
    pub icp_transfer_timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
pub struct OldCMCyclesPositionPurchasePurchaserMessageQuest {
    pub cycles_position_id: PositionId,
    pub cycles_position_positor: Principal,
    pub cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    pub purchase_id: PurchaseId,
    pub purchase_timestamp_nanos: u128,
    // cycles in the call
    pub icp_payment: IcpTokens,
}

#[derive(CandidType, Deserialize)]
pub struct OldCMIcpPositionPurchasePositorMessageQuest {
    pub icp_position_id: PositionId,
    pub icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    pub purchaser: Principal,
    pub purchase_id: PurchaseId,
    pub icp_purchase: IcpTokens,
    pub purchase_timestamp_nanos: u128,
    // cycles in the call
}

#[derive(CandidType, Deserialize)]
pub struct OldCMIcpPositionPurchasePurchaserMessageQuest {
    pub icp_position_id: PositionId,
    pub purchase_id: PurchaseId, 
    pub positor: Principal,
    pub purchase_timestamp_nanos: u128,
    pub cycles_payment: Cycles,
    pub icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    pub icp_purchase: IcpTokens,
    pub icp_transfer_block_height: IcpBlockHeight,
    pub icp_transfer_timestamp_nanos: u128,
}




#[derive(CandidType, Deserialize)]
struct OldCMMessageLogs{
    cm_message_cycles_position_purchase_positor_logs: Vec<OldCMMessageCyclesPositionPurchasePositorLog>,
    cm_message_cycles_position_purchase_purchaser_logs: Vec<OldCMMessageCyclesPositionPurchasePurchaserLog>,
    cm_message_icp_position_purchase_positor_logs: Vec<OldCMMessageIcpPositionPurchasePositorLog>,
    cm_message_icp_position_purchase_purchaser_logs: Vec<OldCMMessageIcpPositionPurchasePurchaserLog>,
    cm_message_void_cycles_position_positor_logs: Vec<OldCMMessageVoidCyclesPositionPositorLog>,
    cm_message_void_icp_position_positor_logs: Vec<OldCMMessageVoidIcpPositionPositorLog>,    
}

#[derive(CandidType, Deserialize)]
struct OldUserData {
    cycles_balance: Cycles,
    cycles_transfers_in: Vec<CyclesTransferIn>,
    cycles_transfers_out: Vec<CyclesTransferOut>,
    cm_calls_out: OldCMCallsOut, 
    cm_message_logs: OldCMMessageLogs,
    known_icrc1_ledgers: HashSet<Principal>
}

#[derive(CandidType, Deserialize)]
struct OldCBData {
    user_canister_creation_timestamp_nanos: u128,
    cts_id: Principal,
    cbsm_id: Principal,
    cycles_market_id: Principal,
    cycles_market_cmcaller: Principal,
    user_id: Principal,
    storage_size_mib: u128,
    lifetime_termination_timestamp_seconds: u128,
    cycles_transferrer_canisters: Vec<Principal>,
    user_data: OldUserData,
    cycles_transfers_id_counter: u128,
}

// ------------------------------


pub const CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

pub const CYCLES_MARKET_CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const CYCLES_MARKET_PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;
pub const CYCLES_MARKET_TRANSFER_TOKEN_BALANCE_FEE: Cycles = 50_000_000_000;

const USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE: usize = 32;
const MINIMUM_USER_TRANSFER_CYCLES: Cycles = 10_000_000_000;
const CYCLES_TRANSFER_IN_MINIMUM_CYCLES: Cycles = 10_000_000_000;




const MINIMUM_LENGTHEN_LIFETIME_SECONDS: u128 = SECONDS_IN_A_DAY * 30;

const MINIMUM_CYCLES_FOR_THE_CTSFUEL: Cycles = 10_000_000_000;

#[allow(non_upper_case_globals)]
const MAXIMUM_STORAGE_SIZE_MiB: u128 = 1024;

const DELETE_LOG_MINIMUM_WAIT_NANOS: u128 = NANOS_IN_A_SECOND * SECONDS_IN_A_DAY * 45;

const STABLE_MEMORY_ID_CB_DATA_SERIALIZATION: MemoryId = MemoryId::new(0);

const USER_CANISTER_BACKUP_CYCLES: Cycles = 1_000_000_000_000;

const CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE: &'static str = "ctsfuel_balance is too low";


thread_local! {
   
    static CB_DATA: RefCell<CBData> = RefCell::new(CBData::new());

    // not save in a CBData
    static MEMORY_SIZE_AT_THE_START: Cell<usize> = Cell::new(0); 
    static CYCLES_TRANSFERRER_CANISTERS_ROUND_ROBIN_COUNTER: Cell<usize> = Cell::new(0);
    static STOP_CALLS: Cell<bool> = Cell::new(false);
    static STATE_SNAPSHOT_CB_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());

}



// ---------------------------------------------------------------------------------


#[init]
fn canister_init(user_canister_init: CyclesBankInit) {
    
    stable_memory_tools::init(&CB_DATA, STABLE_MEMORY_ID_CB_DATA_SERIALIZATION);
    
    with_mut(&CB_DATA, |cb_data| {
        *cb_data = CBData{
            user_canister_creation_timestamp_nanos:                 time_nanos(),
            cts_id:                                                 user_canister_init.cts_id,
            cbsm_id:                                                user_canister_init.cbsm_id,
            user_id:                                                user_canister_init.user_id,
            storage_size_mib:                                       user_canister_init.storage_size_mib,
            lifetime_termination_timestamp_seconds:                 user_canister_init.lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters:                           user_canister_init.cycles_transferrer_canisters,
            user_data:                                              UserData::new(),
            cycles_transfers_id_counter:                            0u128    
        };
    });

    
    localkey::cell::set(&MEMORY_SIZE_AT_THE_START, core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES);
    
}




#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    
    localkey::cell::set(&MEMORY_SIZE_AT_THE_START, core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES);

    // custom stable memory read then load then overwrite old stable memory zeros then call stable_memory_tools::init
    const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;
    
    let mut uc_upgrade_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut uc_upgrade_data_candid_bytes_len_u64_be_bytes);
    let uc_upgrade_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(uc_upgrade_data_candid_bytes_len_u64_be_bytes); 
    
    let mut uc_upgrade_data_candid_bytes: Vec<u8> = vec![0; uc_upgrade_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut uc_upgrade_data_candid_bytes);
    
    let old_cb_data: OldCBData = decode_one::<OldCBData>(&uc_upgrade_data_candid_bytes).unwrap();
    let new_cb_data: CBData = CBData{
        user_canister_creation_timestamp_nanos: old_cb_data.user_canister_creation_timestamp_nanos,
        cts_id: old_cb_data.cts_id,
        cbsm_id: old_cb_data.cbsm_id,
        user_id: old_cb_data.user_id,
        storage_size_mib: old_cb_data.storage_size_mib,
        lifetime_termination_timestamp_seconds: old_cb_data.lifetime_termination_timestamp_seconds,
        cycles_transferrer_canisters: old_cb_data.cycles_transferrer_canisters,
        user_data: UserData{
            cycles_balance: old_cb_data.user_data.cycles_balance,
            cycles_transfers_in: old_cb_data.user_data.cycles_transfers_in,
            cycles_transfers_out: old_cb_data.user_data.cycles_transfers_out,
            cm_trade_contracts: [
                (
                    Icrc1TokenTradeContract{
                        icrc1_ledger_canister_id: MAINNET_LEDGER_CANISTER_ID,
                        trade_contract_canister_id: Principal::from_text("").unwrap(),
                        opt_cm_caller: Some(Principal::from_text("").unwrap())
                    },
                    CMTradeContractLogs::new()
                )
            ].into() // HashMap<Icrc1TokenTradeContract, CMTradeContractLogs>,
        },
        cycles_transfers_id_counter: old_cb_data.cycles_transfers_id_counter,
    };

    with_mut(&CB_DATA, |cb_data| {
        *cb_data = new_cb_data;
    });
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &vec![0u8; uc_upgrade_data_candid_bytes_len_u64 as usize * 2 + 8]);
    
    
    stable_memory_tools::init(&CB_DATA, STABLE_MEMORY_ID_CB_DATA_SERIALIZATION); 
    
    // change for the post_upgrade for the next upgrade
    // stable_memory_tools::post_upgrade(&CB_DATA, STABLE_MEMORY_ID_CB_DATA_SERIALIZATION, None::<fn(OldCTSData) -> CTSData>);
}

// ---------------------------

// this is onli for ingress-messages (calls that come from outside the network)
#[no_mangle]
fn canister_inspect_message() {
    use cts_lib::ic_cdk::api::call::{/*method_name, */accept_message};
    
    if caller() != user_id() {
        trap("caller must be the owner");
    }
    
    accept_message();
}



// ---------------------------------------------------------------------------------

fn cts_id() -> Principal {
    with(&CB_DATA, |cb_data| { cb_data.cts_id })
}

fn user_id() -> Principal {
    with(&CB_DATA, |cb_data| { cb_data.user_id })
}

fn new_cycles_transfer_id(id_counter: &mut u128) -> u128 {
    let id: u128 = id_counter.clone();
    *id_counter += 1;
    id
}

// round-robin on the cycles-transferrer-canisters
fn next_cycles_transferrer_canister_round_robin() -> Option<Principal> {
    with(&CB_DATA, |cb_data| { 
        let ctcs: &Vec<Principal> = &(cb_data.cycles_transferrer_canisters);
        match ctcs.len() {
            0 => None,
            1 => Some(ctcs[0]),
            l => {
                CYCLES_TRANSFERRER_CANISTERS_ROUND_ROBIN_COUNTER.with(|ctcs_rrc| { 
                    let c_i: usize = ctcs_rrc.get();                    
                    if c_i <= l-1 {
                        let ctc_id: Principal = ctcs[c_i];
                        if c_i == l-1 { ctcs_rrc.set(0); } else { ctcs_rrc.set(c_i + 1); }
                        Some(ctc_id)
                    } else {
                        ctcs_rrc.set(1); // we check before that the len of the ctcs is at least 2 in the first match                         
                        Some(ctcs[0])
                    } 
                })
            }
        } 
    })
}
    
// compute the size of a CyclesTransferIn and of a CyclesTransferOut, check the length of both vectors, and compute the current storage usage. 
fn calculate_current_storage_usage(cb_data: &CBData) -> u128 {
    (
        localkey::cell::get(&MEMORY_SIZE_AT_THE_START) 
        + 
        cb_data.user_data.cycles_transfers_in.len() * ( std::mem::size_of::<CyclesTransferIn>() + 32/*for the cycles-transfer-memo-heap-size*/ )
        + 
        cb_data.user_data.cycles_transfers_out.len() * ( std::mem::size_of::<CyclesTransferOut>() + 32/*for the cycles-transfer-memo-heap-size*/ + 20/*for the possible-call-error-string-heap-size*/ )
        +
        cb_data.user_data.cm_trade_contracts.len() * std::mem::size_of::<Icrc1TokenTradeContract>()
        +
        cb_data.user_data.cm_trade_contracts
            .values()
            .fold(0, |c, cm_trade_contract_logs| { 
                c
                +
                cm_trade_contract_logs.cm_calls_out.cm_cycles_positions.len() * std::mem::size_of::<CMCyclesPosition>()
                +
                cm_trade_contract_logs.cm_calls_out.cm_token_positions.len() * std::mem::size_of::<CMTokenPosition>()
                +
                cm_trade_contract_logs.cm_calls_out.cm_cycles_positions_purchases.len() * std::mem::size_of::<CMCyclesPositionPurchase>()
                +
                cm_trade_contract_logs.cm_calls_out.cm_token_positions_purchases.len() * std::mem::size_of::<CMTokenPositionPurchase>()
                +
                cm_trade_contract_logs.cm_calls_out.cm_token_transfers_out.len() * std::mem::size_of::<CMTokenTransferOut>()
                +
                cm_trade_contract_logs.cm_message_logs.cm_message_cycles_position_purchase_positor_logs.len() * std::mem::size_of::<CMMessageCyclesPositionPurchasePositorLog>()            
                +
                cm_trade_contract_logs.cm_message_logs.cm_message_cycles_position_purchase_purchaser_logs.len() * std::mem::size_of::<CMMessageCyclesPositionPurchasePurchaserLog>()
                +
                cm_trade_contract_logs.cm_message_logs.cm_message_token_position_purchase_positor_logs.len() * std::mem::size_of::<CMMessageTokenPositionPurchasePositorLog>()
                +
                cm_trade_contract_logs.cm_message_logs.cm_message_token_position_purchase_purchaser_logs.len() * std::mem::size_of::<CMMessageTokenPositionPurchasePurchaserLog>()
                +
                cm_trade_contract_logs.cm_message_logs.cm_message_void_cycles_position_positor_logs.len() * std::mem::size_of::<CMMessageVoidCyclesPositionPositorLog>()
                +
                cm_trade_contract_logs.cm_message_logs.cm_message_void_token_position_positor_logs.len() * std::mem::size_of::<CMMessageVoidTokenPositionPositorLog>()
            })
                        
    ) as u128
}

fn calculate_free_storage(cb_data: &CBData) -> u128 {
    ( cb_data.storage_size_mib * MiB as u128 ).saturating_sub(calculate_current_storage_usage(cb_data))
}


fn ctsfuel_balance(cb_data: &CBData) -> CTSFuel {
    canister_balance128()
    .saturating_sub(cb_data.user_data.cycles_balance)
    .saturating_sub(USER_CANISTER_BACKUP_CYCLES)
    .saturating_sub(
        (
            cb_data.lifetime_termination_timestamp_seconds.saturating_sub(time_seconds()) 
            * 
            cts_lib::tools::cb_storage_size_mib_as_cb_network_memory_allocation_mib(cb_data.storage_size_mib) * MiB as u128 // canister-memory-allocation in the mib
        )
        *
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES
        /
        GiB as u128 /*network storage charge per byte per second*/
    )
}

fn truncate_cycles_transfer_memo(mut cycles_transfer_memo: CyclesTransferMemo) -> CyclesTransferMemo {
    match cycles_transfer_memo {
        CyclesTransferMemo::Nat(_n) => {},
        CyclesTransferMemo::Int(_int) => {},
        CyclesTransferMemo::Blob(ref mut b) => {
            b.truncate(32);
            b.shrink_to_fit();
        },
         CyclesTransferMemo::Text(ref mut s) => {
            s.truncate(32);
            s.shrink_to_fit();
        }
    }
    cycles_transfer_memo
}

fn maintenance_check() {
    if localkey::cell::get(&STOP_CALLS) == true { 
        trap("Maintenance, try soon."); 
    }
}


// -------------- DOWNLOAD-LOGS-MECHANISM ------------------

#[derive(CandidType, Deserialize)]
pub struct DownloadCBLogsQuest {
    pub opt_start_before_i: Option<u64>,
    pub chunk_size: u64
}

#[derive(CandidType)]
pub struct DownloadCBLogsSponse<'a, T: 'a> {
    pub logs_len: u64,
    pub logs: Option<&'a [T]>
}

fn download_logs<T: CandidType>(q: DownloadCBLogsQuest, data: &Vec<T>/*not a slice here bc want to make sure we always pass the whole vec into this function*/) -> DownloadCBLogsSponse<T> {
    DownloadCBLogsSponse{
        logs_len: data.len() as u64,
        logs: data[..q.opt_start_before_i.map(|i| i as usize).unwrap_or(data.len())].rchunks(q.chunk_size as usize).next()
    }
}

// ---------------------------------------------


#[export_name = "canister_update cycles_transfer"]
pub fn cycles_transfer() { // (ct: CyclesTransfer) -> ()

    maintenance_check();

    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        if caller() == cts_id() {
            with_mut(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_accept128(msg_cycles_available128())); });
            reply::<()>(());
            return;            
        }
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }

    if with(&CB_DATA, |cb_data| { calculate_free_storage(cb_data) }) < std::mem::size_of::<CyclesTransferIn>() as u128 + 32 {
        if caller() == cts_id() {
            with_mut(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_accept128(msg_cycles_available128())); });
            reply::<()>(());
            return;            
        }
        trap("Canister memory is full, cannot create a log of the cycles-transfer.");
    }

    if arg_data_raw_size() > 150 {
        trap("arg_data_raw_size can be max 150 bytes");
    }

    if msg_cycles_available128() < CYCLES_TRANSFER_IN_MINIMUM_CYCLES {
        trap(&format!("minimum cycles transfer cycles: {}", CYCLES_TRANSFER_IN_MINIMUM_CYCLES));
    }
        
    let cycles_cept: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    let (ct_memo, by_the_canister): (CyclesTransferMemo, Principal) = {
        if with(&CB_DATA, |cb_data| { cb_data.cycles_transferrer_canisters.contains(&caller()) }) { 
            let (ct,): (cycles_transferrer::CyclesTransfer,) = arg_data::<(cycles_transferrer::CyclesTransfer,)>();
            (ct.memo, ct.original_caller.unwrap_or(caller()))
        } else {
            let (ct,): (CyclesTransfer,) = arg_data::<(CyclesTransfer,)>();
            (ct.memo, caller())    
        }
    };
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(cycles_cept);
        cb_data.user_data.cycles_transfers_in.push(
            CyclesTransferIn{
                id: new_cycles_transfer_id(&mut cb_data.cycles_transfers_id_counter),
                by_the_canister,
                cycles: cycles_cept,
                cycles_transfer_memo: truncate_cycles_transfer_memo(ct_memo),
                timestamp_nanos: time_nanos()
            }
        );
    });
    
    reply::<()>(());
    return;
    
}



#[query(manual_reply = true)]
pub fn download_cycles_transfers_in(q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }    
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, &cb_data.user_data.cycles_transfers_in),)); 
    });
} 

#[update(manual_reply = true)]
pub fn delete_cycles_transfers_in(delete_cycles_transfers_in_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with_mut(&CB_DATA, |cb_data| {
        for delete_cycles_transfer_in_id in delete_cycles_transfers_in_ids.into_iter() {
            match cb_data.user_data.cycles_transfers_in.binary_search_by_key(&delete_cycles_transfer_in_id, |cycles_transfer_in| { cycles_transfer_in.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cycles_transfers_in[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cycles_transfer_in id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cycles_transfer_in_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cycles_transfers_in.remove(i);
                },
                Err(_) => {
                    trap(&format!("cycles_transfer_in id: {} not found.", delete_cycles_transfer_in_id))
                }
            }
        }
    });
    
    reply::<()>(());
}






#[derive(CandidType, Deserialize, Clone)]
pub struct UserTransferCyclesQuest {
    for_the_canister: Principal,
    cycles: Cycles,
    cycles_transfer_memo: CyclesTransferMemo
}


#[derive(CandidType)]
pub enum UserTransferCyclesError {
    CTSFuelTooLow,
    MemoryIsFull,
    InvalidCyclesTransferMemoSize{max_size_bytes: u128},
    InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: Cycles },
    CyclesBalanceTooLow { cycles_balance: Cycles, cycles_transferrer_transfer_cycles_fee: Cycles },
    CyclesTransferrerTransferCyclesError(cycles_transferrer::TransferCyclesError),
    CyclesTransferrerTransferCyclesCallError((u32, String))
}

#[update]
pub async fn transfer_cycles(mut q: UserTransferCyclesQuest) -> Result<u128, UserTransferCyclesError> {

    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    maintenance_check();
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 15_000_000_000 {
        return Err(UserTransferCyclesError::CTSFuelTooLow);
    }
    
    if with(&CB_DATA, |cb_data| { calculate_free_storage(cb_data) }) < std::mem::size_of::<CyclesTransferOut>() as u128 + 32 + 40 {
        return Err(UserTransferCyclesError::MemoryIsFull);
    }
    
    if q.cycles < MINIMUM_USER_TRANSFER_CYCLES {
        return Err(UserTransferCyclesError::InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: MINIMUM_USER_TRANSFER_CYCLES });
    }
    
    if q.cycles + CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE > with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) {
        return Err(UserTransferCyclesError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), cycles_transferrer_transfer_cycles_fee: CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE });
    }
    
    // check memo size
    match q.cycles_transfer_memo {
        CyclesTransferMemo::Blob(ref mut b) => {
            if b.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u128}); 
            }
            b.shrink_to_fit();
            if b.capacity() > b.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Text(ref mut s) => {
            if s.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u128}); 
            }
            s.shrink_to_fit();
            if s.capacity() > s.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Nat(_n) => {},
        CyclesTransferMemo::Int(_int) => {} 
    }
 
    let cycles_transfer_id: u128 = with_mut(&CB_DATA, |cb_data| {
        let cycles_transfer_id: u128 = new_cycles_transfer_id(&mut cb_data.cycles_transfers_id_counter);        
        // take the user-cycles before the transfer, and refund in the callback 
        cb_data.user_data.cycles_balance -= q.cycles + CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE;
        cb_data.user_data.cycles_transfers_out.push(
            CyclesTransferOut{
                id: cycles_transfer_id,
                for_the_canister: q.for_the_canister,
                cycles_sent: q.cycles,
                cycles_refunded: None,   // None means the cycles_transfer-call-callback did not come back yet(did not give-back a reply-or-reject-sponse) 
                cycles_transfer_memo: q.cycles_transfer_memo.clone(),
                timestamp_nanos: time_nanos(), // time sent
                opt_cycles_transfer_call_error: None,
                fee_paid: CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE as u64
            }
        );
        cycles_transfer_id
    });
    
    let q_cycles: Cycles = q.cycles; // copy cause want the value to stay on the stack for the closure to run with it. after the q is move into the candid params
    let cycles_transferrer_transfer_cycles_fee: Cycles = CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE; // copy the value to stay on the stack for the closure to run with it.
    
    let cancel_user_transfer_cycles = || {
        with_mut(&CB_DATA, |cb_data| {
            cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(q_cycles + cycles_transferrer_transfer_cycles_fee);
            
            match cb_data.user_data.cycles_transfers_out.iter().rposition(
                |cycles_transfer_out: &CyclesTransferOut| { 
                    (*cycles_transfer_out).id == cycles_transfer_id
                }
            ) {
                Some(i) => { cb_data.user_data.cycles_transfers_out.remove(i); },
                None => {}
            }
        });
    };
        
    match call_with_payment128::<(cycles_transferrer::TransferCyclesQuest,), (Result<(), cycles_transferrer::TransferCyclesError>,)>(
        next_cycles_transferrer_canister_round_robin().expect("0 known cycles transferrer canisters.")/*before the first await*/,
        "transfer_cycles",
        (cycles_transferrer::TransferCyclesQuest{
            user_cycles_transfer_id: cycles_transfer_id,
            for_the_canister: q.for_the_canister,
            cycles: q.cycles,
            cycles_transfer_memo: q.cycles_transfer_memo
        },),
        q.cycles + cycles_transferrer_transfer_cycles_fee
    ).await { // it is possible that this callback will be called after the cycles_transferrer calls the cycles_transferrer_user_transfer_cycles_callback
        Ok((cycles_transferrer_transfer_cycles_sponse,)) => match cycles_transferrer_transfer_cycles_sponse {
            Ok(()) => return Ok(cycles_transfer_id), // Ok here means the cycles-transfer call will either be delivered, returned because the destination canister does not exist or returned because of an out of cycles condition.
            Err(cycles_transferrer_transfer_cycles_error) => {
                cancel_user_transfer_cycles();
                return Err(UserTransferCyclesError::CyclesTransferrerTransferCyclesError(cycles_transferrer_transfer_cycles_error));
            }
        }, 
        Err(cycles_transferrer_transfer_cycles_call_error) => {
            cancel_user_transfer_cycles();
            return Err(UserTransferCyclesError::CyclesTransferrerTransferCyclesCallError((cycles_transferrer_transfer_cycles_call_error.0 as u32, cycles_transferrer_transfer_cycles_call_error.1)));
        }
    }
    
}



// no check of the ctsfuel-balance here, cause of the check in the user_transfer_cycles-method. set on the side the ctsfuel for the callback?

#[update]
pub fn cycles_transferrer_transfer_cycles_callback(q: cycles_transferrer::TransferCyclesCallbackQuest) -> () {
    
    if with(&CB_DATA, |cb_data| { cb_data.cycles_transferrer_canisters.contains(&caller()) }) == false {
        trap("caller must be one of the CTS-cycles-transferrer-canisters for this method.");
    }
    
    //maintenance_check(); // make sure that when set a stop-call-flag, there are 0 ongoing-$cycles-transfers. cycles-transfer-callback errors will hold for
    
    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128()); 

    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(cycles_transfer_refund);
        if let Some(cycles_transfer_out/*: &mut CyclesTransferOut*/) = cb_data.user_data.cycles_transfers_out.iter_mut().rev().find(|cycles_transfer_out: &&mut CyclesTransferOut| { (**cycles_transfer_out).id == q.user_cycles_transfer_id }) {
            cycles_transfer_out.cycles_refunded = Some(cycles_transfer_refund);
            cycles_transfer_out.opt_cycles_transfer_call_error = q.opt_cycles_transfer_call_error;
        }
    });

}





#[query(manual_reply = true)]
pub fn download_cycles_transfers_out(q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();

    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, &cb_data.user_data.cycles_transfers_out),)); 
    });
}


#[update(manual_reply = true)]
pub fn delete_cycles_transfers_out(delete_cycles_transfers_out_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with_mut(&CB_DATA, |cb_data| {
        for delete_cycles_transfer_out_id in delete_cycles_transfers_out_ids.into_iter() {
            match cb_data.user_data.cycles_transfers_out.binary_search_by_key(&delete_cycles_transfer_out_id, |cycles_transfer_out| { cycles_transfer_out.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cycles_transfers_out[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cycles_transfer_out id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cycles_transfer_out_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cycles_transfers_out.remove(i);
                },
                Err(_) => {
                    trap(&format!("cycles_transfer_out id: {} not found.", delete_cycles_transfer_out_id))
                }
            }
        }
    });
    
    reply::<()>(());
}



// --------------------------
// bank-token-methods


#[update(manual_reply = true)]
pub async fn transfer_icrc1(icrc1_ledger: Principal, icrc1_transfer_arg_raw: Vec<u8>) {//-> CallResult<Vec<u8>> {
    if caller() != user_id() { trap("Caller must be the user"); }
    
    let call_result: CallResult<Vec<u8>> = call_raw128(
        icrc1_ledger,
        "icrc1_transfer",
        &icrc1_transfer_arg_raw,//&encode_one(&icrc1_transfer_arg).unwrap(),
        0
    ).await;
    
    reply::<(CallResult<Vec<u8>>,)>((call_result,));
}


// because the first icp account ids are not possible to use with the icrc1_transfer function.
#[update(manual_reply = true)]
pub async fn transfer_icp(transfer_arg_raw: Vec<u8>) {
    if caller() != user_id() { trap("Caller must be the user"); }
    
    let s: CallResult<Vec<u8>> = call_raw128(
        MAINNET_LEDGER_CANISTER_ID,
        "transfer",
        &transfer_arg_raw,
        0
    ).await;
    
    reply::<(CallResult<Vec<u8>>,)>((s,));

}




// ---------------------------------------------------
// cycles-market methods






#[derive(CandidType)]
pub enum UserCMCreateCyclesPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_create_position_fee: Cycles },
    CyclesMarketCreateCyclesPositionCallError((u32, String)),
    CyclesMarketCreateCyclesPositionCallSponseCandidDecodeError{candid_error: String, sponse_bytes: Vec<u8> },
    CyclesMarketCreateCyclesPositionError(cm_icrc1token_trade_contract::CreateCyclesPositionError)
}


#[update]
pub async fn cm_create_cycles_position(icrc1token_trade_contract: Icrc1TokenTradeContract, q: cm_icrc1token_trade_contract::CreateCyclesPositionQuest) -> Result<cm_icrc1token_trade_contract::CreateCyclesPositionSuccess, UserCMCreateCyclesPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 30_000_000_000 {
        return Err(UserCMCreateCyclesPositionError::CTSFuelTooLow);
    }
    
    if with(&CB_DATA, |cb_data| { calculate_free_storage(cb_data) }) < std::mem::size_of::<CMCyclesPosition>() as u128 {
        return Err(UserCMCreateCyclesPositionError::MemoryIsFull);
    }
   
    if with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) < q.cycles + CYCLES_MARKET_CREATE_POSITION_FEE {
        return Err(UserCMCreateCyclesPositionError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), cycles_market_create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE });
    }

    let mut call_future = call_raw128(   // <(&cycles_market::CreateCyclesPositionQuest,), (cycles_market::CreateCyclesPositionResult,)>
        icrc1token_trade_contract.trade_contract_canister_id,
        "create_cycles_position",
        &encode_one(&q).unwrap(),
        q.cycles + CYCLES_MARKET_CREATE_POSITION_FEE
    );
    
    if let futures::task::Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
        let call_error: (RejectionCode, String) = call_result_with_an_error.unwrap_err();
        return Err(UserCMCreateCyclesPositionError::CyclesMarketCreateCyclesPositionCallError((call_error.0 as u32, "call_perform error".to_string())));
    }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_sub(q.cycles + CYCLES_MARKET_CREATE_POSITION_FEE);
        cb_data.user_data.cm_trade_contracts.entry(icrc1token_trade_contract).or_insert(CMTradeContractLogs::new());
    });
    
    let call_result: CallResult<Vec<u8>> = call_future.await;

    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_refunded128());
    });
    
    match call_result {
        Ok(sponse_bytes) => match decode_one::<cm_icrc1token_trade_contract::CreateCyclesPositionResult>(&sponse_bytes) {
            Ok(cm_create_cycles_position_result) => match cm_create_cycles_position_result {
                Ok(cm_create_cycles_position_success) => {
                    with_mut(&CB_DATA, |cb_data| {
                        cb_data.user_data.cm_trade_contracts.get_mut(&icrc1token_trade_contract).unwrap().cm_calls_out.cm_cycles_positions.push(
                            CMCyclesPosition{
                                id: cm_create_cycles_position_success.position_id,
                                create_cycles_position_quest: q,
                                create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE as u64,
                                timestamp_nanos: time_nanos(),
                            }
                        );
                    });
                    Ok(cm_create_cycles_position_success)
                },
                Err(cm_create_cycles_position_error) => {
                    return Err(UserCMCreateCyclesPositionError::CyclesMarketCreateCyclesPositionError(cm_create_cycles_position_error));
                }
            },
            Err(candid_decode_error) => {
                return Err(UserCMCreateCyclesPositionError::CyclesMarketCreateCyclesPositionCallSponseCandidDecodeError{candid_error: format!("{:?}", candid_decode_error), sponse_bytes: sponse_bytes });
            }
        },
        Err(call_error) => {
            return Err(UserCMCreateCyclesPositionError::CyclesMarketCreateCyclesPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}



// ------------


#[derive(CandidType)]
pub enum UserCMCreateTokenPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_create_position_fee: Cycles },
    CyclesMarketCreateTokenPositionCallError((u32, String)),
    CyclesMarketCreateTokenPositionCallSponseCandidDecodeError{candid_error: String, sponse_bytes: Vec<u8> },
    CyclesMarketCreateTokenPositionError(cm_icrc1token_trade_contract::CreateTokenPositionError)
}


#[update]
pub async fn cm_create_token_position(icrc1token_trade_contract: Icrc1TokenTradeContract, q: cm_icrc1token_trade_contract::CreateTokenPositionQuest) -> Result<cm_icrc1token_trade_contract::CreateTokenPositionSuccess, UserCMCreateTokenPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 30_000_000_000 {
        return Err(UserCMCreateTokenPositionError::CTSFuelTooLow);
    }
    
    if with(&CB_DATA, |cb_data| { calculate_free_storage(cb_data) }) < std::mem::size_of::<CMTokenPosition>() as u128 {
        return Err(UserCMCreateTokenPositionError::MemoryIsFull);
    }
   
    if with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) < CYCLES_MARKET_CREATE_POSITION_FEE {
        return Err(UserCMCreateTokenPositionError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), cycles_market_create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE });
    }
    
    let mut call_future = call_raw128(
        icrc1token_trade_contract.trade_contract_canister_id,
        "create_token_position",
        &encode_one(&q).unwrap(),
        CYCLES_MARKET_CREATE_POSITION_FEE
    );
    
    if let futures::task::Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
        let call_error: (RejectionCode, String) = call_result_with_an_error.unwrap_err();
        return Err(UserCMCreateTokenPositionError::CyclesMarketCreateTokenPositionCallError((call_error.0 as u32, "call_perform error".to_string())));
    }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_sub(CYCLES_MARKET_CREATE_POSITION_FEE);
        cb_data.user_data.cm_trade_contracts.entry(icrc1token_trade_contract).or_insert(CMTradeContractLogs::new());
    });
    
    let call_result: CallResult<Vec<u8>> = call_future.await;

    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_refunded128());
    });

    match call_result {
        Ok(sponse_bytes) => match decode_one::<cm_icrc1token_trade_contract::CreateTokenPositionResult>(&sponse_bytes) {
            Ok(cm_create_token_position_result) => match cm_create_token_position_result {
                Ok(cm_create_token_position_success) => {
                    with_mut(&CB_DATA, |cb_data| {
                        cb_data.user_data.cm_trade_contracts.get_mut(&icrc1token_trade_contract).unwrap().cm_calls_out.cm_token_positions.push(
                            CMTokenPosition{
                                id: cm_create_token_position_success.position_id,   
                                create_token_position_quest: q,
                                create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE as u64,
                                timestamp_nanos: time_nanos(),
                            }
                        );
                    });
                    Ok(cm_create_token_position_success)
                },
                Err(cm_create_token_position_error) => {
                    return Err(UserCMCreateTokenPositionError::CyclesMarketCreateTokenPositionError(cm_create_token_position_error));
                }
            },
            Err(candid_decode_error) => {
                return Err(UserCMCreateTokenPositionError::CyclesMarketCreateTokenPositionCallSponseCandidDecodeError{candid_error: format!("{:?}", candid_decode_error), sponse_bytes: sponse_bytes });
            }
        },
        Err(call_error) => {
            return Err(UserCMCreateTokenPositionError::CyclesMarketCreateTokenPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}



// --------------

#[derive(CandidType, Deserialize)]
pub struct UserCMPurchaseCyclesPositionQuest {
    cycles_market_purchase_cycles_position_quest: cm_icrc1token_trade_contract::PurchaseCyclesPositionQuest,
    cycles_position_cycles_per_token_rate: cm_icrc1token_trade_contract::CyclesPerToken, // for the user_canister-log
    cycles_position_positor: Principal,
}

#[derive(CandidType)]
pub enum UserCMPurchaseCyclesPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_purchase_position_fee: Cycles },
    CyclesMarketPurchaseCyclesPositionCallError((u32, String)),
    CyclesMarketPurchaseCyclesPositionCallSponseCandidDecodeError{candid_error: String, sponse_bytes: Vec<u8>},
    CyclesMarketPurchaseCyclesPositionError(cm_icrc1token_trade_contract::PurchaseCyclesPositionError)
}


#[update]
pub async fn cm_purchase_cycles_position(icrc1token_trade_contract: Icrc1TokenTradeContract, q: UserCMPurchaseCyclesPositionQuest) -> Result<cm_icrc1token_trade_contract::PurchaseCyclesPositionSuccess, UserCMPurchaseCyclesPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 30_000_000_000 {
        return Err(UserCMPurchaseCyclesPositionError::CTSFuelTooLow);
    }
    
    if with(&CB_DATA, |cb_data| { calculate_free_storage(cb_data) }) < std::mem::size_of::<CMCyclesPositionPurchase>() as u128 {
        return Err(UserCMPurchaseCyclesPositionError::MemoryIsFull);
    }
    
    if with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) < CYCLES_MARKET_PURCHASE_POSITION_FEE {
        return Err(UserCMPurchaseCyclesPositionError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), cycles_market_purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE });
    }
    
    let mut call_future = call_raw128(  // <(&cycles_market::PurchaseCyclesPositionQuest,), (cycles_market::PurchaseCyclesPositionResult,)>
        icrc1token_trade_contract.trade_contract_canister_id,
        "purchase_cycles_position",
        &encode_one(&q.cycles_market_purchase_cycles_position_quest).unwrap(), // unwrap is safe here bc before the first await
        CYCLES_MARKET_PURCHASE_POSITION_FEE
    );
    
    if let futures::task::Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
        let call_error: (RejectionCode, String) = call_result_with_an_error.unwrap_err();
        return Err(UserCMPurchaseCyclesPositionError::CyclesMarketPurchaseCyclesPositionCallError((call_error.0 as u32, "call_perform error".to_string())));
    }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_sub(CYCLES_MARKET_PURCHASE_POSITION_FEE);
        cb_data.user_data.cm_trade_contracts.entry(icrc1token_trade_contract).or_insert(CMTradeContractLogs::new());
    });
    
    let call_result: CallResult<Vec<u8>> = call_future.await;

    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_refunded128());
    });

    match call_result {
        Ok(sponse_bytes) => match decode_one::<cm_icrc1token_trade_contract::PurchaseCyclesPositionResult>(&sponse_bytes) {
            Ok(cm_purchase_cycles_position_result) => match cm_purchase_cycles_position_result {
                Ok(cm_purchase_cycles_position_success) => {
                    with_mut(&CB_DATA, |cb_data| {
                        cb_data.user_data.cm_trade_contracts.get_mut(&icrc1token_trade_contract).unwrap().cm_calls_out.cm_cycles_positions_purchases.push(
                            CMCyclesPositionPurchase{
                                cycles_position_id: q.cycles_market_purchase_cycles_position_quest.cycles_position_id,
                                cycles_position_cycles_per_token_rate: q.cycles_position_cycles_per_token_rate,
                                cycles_position_positor: q.cycles_position_positor,
                                id: cm_purchase_cycles_position_success.purchase_id,
                                cycles: q.cycles_market_purchase_cycles_position_quest.cycles,
                                purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE as u64,
                                timestamp_nanos: time_nanos(),
                            }
                        );
                    });
                    Ok(cm_purchase_cycles_position_success)
                },
                Err(cm_purchase_cycles_position_error) => {
                    return Err(UserCMPurchaseCyclesPositionError::CyclesMarketPurchaseCyclesPositionError(cm_purchase_cycles_position_error));
                }
            },
            Err(candid_decode_error) => {
                return Err(UserCMPurchaseCyclesPositionError::CyclesMarketPurchaseCyclesPositionCallSponseCandidDecodeError{candid_error: format!("{:?}", candid_decode_error), sponse_bytes: sponse_bytes });
            }
        },
        Err(call_error) => {
            return Err(UserCMPurchaseCyclesPositionError::CyclesMarketPurchaseCyclesPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}


// ---------------

#[derive(CandidType, Deserialize)]
pub struct UserCMPurchaseTokenPositionQuest {
    cycles_market_purchase_token_position_quest: cm_icrc1token_trade_contract::PurchaseTokenPositionQuest,
    token_position_cycles_per_token_rate: cm_icrc1token_trade_contract::CyclesPerToken, // for the user_canister-log
    token_position_positor: Principal,
}

#[derive(CandidType)]
pub enum UserCMPurchaseTokenPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_purchase_position_fee: Cycles },
    CyclesMarketPurchaseTokenPositionCallError((u32, String)),
    CyclesMarketPurchaseTokenPositionCallSponseCandidDecodeError{candid_error: String, sponse_bytes: Vec<u8>},    
    CyclesMarketPurchaseTokenPositionError(cm_icrc1token_trade_contract::PurchaseTokenPositionError)
}


#[update]
pub async fn cm_purchase_token_position(icrc1token_trade_contract: Icrc1TokenTradeContract, q: UserCMPurchaseTokenPositionQuest) -> Result<cm_icrc1token_trade_contract::PurchaseTokenPositionSuccess, UserCMPurchaseTokenPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 30_000_000_000 {
        return Err(UserCMPurchaseTokenPositionError::CTSFuelTooLow);
    }
    
    if with(&CB_DATA, |cb_data| { calculate_free_storage(cb_data) }) < std::mem::size_of::<CMTokenPositionPurchase>() as u128 {
        return Err(UserCMPurchaseTokenPositionError::MemoryIsFull);
    }
    
    let purchase_token_position_cycles_payment: Cycles = tokens_transform_cycles(q.cycles_market_purchase_token_position_quest.tokens, q.token_position_cycles_per_token_rate);
    
    if with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) < CYCLES_MARKET_PURCHASE_POSITION_FEE + purchase_token_position_cycles_payment {
        return Err(UserCMPurchaseTokenPositionError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), cycles_market_purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE });
    }
    
    let mut call_future = call_raw128(
        icrc1token_trade_contract.trade_contract_canister_id,
        "purchase_token_position",
        &encode_one(&q.cycles_market_purchase_token_position_quest).unwrap(),
        CYCLES_MARKET_PURCHASE_POSITION_FEE + purchase_token_position_cycles_payment       
    );
    
    if let futures::task::Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
        let call_error: (RejectionCode, String) = call_result_with_an_error.unwrap_err();
        return Err(UserCMPurchaseTokenPositionError::CyclesMarketPurchaseTokenPositionCallError((call_error.0 as u32, "call_perform error".to_string())));
    }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_sub(CYCLES_MARKET_PURCHASE_POSITION_FEE + purchase_token_position_cycles_payment);
        cb_data.user_data.cm_trade_contracts.entry(icrc1token_trade_contract).or_insert(CMTradeContractLogs::new());
    });
    
    let call_result: CallResult<Vec<u8>> = call_future.await;

    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_refunded128());
    });

    match call_result {
        Ok(sponse_bytes) => match decode_one::<cm_icrc1token_trade_contract::PurchaseTokenPositionResult>(&sponse_bytes) {
            Ok(cm_purchase_token_position_result) => match cm_purchase_token_position_result {
                Ok(cm_purchase_token_position_success) => {
                    with_mut(&CB_DATA, |cb_data| {
                        cb_data.user_data.cm_trade_contracts.get_mut(&icrc1token_trade_contract).unwrap().cm_calls_out.cm_token_positions_purchases.push(
                            CMTokenPositionPurchase{
                                token_position_id: q.cycles_market_purchase_token_position_quest.token_position_id,
                                token_position_cycles_per_token_rate: q.token_position_cycles_per_token_rate,
                                token_position_positor: q.token_position_positor,
                                id: cm_purchase_token_position_success.purchase_id,
                                tokens: q.cycles_market_purchase_token_position_quest.tokens,
                                purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE as u64,
                                timestamp_nanos: time_nanos(),
                            }
                        );
                    });
                    Ok(cm_purchase_token_position_success)
                },
                Err(cm_purchase_token_position_error) => {
                    return Err(UserCMPurchaseTokenPositionError::CyclesMarketPurchaseTokenPositionError(cm_purchase_token_position_error));
                }
            },
            Err(candid_decode_error) => {
                return Err(UserCMPurchaseTokenPositionError::CyclesMarketPurchaseTokenPositionCallSponseCandidDecodeError{candid_error: format!("{:?}", candid_decode_error), sponse_bytes: sponse_bytes });
            }
        },
        Err(call_error) => {
            return Err(UserCMPurchaseTokenPositionError::CyclesMarketPurchaseTokenPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}


// ---------------------

#[derive(CandidType)]
pub enum UserCMVoidPositionError {
    CTSFuelTooLow,
    CyclesMarketVoidPositionCallError((u32, String)),
    CyclesMarketVoidPositionError(cm_icrc1token_trade_contract::VoidPositionError)
}


#[update]
pub async fn cm_void_position(icrc1token_trade_contract: Icrc1TokenTradeContract, q: cm_icrc1token_trade_contract::VoidPositionQuest) -> Result<(), UserCMVoidPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 30_000_000_000 {
        return Err(UserCMVoidPositionError::CTSFuelTooLow);
    }
    
    match call::<(cm_icrc1token_trade_contract::VoidPositionQuest,), (Result<(), cm_icrc1token_trade_contract::VoidPositionError>,)>(
        icrc1token_trade_contract.trade_contract_canister_id,
        "void_position",
        (q,)
    ).await {
        Ok((cm_void_position_result,)) => match cm_void_position_result {
            Ok(()) => Ok(()),
            Err(cm_void_position_error) => {
                return Err(UserCMVoidPositionError::CyclesMarketVoidPositionError(cm_void_position_error));
            }
        },
        Err(call_error) => {
            return Err(UserCMVoidPositionError::CyclesMarketVoidPositionCallError((call_error.0 as u32, call_error.1)));
        }
    
    }
    
}


// -------


#[derive(CandidType)]
pub enum UserCMTransferTokenBalanceError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_transfer_token_balance_fee: Cycles },
    CyclesMarketTransferTokenBalanceCallError((u32, String)),
    CyclesMarketTransferTokenBalanceCallSponseCandidDecodeError{candid_error: String, sponse_bytes: Vec<u8> },
    CyclesMarketTransferTokenBalanceError(cm_icrc1token_trade_contract::TransferTokenBalanceError)
}

#[update]
pub async fn cm_transfer_token_balance(icrc1token_trade_contract: Icrc1TokenTradeContract, q: cm_icrc1token_trade_contract::TransferTokenBalanceQuest) -> Result<BlockId, UserCMTransferTokenBalanceError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 30_000_000_000 {
        return Err(UserCMTransferTokenBalanceError::CTSFuelTooLow);
    }

    if with(&CB_DATA, |cb_data| { calculate_free_storage(cb_data) }) < std::mem::size_of::<CMTokenTransferOut>() as u128 {
        return Err(UserCMTransferTokenBalanceError::MemoryIsFull);
    }

    if with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) < CYCLES_MARKET_TRANSFER_TOKEN_BALANCE_FEE {
        return Err(UserCMTransferTokenBalanceError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), cycles_market_transfer_token_balance_fee: CYCLES_MARKET_TRANSFER_TOKEN_BALANCE_FEE });
    }
    
    let mut call_future = call_raw128(
        icrc1token_trade_contract.trade_contract_canister_id,
        "transfer_token_balance",
        &encode_one(&q).unwrap(),
        CYCLES_MARKET_TRANSFER_TOKEN_BALANCE_FEE
    );
    
    if let futures::task::Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
        let call_error: (RejectionCode, String) = call_result_with_an_error.unwrap_err();
        return Err(UserCMTransferTokenBalanceError::CyclesMarketTransferTokenBalanceCallError((call_error.0 as u32, "call_perform error".to_string())));
    }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_sub(CYCLES_MARKET_TRANSFER_TOKEN_BALANCE_FEE);
        cb_data.user_data.cm_trade_contracts.entry(icrc1token_trade_contract).or_insert(CMTradeContractLogs::new());
    });
    
    let call_result: CallResult<Vec<u8>> = call_future.await;

    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_refunded128());
    });

    match call_result {
        Ok(sponse_bytes) => match decode_one::<cm_icrc1token_trade_contract::TransferTokenBalanceResult>(&sponse_bytes) {
            Ok(cm_transfer_token_balance_result) => match cm_transfer_token_balance_result {
                Ok(block_height) => {
                    with_mut(&CB_DATA, |cb_data| {
                        cb_data.user_data.cm_trade_contracts.get_mut(&icrc1token_trade_contract).unwrap().cm_calls_out.cm_token_transfers_out.push(
                            CMTokenTransferOut{
                                tokens: q.tokens,
                                token_ledger_transfer_fee: q.token_fee,
                                to: q.to,
                                block_height: block_height.clone(),
                                timestamp_nanos: time_nanos(),
                                transfer_token_balance_fee: CYCLES_MARKET_TRANSFER_TOKEN_BALANCE_FEE as u64
                            }
                        );
                    });
                    Ok(block_height)
                },
                Err(cm_transfer_token_balance_error) => {
                    return Err(UserCMTransferTokenBalanceError::CyclesMarketTransferTokenBalanceError(cm_transfer_token_balance_error));
                }
            },
            Err(candid_decode_error) => {
                return Err(UserCMTransferTokenBalanceError::CyclesMarketTransferTokenBalanceCallSponseCandidDecodeError{candid_error: format!("{:?}", candid_decode_error), sponse_bytes: sponse_bytes });                
            }
        },
        Err(call_error) => {
            return Err(UserCMTransferTokenBalanceError::CyclesMarketTransferTokenBalanceCallError((call_error.0 as u32, call_error.1)));
        }
    }

}



// -------------------------------

fn get_mut_cm_trade_contract_logs_of_the_cm_caller_or_trap(cb_data: &mut CBData) -> &mut CMTradeContractLogs {
    cb_data.user_data.cm_trade_contracts
        .iter_mut()
        .find(|(k,_v): &(&Icrc1TokenTradeContract, &mut CMTradeContractLogs)| {
            let mut possible_callers: Vec<Principal> = vec![k.trade_contract_canister_id];
            if let Some(cm_caller) = k.opt_cm_caller { possible_callers.push(cm_caller); }
            possible_callers.contains(&caller())
        })
        .map(|(_k,v): (&Icrc1TokenTradeContract, &mut CMTradeContractLogs)| {
            v
        })
        .unwrap_or_else(|| trap("Unknown caller"))
}

#[update]
pub fn cm_message_cycles_position_purchase_positor(q: cm_icrc1token_trade_contract::CMCyclesPositionPurchasePositorMessageQuest) {
    
    with_mut(&CB_DATA, |cb_data| {
        get_mut_cm_trade_contract_logs_of_the_cm_caller_or_trap(cb_data).cm_message_logs.cm_message_cycles_position_purchase_positor_logs.push(
            CMMessageCyclesPositionPurchasePositorLog{
                timestamp_nanos: time_nanos(),
                cm_message_cycles_position_purchase_positor_quest: q
            }
        );
    });

}

#[update]
pub fn cm_message_cycles_position_purchase_purchaser(q: cm_icrc1token_trade_contract::CMCyclesPositionPurchasePurchaserMessageQuest) {
    
    let cycles_purchase: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(cycles_purchase); 
        get_mut_cm_trade_contract_logs_of_the_cm_caller_or_trap(cb_data).cm_message_logs.cm_message_cycles_position_purchase_purchaser_logs.push(
            CMMessageCyclesPositionPurchasePurchaserLog{
                timestamp_nanos: time_nanos(),
                cycles_purchase,
                cm_message_cycles_position_purchase_purchaser_quest: q
            }
        );
    });    

}

#[update]
pub fn cm_message_token_position_purchase_positor(q: cm_icrc1token_trade_contract::CMTokenPositionPurchasePositorMessageQuest) {
    
    let cycles_payment: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(cycles_payment); 
        get_mut_cm_trade_contract_logs_of_the_cm_caller_or_trap(cb_data).cm_message_logs.cm_message_token_position_purchase_positor_logs.push(
            CMMessageTokenPositionPurchasePositorLog{
                timestamp_nanos: time_nanos(),
                cycles_payment,
                cm_message_token_position_purchase_positor_quest: q
            }
        );
    });
    
}

#[update]
pub fn cm_message_token_position_purchase_purchaser(q: cm_icrc1token_trade_contract::CMTokenPositionPurchasePurchaserMessageQuest) {
    
    with_mut(&CB_DATA, |cb_data| {
        get_mut_cm_trade_contract_logs_of_the_cm_caller_or_trap(cb_data).cm_message_logs.cm_message_token_position_purchase_purchaser_logs.push(
            CMMessageTokenPositionPurchasePurchaserLog{
                timestamp_nanos: time_nanos(),
                cm_message_token_position_purchase_purchaser_quest: q
            }
        );
    });
    
}

#[update]
pub fn cm_message_void_cycles_position_positor(q: cm_icrc1token_trade_contract::CMVoidCyclesPositionPositorMessageQuest) {
    
    let void_cycles: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(void_cycles); 
        get_mut_cm_trade_contract_logs_of_the_cm_caller_or_trap(cb_data).cm_message_logs.cm_message_void_cycles_position_positor_logs.push(
            CMMessageVoidCyclesPositionPositorLog{
                timestamp_nanos: time_nanos(),
                void_cycles,
                cm_message_void_cycles_position_positor_quest: q
            }
        );
    });

}

#[update]
pub fn cm_message_void_token_position_positor(q: cm_icrc1token_trade_contract::CMVoidTokenPositionPositorMessageQuest) {

    with_mut(&CB_DATA, |cb_data| {
        get_mut_cm_trade_contract_logs_of_the_cm_caller_or_trap(cb_data).cm_message_logs.cm_message_void_token_position_positor_logs.push(
            CMMessageVoidTokenPositionPositorLog{
                timestamp_nanos: time_nanos(),
                cm_message_void_token_position_positor_quest: q
            }
        );
    });


}





// -------------------------------
// download cm data






#[query(manual_reply = true)]
pub fn download_cm_cycles_positions(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_calls_out.cm_cycles_positions).unwrap_or(&vec![])),));
    });    
}

#[query(manual_reply = true)]
pub fn download_cm_token_positions(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_calls_out.cm_token_positions).unwrap_or(&vec![])),));
    });
}

#[query(manual_reply = true)]
pub fn download_cm_cycles_positions_purchases(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_calls_out.cm_cycles_positions_purchases).unwrap_or(&vec![])),));
    });
}



#[query(manual_reply = true)]
pub fn download_cm_token_positions_purchases(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_calls_out.cm_token_positions_purchases).unwrap_or(&vec![])),));
    });
}


#[query(manual_reply = true)]
pub fn download_cm_token_transfers_out(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_calls_out.cm_token_transfers_out).unwrap_or(&vec![])),));
    });
}



// -----------------

#[query(manual_reply = true)]
pub fn download_cm_message_cycles_position_purchase_positor_logs(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_message_logs.cm_message_cycles_position_purchase_positor_logs).unwrap_or(&vec![])),));
    });
}


#[query(manual_reply = true)]
pub fn download_cm_message_cycles_position_purchase_purchaser_logs(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_message_logs.cm_message_cycles_position_purchase_purchaser_logs).unwrap_or(&vec![])),));
    });
}


#[query(manual_reply = true)]
pub fn download_cm_message_token_position_purchase_positor_logs(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_message_logs.cm_message_token_position_purchase_positor_logs).unwrap_or(&vec![])),));
    });
}



#[query(manual_reply = true)]
pub fn download_cm_message_token_position_purchase_purchaser_logs(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_message_logs.cm_message_token_position_purchase_purchaser_logs).unwrap_or(&vec![])),));
    });
}

#[query(manual_reply = true)]
pub fn download_cm_message_void_cycles_position_positor_logs(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_message_logs.cm_message_void_cycles_position_positor_logs).unwrap_or(&vec![])),));
    });
}



#[query(manual_reply = true)]
pub fn download_cm_message_void_token_position_positor_logs(icrc1token_trade_contract: Icrc1TokenTradeContract, q: DownloadCBLogsQuest) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if with(&CB_DATA, |cb_data| { ctsfuel_balance(cb_data) }) < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    maintenance_check();
    
    with(&CB_DATA, |cb_data| {
        reply((download_logs(q, cb_data.user_data.cm_trade_contracts.get(&icrc1token_trade_contract).map(|tc| &tc.cm_message_logs.cm_message_void_token_position_positor_logs).unwrap_or(&vec![])),));
    });
}



// ----------------------------------------------------------




#[derive(CandidType)]
pub struct UserUCMetrics<'a> {
    global_allocator_counter: u64,
    cycles_balance: Cycles,
    ctsfuel_balance: CTSFuel,
    storage_size_mib: u128,
    lifetime_termination_timestamp_seconds: u128,
    cycles_transferrer_canisters: &'a Vec<Principal>,
    user_id: Principal,
    user_canister_creation_timestamp_nanos: u128,
    storage_usage: u128,
    cycles_transfers_id_counter: u128,
    cycles_transfers_in_len: u128,
    cycles_transfers_out_len: u128,
    cm_trade_contracts_logs_lengths: HashMap<&'a Icrc1TokenTradeContract, CMTradeContractLogsLengths>,    
    
}


#[derive(CandidType)]
pub struct CMTradeContractLogsLengths {
    cm_calls_out_lengths: CMCallsOutLengths,
    cm_message_logs_lengths: CMMessageLogsLengths,
}
#[derive(CandidType)]
pub struct CMCallsOutLengths {
    cm_cycles_positions_length: u64,
    cm_token_positions_length: u64,
    cm_cycles_positions_purchases_length: u64,
    cm_token_positions_purchases_length: u64,    
    cm_token_transfers_out_length: u64,
}
#[derive(CandidType)]
pub struct CMMessageLogsLengths{
    cm_message_cycles_position_purchase_positor_logs_length: u64,
    cm_message_cycles_position_purchase_purchaser_logs_length: u64,
    cm_message_token_position_purchase_positor_logs_length: u64,
    cm_message_token_position_purchase_purchaser_logs_length: u64,
    cm_message_void_cycles_position_positor_logs_length: u64,
    cm_message_void_token_position_positor_logs_length: u64,    
}

fn cm_trade_contract_logs_lengths(cm_trade_contract_logs: &CMTradeContractLogs) -> CMTradeContractLogsLengths {
    CMTradeContractLogsLengths{
        cm_calls_out_lengths: CMCallsOutLengths{
            cm_cycles_positions_length: cm_trade_contract_logs.cm_calls_out.cm_cycles_positions.len() as u64,
            cm_token_positions_length: cm_trade_contract_logs.cm_calls_out.cm_token_positions.len() as u64,
            cm_cycles_positions_purchases_length: cm_trade_contract_logs.cm_calls_out.cm_cycles_positions_purchases.len() as u64,
            cm_token_positions_purchases_length: cm_trade_contract_logs.cm_calls_out.cm_token_positions_purchases.len() as u64,    
            cm_token_transfers_out_length: cm_trade_contract_logs.cm_calls_out.cm_token_transfers_out.len() as u64,
        },
        cm_message_logs_lengths: CMMessageLogsLengths{
            cm_message_cycles_position_purchase_positor_logs_length: cm_trade_contract_logs.cm_message_logs.cm_message_cycles_position_purchase_positor_logs.len() as u64,
            cm_message_cycles_position_purchase_purchaser_logs_length: cm_trade_contract_logs.cm_message_logs.cm_message_cycles_position_purchase_purchaser_logs.len() as u64,
            cm_message_token_position_purchase_positor_logs_length: cm_trade_contract_logs.cm_message_logs.cm_message_token_position_purchase_positor_logs.len() as u64,
            cm_message_token_position_purchase_purchaser_logs_length: cm_trade_contract_logs.cm_message_logs.cm_message_token_position_purchase_purchaser_logs.len() as u64,
            cm_message_void_cycles_position_positor_logs_length: cm_trade_contract_logs.cm_message_logs.cm_message_void_cycles_position_positor_logs.len() as u64,
            cm_message_void_token_position_positor_logs_length: cm_trade_contract_logs.cm_message_logs.cm_message_void_token_position_positor_logs.len() as u64,  
        },
    }
}


#[query(manual_reply = true)]
pub fn metrics() { //-> UserUCMetrics {
    if caller() != user_id() && caller() != cts_id() {
        trap("Caller must be the user for this method.");
    }
    
    with(&CB_DATA, |cb_data| {
        reply::<(UserUCMetrics,)>((UserUCMetrics{
            global_allocator_counter: get_allocated_bytes_count() as u64,
            cycles_balance: cb_data.user_data.cycles_balance,
            ctsfuel_balance: ctsfuel_balance(cb_data),
            storage_size_mib: cb_data.storage_size_mib,
            lifetime_termination_timestamp_seconds: cb_data.lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters: &(cb_data.cycles_transferrer_canisters),
            user_id: cb_data.user_id,
            user_canister_creation_timestamp_nanos: cb_data.user_canister_creation_timestamp_nanos,
            storage_usage: calculate_current_storage_usage(cb_data),
            cycles_transfers_id_counter: cb_data.cycles_transfers_id_counter,
            cycles_transfers_in_len: cb_data.user_data.cycles_transfers_in.len() as u128,
            cycles_transfers_out_len: cb_data.user_data.cycles_transfers_out.len() as u128,
            cm_trade_contracts_logs_lengths: cb_data.user_data.cm_trade_contracts.iter().map(|(k,v)| { (k, cm_trade_contract_logs_lengths(v)) }).collect(),
        },));
    });
}


// --------------------------------------------------------

/*

#[derive(CandidType)]
pub enum UserCyclesBalanceForTheCTSFuelError {
    MinimumCyclesForTheCTSFuel{ minimum_cycles_for_the_ctsfuel: Cycles },
    CyclesBalanceTooLow { cycles_balance: Cycles }
}

#[update]
pub fn cycles_balance_for_the_ctsfuel(cycles_for_the_ctsfuel: Cycles) -> Result<(), UserCyclesBalanceForTheCTSFuelError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    maintenance_check();
    
    if cycles_for_the_ctsfuel < MINIMUM_CYCLES_FOR_THE_CTSFUEL {
        return Err(UserCyclesBalanceForTheCTSFuelError::MinimumCyclesForTheCTSFuel{ minimum_cycles_for_the_ctsfuel: MINIMUM_CYCLES_FOR_THE_CTSFUEL });
    }
    
    if with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) < cycles_for_the_ctsfuel {
        return Err(UserCyclesBalanceForTheCTSFuelError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) });        
    } 
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance -= cycles_for_the_ctsfuel;
        // cycles-transfer-out log? what if storage is full and ctsfuel is empty?
    });
    
    Ok(())
}



// ---------------------------------------------



#[derive(CandidType)]
pub enum LengthenLifetimeError {
    MinimumSetLifetimeTerminationTimestampSeconds(u128),
    CyclesBalanceTooLow{ cycles_balance: Cycles, lengthen_cost_cycles: Cycles },
    CBSMCallError((u32, String))
}

#[update]
pub async fn lengthen_lifetime(q: LengthenLifetimeQuest) -> Result<u128/*new-lifetime-termination-timestamp-seconds*/, LengthenLifetimeError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    maintenance_check();

    let minimum_set_lifetime_termination_timestamp_seconds: u128 = with(&CB_DATA, |cb_data| { cb_data.lifetime_termination_timestamp_seconds }).checked_add(MINIMUM_LENGTHEN_LIFETIME_SECONDS).unwrap_or_else(|| { trap("time is not support at the moment") });
    if q.set_lifetime_termination_timestamp_seconds < minimum_set_lifetime_termination_timestamp_seconds {
        return Err(LengthenLifetimeError::MinimumSetLifetimeTerminationTimestampSeconds(minimum_set_lifetime_termination_timestamp_seconds));
    }

    let lengthen_cost_cycles: Cycles = {
        ( q.set_lifetime_termination_timestamp_seconds - with(&CB_DATA, |cb_data| { cb_data.lifetime_termination_timestamp_seconds }) )
        * cts_lib::tools::cb_storage_size_mib_as_cb_network_memory_allocation_mib(with(&CB_DATA, |cb_data| { cb_data.storage_size_mib })) // canister-memory-allocation in the mib 
        * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES / 1024/*network storage charge per MiB per second*/
    };
    
    if lengthen_cost_cycles > with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) {
        return Err(LengthenLifetimeError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), lengthen_cost_cycles });
    }
    
    with_mut(&CB_DATA, |cb_data| {    
        cb_data.user_data.cycles_balance -= lengthen_cost_cycles; 
    });
    
    match call::<(&LengthenLifetimeQuest,),()>(
        with(&CB_DATA, |cb_data| { cb_data.cbsm_id }),
        "cb_lengthen_lifetime",
        (&q,),
    ).await {
        Ok(()) => {
            with_mut(&CB_DATA, |cb_data| {    
                cb_data.lifetime_termination_timestamp_seconds = q.set_lifetime_termination_timestamp_seconds;
                Ok(cb_data.lifetime_termination_timestamp_seconds)
            })                    
        },
        Err(call_error) => {
            with_mut(&CB_DATA, |cb_data| {    
                cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(lengthen_cost_cycles); 
            });
            return Err(LengthenLifetimeError::CBSMCallError((call_error.0 as u32, call_error.1)));
        }
    }
    
}



// ---------------------------

#[derive(CandidType, Deserialize)]
pub struct UserChangeStorageSizeQuest {
    new_storage_size_mib: u128
}

#[derive(CandidType)]
pub enum UserChangeStorageSizeMibError {
    NewStorageSizeMibTooLow{ minimum_new_storage_size_mib: u128 },
    NewStorageSizeMibTooHigh{ maximum_new_storage_size_mib: u128 },
    CyclesBalanceTooLow{ cycles_balance: Cycles, new_storage_size_mib_cost_cycles: Cycles },
    ManagementCanisterUpdateSettingsCallError((u32, String))
}

#[update]
pub async fn change_storage_size(q: UserChangeStorageSizeQuest) -> Result<(), UserChangeStorageSizeMibError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    let minimum_new_storage_size_mib: u128 = with(&CB_DATA, |cb_data| { cb_data.storage_size_mib }) + 10; 
    
    if q.new_storage_size_mib < minimum_new_storage_size_mib  {
        return Err(UserChangeStorageSizeMibError::NewStorageSizeMibTooLow{ minimum_new_storage_size_mib }); 
    };
    
    if q.new_storage_size_mib > MAXIMUM_STORAGE_SIZE_MiB {
        return Err(UserChangeStorageSizeMibError::NewStorageSizeMibTooHigh{ maximum_new_storage_size_mib: MAXIMUM_STORAGE_SIZE_MiB });     
    }
    
    let new_storage_size_mib_cost_cycles: Cycles = {
        ( cts_lib::tools::cb_storage_size_mib_as_cb_network_memory_allocation_mib(q.new_storage_size_mib) - cts_lib::tools::cb_storage_size_mib_as_cb_network_memory_allocation_mib(with(&CB_DATA, |cb_data| { cb_data.storage_size_mib })) ) // grow canister-memory-allocation in the mib 
        * with(&CB_DATA, |cb_data| { cb_data.lifetime_termination_timestamp_seconds }).checked_sub(time_seconds()).unwrap_or_else(|| { trap("user-contract-lifetime is with the termination.") })
        * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES / 1024 /*network storage charge per MiB per second*/
    };
    
    if with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }) < new_storage_size_mib_cost_cycles {
        return Err(UserChangeStorageSizeMibError::CyclesBalanceTooLow{ cycles_balance: with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance }), new_storage_size_mib_cost_cycles });
    }

    // take the cycles before the .await and if error after here, refund the cycles
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance -= new_storage_size_mib_cost_cycles; 
    });

    match call::<(management_canister::ChangeCanisterSettingsRecord,), ()>(
        MANAGEMENT_CANISTER_ID,
        "update_settings",
        (management_canister::ChangeCanisterSettingsRecord{
            canister_id: ic_cdk::api::id(),
            settings: management_canister::ManagementCanisterOptionalCanisterSettings{
                controllers : None,
                compute_allocation : None,
                memory_allocation : Some((cts_lib::tools::cb_storage_size_mib_as_cb_network_memory_allocation_mib(q.new_storage_size_mib) * MiB as u128).into()),
                freezing_threshold : None,
            }
        },)
    ).await {
        Ok(()) => {
            with_mut(&CB_DATA, |cb_data| {
                cb_data.storage_size_mib = q.new_storage_size_mib;
            });
            Ok(())
        },
        Err(call_error) => {
            with_mut(&CB_DATA, |cb_data| {
                cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(new_storage_size_mib_cost_cycles); 
            });
            return Err(UserChangeStorageSizeMibError::ManagementCanisterUpdateSettingsCallError((call_error.0 as u32, call_error.1)));
        }
    }


}

*/


// -----------------------------------------------------------------------------------







#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::get(&STOP_CALLS)
}




// -------------------------------------------------------------------------

#[derive(CandidType)]
pub struct CTSUCMetrics {
    canister_cycles_balance: Cycles,
    cycles_balance: Cycles,
    ctsfuel_balance: CTSFuel,
    wasm_memory_size_bytes: u128,
    stable_memory_size_bytes: u64,
    storage_size_mib: u128,
    lifetime_termination_timestamp_seconds: u128,
    cycles_transferrer_canisters: Vec<Principal>,
    user_id: Principal,
    user_canister_creation_timestamp_nanos: u128,
    cycles_transfers_id_counter: u128,
    cycles_transfers_out_len: u128,
    cycles_transfers_in_len: u128,
    memory_size_at_the_start: u128,
    storage_usage: u128,
    free_storage: u128,
}


#[query]
pub fn cts_see_metrics() -> CTSUCMetrics {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    with(&CB_DATA, |cb_data| {
        CTSUCMetrics{
            canister_cycles_balance: canister_balance128(),
            cycles_balance: cb_data.user_data.cycles_balance,
            ctsfuel_balance: ctsfuel_balance(cb_data),
            wasm_memory_size_bytes: ( core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES ) as u128,
            stable_memory_size_bytes: ic_cdk::api::stable::stable64_size() * WASM_PAGE_SIZE_BYTES as u64,
            storage_size_mib: cb_data.storage_size_mib,
            lifetime_termination_timestamp_seconds: cb_data.lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters: cb_data.cycles_transferrer_canisters.clone(),
            user_id: cb_data.user_id,
            user_canister_creation_timestamp_nanos: cb_data.user_canister_creation_timestamp_nanos,
            cycles_transfers_id_counter: cb_data.cycles_transfers_id_counter,
            cycles_transfers_in_len: cb_data.user_data.cycles_transfers_in.len() as u128,
            cycles_transfers_out_len: cb_data.user_data.cycles_transfers_out.len() as u128,
            memory_size_at_the_start: localkey::cell::get(&MEMORY_SIZE_AT_THE_START) as u128,
            storage_usage: calculate_current_storage_usage(cb_data),
            free_storage: calculate_free_storage(cb_data)
        }
    })
}









