use std::{
    cell::{Cell, RefCell},
    collections::{HashSet},
    time::Duration,
};
use cts_lib::{
    tools::{
        localkey::{
            self,
            refcell::{with, with_mut}
        },
        principal_as_thirty_bytes,
        cycles_transform_tokens,
        tokens_transform_cycles,
        principal_token_subaccount,
        time_nanos,
        time_nanos_u64,
        time_seconds,
        caller_is_controller_gaurd,
        call_error_as_u32_and_string,
    },
    consts::{
        KiB,
        MiB,
        MANAGEMENT_CANISTER_ID,
        NANOS_IN_A_SECOND,
        SECONDS_IN_AN_HOUR,
    },
    types::{
        Cycles,
        CyclesTransferRefund,
        CallError,
        canister_code::CanisterCode,
        cycles_market::{icrc1token_trade_contract::{*, icrc1token_trade_log_storage::*}, cm_caller::*},
    },
    management_canister,
    icrc::{
        IcrcId, 
        //IcrcSub,
        //ICRC_DEFAULT_SUBACCOUNT,
        IcrcMemo,
        Tokens,
        TokenTransferError,
        TokenTransferArg,
        BlockId,
        icrc1_transfer,
        icrc1_balance_of
    },
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                call,
                call_with_payment128,
                call_raw128,
                reply,
                msg_cycles_refunded128,
                msg_cycles_available128,
                msg_cycles_accept128,
            },
            canister_balance128,
            /*
            stable::{
                stable64_write,
                stable64_size,
                stable64_read,
                stable64_grow
           }
           */
           
        },
        export::{
            Principal,
            candid::{
                self, 
                CandidType,
                Deserialize,
                utils::{encode_one, decode_one},
                error::Error as CandidError,
            }
        },
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    },
    stable_memory_tools::{self, MemoryId},
};


use serde_bytes::{ByteBuf, Bytes};
use serde::Serialize;

// -------

mod types;
use types::*;

mod payouts;
use payouts::_do_payouts;



// ---------------

// round robin on multiple cm_callers if the load gets heavy. scalable payouts!







#[derive(Serialize, Deserialize)]
struct OldCMData {}

#[derive(Serialize, Deserialize)]
struct CMData {
    cts_id: Principal,
    cm_main_id: Principal,
    cm_caller: Principal,
    icrc1_token_ledger: Principal,
    icrc1_token_ledger_transfer_fee: Tokens,
    positions_id_counter: u128,
    trade_logs_id_counter: u128,
    mid_call_user_token_balance_locks: HashSet<Principal>,
    cycles_positions: Vec<CyclesPosition>,
    token_positions: Vec<TokenPosition>,
    trade_logs: Vec<TradeLog>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    void_token_positions: Vec<VoidTokenPosition>,
    do_payouts_errors: Vec<CallError>,
    trade_log_storage_canisters: Vec<TradeLogStorageCanisterData>,
    #[serde(with = "serde_bytes")]
    trade_log_storage_buffer: Vec<u8>,
    trade_log_storage_flush_lock: bool,
    create_trade_log_storage_canister_temp_holder: Option<Principal>,
    flush_trade_log_storage_errors: Vec<(FlushTradeLogStorageError, u64/*timestamp_nanos*/)>,
    trade_log_storage_canister_code: CanisterCode,
}

impl CMData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            cm_main_id: Principal::from_slice(&[]),
            cm_caller: Principal::from_slice(&[]),
            icrc1_token_ledger: Principal::from_slice(&[]),
            icrc1_token_ledger_transfer_fee: 0,
            positions_id_counter: 0,
            trade_logs_id_counter: 0,
            mid_call_user_token_balance_locks: HashSet::new(),
            cycles_positions: Vec::new(),
            token_positions: Vec::new(),
            trade_logs: Vec::new(),
            void_cycles_positions: Vec::new(),
            void_token_positions: Vec::new(),
            do_payouts_errors: Vec::new(),
            trade_log_storage_canisters: Vec::new(),
            trade_log_storage_buffer: Vec::new(),
            trade_log_storage_flush_lock: false,
            create_trade_log_storage_canister_temp_holder: None,
            flush_trade_log_storage_errors: Vec::new(),
            trade_log_storage_canister_code: CanisterCode::empty(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TradeLogStorageCanisterData {
    log_size: u32,
    first_log_id: u128,
    length: u64, // number of logs current store on this storage canister
    is_full: bool,
    canister_id: Principal,
    creation_timestamp: u128, // set once when storage canister is create.
    module_hash: [u8; 32] // update this field when upgrading the storage canisters.
}






pub const CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;

pub const TRANSFER_TOKEN_BALANCE_FEE: Cycles = 50_000_000_000;

pub const CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

pub const MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK: u128 = NANOS_IN_A_SECOND * SECONDS_IN_AN_HOUR * 72;
pub const VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS: u128 = SECONDS_IN_AN_HOUR * 1;


pub const MINIMUM_CYCLES_POSITION: Cycles = 1_000_000_000_000;

pub const MINIMUM_TOKEN_POSITION: Tokens = 1;

#[allow(non_upper_case_globals)]
mod memory_location {
    use crate::*;
    
    pub const CANISTER_NETWORK_MEMORY_ALLOCATION_MiB: usize = 500; // multiple of 10
    pub const CANISTER_DATA_STORAGE_SIZE_MiB: usize = CANISTER_NETWORK_MEMORY_ALLOCATION_MiB / 2 - 20/*memory-size at the start [re]placement*/; 

    pub const CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
    pub const MAX_CYCLES_POSITIONS: usize = CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<CyclesPosition>();

    pub const TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
    pub const MAX_TOKEN_POSITIONS: usize = TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<TokenPosition>();

    pub const TRADE_LOGS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 2;
    pub const MAX_TRADE_LOGS: usize = TRADE_LOGS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<TradeLog>();

    pub const VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
    pub const MAX_VOID_CYCLES_POSITIONS: usize = VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidCyclesPosition>();

    pub const VOID_TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
    pub const MAX_VOID_TOKEN_POSITIONS: usize = VOID_TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidTokenPosition>();
}
use memory_location::*;

const DO_VOID_CYCLES_POSITIONS_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 5;
const DO_VOID_TOKEN_POSITIONS_TOKEN_PAYOUTS_CHUNK_SIZE: usize = 5;
const DO_TRADE_LOGS_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_TRADE_LOGS_TOKEN_PAYOUTS_CHUNK_SIZE: usize = 10;


const CM_MESSAGE_METHOD_VOID_CYCLES_POSITION_POSITOR: &'static str       = "cm_message_void_cycles_position_positor";
const CM_MESSAGE_METHOD_VOID_TOKEN_POSITION_POSITOR: &'static str          = "cm_message_void_token_position_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR: &'static str   = "cm_message_cycles_position_purchase_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_cycles_position_purchase_purchaser";
const CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_POSITOR: &'static str      = "cm_message_token_position_purchase_positor";
const CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_PURCHASER: &'static str    = "cm_message_token_position_purchase_purchaser";

const CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_cycles_position_purchase_purchaser_cmcaller_callback";
const CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_POSITOR: &'static str = "cm_message_cycles_position_purchase_positor_cmcaller_callback";
const CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_token_position_purchase_purchaser_cmcaller_callback";
const CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_POSITOR: &'static str = "cm_message_token_position_purchase_positor_cmcaller_callback";
const CMCALLER_CALLBACK_VOID_CYCLES_POSITION_POSITOR: &'static str = "cm_message_void_cycles_position_positor_cmcaller_callback";
const CMCALLER_CALLBACK_VOID_TOKEN_POSITION_POSITOR: &'static str = "cm_message_void_token_position_positor_cmcaller_callback";



const TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO: &[u8; 8] = b"CM-IPP-0";
const CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO: &[u8; 8] = b"CM-CPP-0";

const TRANSFER_TOKEN_BALANCE_MEMO: &[u8; 8] = b"CMTRNSFR";

const MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS: usize = 500;

const FLUSH_TRADE_LOGS_STORAGE_BUFFER_AT_SIZE: usize = 1 * MiB; // can make this bigger 5 or 10 MiB, the flush logic handles flush chunks.

const FLUSH_TRADE_LOGS_STORAGE_BUFFER_CHUNK_SIZE: usize = {
    let before_modulo = 1*MiB+512*KiB; 
    before_modulo - (before_modulo % TradeLog::STABLE_MEMORY_SERIALIZE_SIZE)
};

const STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION: MemoryId = MemoryId::new(0);

const CREATE_TRADE_LOG_STORAGE_CANISTER_CYCLES: Cycles = 10_000_000_000_000;



// think bout the position matches, maker is the position-positor, taker is the position-purchaser,
// perhaps log each trade as a whole, 

/*

struct TradeLog {
    positor: Principal, //maker
    purchaser: Principal, //taker
    tokens: Tokens,
    cycles: Cycles,
    rate: CyclesPerTokenRate,
    //but then how do we know whether the position is a cycles-position or a token-position & whether this is a cycles-position-purchase or a token-position-purchase?
    position_kind: PositionKind
}

enum PositionKind {
    Cycles,
    Token
}

*/




thread_local! {
    
    static CM_DATA: RefCell<CMData> = RefCell::new(CMData::new()); 
    
    // not save through the upgrades
    static TOKEN_LEDGER_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static TOKEN_LEDGER_TRANSFER_FEE: Cell<Tokens> = Cell::new(0);
    static STOP_CALLS: Cell<bool> = Cell::new(false);   
}


// ------------------ INIT ----------------------

#[init]
fn init(cm_init: CMIcrc1TokenTradeContractInit) {
    stable_memory_tools::init(&CM_DATA, STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION);

    with_mut(&CM_DATA, |cm_data| { 
        cm_data.cts_id = cm_init.cts_id; 
        cm_data.cm_main_id = cm_init.cm_main_id; 
        cm_data.cm_caller = cm_init.cm_caller;
        cm_data.icrc1_token_ledger = cm_init.icrc1_token_ledger; 
        cm_data.icrc1_token_ledger_transfer_fee = cm_init.icrc1_token_ledger_transfer_fee;
        cm_data.trade_log_storage_canister_code = cm_init.trade_log_storage_canister_code;
    });
    
    localkey::cell::set(&TOKEN_LEDGER_ID, cm_init.icrc1_token_ledger);
    localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, cm_init.icrc1_token_ledger_transfer_fee);
} 

// ------------------ UPGRADES ------------------------

#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::post_upgrade(&CM_DATA, STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION, None::<fn(OldCMData) -> CMData>);
    
    with(&CM_DATA, |cm_data| {
        localkey::cell::set(&TOKEN_LEDGER_ID, cm_data.icrc1_token_ledger);
        localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, cm_data.icrc1_token_ledger_transfer_fee);
    });
    
    // ---------
    
    // when this token_trade_contract canister is upgrade, we stop the canister first then upgrade then start the canister. 
    // if the cm_caller tries to name-call-back this canister it might be between after it stopped and before it started.
    // so therefore after upgrade, call the cm_caller controller_do_try_callbacks to push through the name-call-backs.
    // 2-minutes to make sure the cm_caller gets back it's system callback - which logs the try-name-callback result - from it's name-call-back call try.
    //set_timer(2-minutes, call cm_caller controller_do_try_callbacks)
    ic_cdk_timers::set_timer(Duration::from_secs(120), || ic_cdk::spawn(call_cm_caller_do_try_callbacks()));
}

async fn call_cm_caller_do_try_callbacks() {
    match call_raw128(
        with(&CM_DATA, |cm_data| { cm_data.cm_caller }),
        "controller_do_try_callbacks",
        &[68, 73, 68, 76, 0, 0],
        0
    ).await {
        Ok(_) => {
            // can decode Vec<TryCallback> see if there are leftovers
            // if leftovers can set the timer
        },
        Err(_call_error) => {
            // can set the timer if need
        }
    };
}


// -------------------------------------------------------------

#[no_mangle]
fn canister_inspect_message() {
    use cts_lib::ic_cdk::api::call::{method_name, accept_message};
    
    if [
        "create_cycles_position",
        "create_token_position",
        "purchase_cycles_position",
        "purchase_token_position",
        "void_position",
        "see_token_lock",
        "transfer_token_balance",
        "cm_message_cycles_position_purchase_purchaser_cmcaller_callback",
        "cm_message_cycles_position_purchase_positor_cmcaller_callback",
        "cm_message_token_position_purchase_purchaser_cmcaller_callback",
        "cm_message_token_position_purchase_positor_cmcaller_callback",
        "cm_message_void_cycles_position_positor_cmcaller_callback",
        "cm_message_void_token_position_positor_cmcaller_callback",
        "see_cycles_positions",
        "see_token_positions",
        "see_cycles_positions_purchases",
        "see_token_positions_purchases",
        "download_cycles_positions_rchunks",
        "download_token_positions_rchunks",
        "download_cycles_positions_purchases_rchunks",
        "download_token_positions_purchases_rchunks",
        "see_trade_logs",
    ].contains(&&method_name()[..]) {
        trap("this method must be call by a canister or a query call.");
    }
    
    
    accept_message();    
}


// -------------------------------------------------------------


fn new_id(cm_data_id_counter: &mut u128) -> u128 {
    let id: u128 = cm_data_id_counter.clone();
    *(cm_data_id_counter) += 1;
    id
}


async fn token_transfer(q: TokenTransferArg) -> Result<Result<BlockId, TokenTransferError>, CallError> {
    icrc1_transfer(localkey::cell::get(&TOKEN_LEDGER_ID), q).await
}

async fn token_balance(count_id: IcrcId) -> Result<Tokens, CallError> {
    icrc1_balance_of(localkey::cell::get(&TOKEN_LEDGER_ID), count_id).await
}




async fn check_user_cycles_market_token_ledger_balance(user_id: &Principal) -> Result<Tokens, CallError> {
    token_balance(
        IcrcId{
            owner: ic_cdk::api::id(),
            subaccount: Some(principal_token_subaccount(user_id))
        }
    ).await
}


fn check_user_token_balance_in_the_lock(cm_data: &CMData, user_id: &Principal) -> Tokens {
    cm_data.token_positions.iter()
        .filter(|token_position: &&TokenPosition| { token_position.positor == *user_id })
        .fold(0, |cummulator: Tokens, user_token_position: &TokenPosition| {
            cummulator + user_token_position.tokens + ( user_token_position.tokens / user_token_position.minimum_purchase * localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) )
        })
    +
    cm_data.trade_logs.iter()
        .filter(|tl: &&TradeLog| {
            tl.token_payout_payor() == *user_id && tl.token_payout_data.token_transfer.is_none() 
        })
        .fold(0, |cummulator: Tokens, user_trade_log_with_unpaid_tokens: &TradeLog| {
            cummulator + user_trade_log_with_unpaid_tokens.tokens + localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE)
        })
}




// ---------------


#[derive(Serialize, Deserialize)]
pub enum FlushTradeLogStorageError {
    CreateTradeLogStorageCanisterError(CreateTradeLogStorageCanisterError),
    TradeLogStorageCanisterCallError(CallError),
    NewTradeLogStorageCanisterIsFull, // when a *new* trade-log-storage-canister returns StorageIsFull on the first flush call. 
}


#[derive(Serialize, Deserialize)]
pub enum CreateTradeLogStorageCanisterError {
    CreateCanisterCallError(CallError),
    InstallCodeCandidError(String),
    InstallCodeCallError(CallError),
}

async fn create_trade_log_storage_canister() -> Result<Principal/*saves the trade-log-storage-canister-data in the CM_DATA*/, CreateTradeLogStorageCanisterError> {
    use management_canister::*;
    
    
    let canister_id: Principal = match with_mut(&CM_DATA, |data| { data.create_trade_log_storage_canister_temp_holder.take() }) {
        Some(canister_id) => canister_id,
        None => {
            match call_with_payment128::<(ManagementCanisterCreateCanisterQuest,), (CanisterIdRecord,)>(
                Principal::management_canister(),
                "create_canister",
                (ManagementCanisterCreateCanisterQuest{
                    settings: None,
                },),
                CREATE_TRADE_LOG_STORAGE_CANISTER_CYCLES, // cycles for the canister
            ).await {
                Ok(r) => r.0.canister_id,
                Err(call_error) => {
                    return Err(CreateTradeLogStorageCanisterError::CreateCanisterCallError(call_error_as_u32_and_string(call_error)));
                }
            }
        }
    };
    
    let mut module_hash: [u8; 32] = [0; 32]; // can't initalize an immutable variable from within a closure because the closure borrows it.
    
    match with(&CM_DATA, |data| {
        module_hash = data.trade_log_storage_canister_code.module_hash().clone();
        
        Ok(call_raw128(
            Principal::management_canister(),
            "install_code",
            &encode_one(
                ManagementCanisterInstallCodeQuest{
                    mode : ManagementCanisterInstallCodeMode::install,
                    canister_id : canister_id,
                    wasm_module : data.trade_log_storage_canister_code.module(),
                    arg : &encode_one(
                        Icrc1TokenTradeLogStorageInit{
                            log_size: TradeLog::STABLE_MEMORY_SERIALIZE_SIZE as u32,
                        }
                    ).map_err(|e| { CreateTradeLogStorageCanisterError::InstallCodeCandidError(format!("{:?}", e)) })?,
                }
            ).map_err(|e| { CreateTradeLogStorageCanisterError::InstallCodeCandidError(format!("{:?}", e)) })?,    
            0
        ))
        
    })?.await {
        Ok(_) => {
            with_mut(&CM_DATA, |data| {
                data.trade_log_storage_canisters.push(
                    TradeLogStorageCanisterData {
                        log_size: TradeLog::STABLE_MEMORY_SERIALIZE_SIZE as u32,
                        first_log_id: data.trade_log_storage_canisters.last().map(|c| c.first_log_id + c.length as u128).unwrap_or(0),
                        length: 0,
                        is_full: false,
                        canister_id: canister_id,
                        creation_timestamp: time_nanos(),
                        module_hash,
                    }
                );
            });
            Ok(canister_id)
        }
        Err(install_code_call_error) => {
            with_mut(&CM_DATA, |data| { data.create_trade_log_storage_canister_temp_holder = Some(canister_id); });
            return Err(CreateTradeLogStorageCanisterError::InstallCodeCallError(call_error_as_u32_and_string(install_code_call_error)));
        }
    }
    
}



async fn do_payouts() {
    
    if with(&CM_DATA, |cm_data| {
        cm_data.void_cycles_positions.len() == 0
        && cm_data.void_token_positions.len() == 0
        && cm_data.trade_logs.len() == 0
    }) { return; }

    match call::<(),()>(
        ic_cdk::api::id(),
        "do_payouts_public_method",
        (),
    ).await {
        Ok(()) => {
                    
            with_mut(&CM_DATA, |cm_data| {
                while cm_data.trade_logs.len() > 0 {
                    if cm_data.trade_logs[0].can_move_into_the_stable_memory_for_the_long_term_storage() == true {
                        cm_data.trade_log_storage_buffer.extend(cm_data.trade_logs.remove(0).into_stable_memory_serialize());
                    } else {
                        break; // bc want to save into the stable-memory in the correct sequence.
                    }
                }
                
                if cm_data.trade_log_storage_buffer.len() >= FLUSH_TRADE_LOGS_STORAGE_BUFFER_AT_SIZE 
                && cm_data.trade_log_storage_flush_lock == false {
                    cm_data.trade_log_storage_flush_lock = true;
                }
            });
            
            if with(&CM_DATA, |cm_data| { cm_data.trade_log_storage_flush_lock == true }) {
                
                let trade_log_storage_canister_id: Principal = {
                    match with(&CM_DATA, |data| { 
                        data.trade_log_storage_canisters
                            .last()
                            .and_then(|storage_canister| { 
                                if storage_canister.is_full { None } else { Some(storage_canister.canister_id) }
                            })
                    }) {
                        Some(c_id) => c_id,
                        None => {
                            match create_trade_log_storage_canister().await {
                                Ok(p) => p,
                                Err(e) => {
                                    with_mut(&CM_DATA, |data| {
                                        data.trade_log_storage_flush_lock = false;
                                        data.flush_trade_log_storage_errors.push((FlushTradeLogStorageError::CreateTradeLogStorageCanisterError(e), time_nanos_u64()));
                                    });
                                    return;
                                }
                            }
                        }
                    }
                };
                
                let chunk_sizes: Vec<usize>/*vec len is the num_of_chunks*/ = with(&CM_DATA, |cm_data| {
                    cm_data.trade_log_storage_buffer.chunks(FLUSH_TRADE_LOGS_STORAGE_BUFFER_CHUNK_SIZE).map(|c| c.len()).collect::<Vec<usize>>()
                });
                
                for chunk_size in chunk_sizes.into_iter() {

                    let chunk_future = with(&CM_DATA, |cm_data| {
                        call_raw128( // <(FlushQuestForward,), (FlushResult,)>
                            trade_log_storage_canister_id,
                            "flush",
                            &encode_one(&
                                FlushQuestForward{
                                    bytes: Bytes::new(&cm_data.trade_log_storage_buffer[..chunk_size]),
                                }
                            ).unwrap(),
                            10_000_000_000 // put some cycles for the trade-log-storage-canister
                        )
                    });
                    
                    match chunk_future.await {
                        Ok(sb) => match decode_one::<FlushResult>(&sb).unwrap() {
                            Ok(_flush_success) => {
                                with_mut(&CM_DATA, |cm_data| {
                                    cm_data.trade_log_storage_canisters.last_mut().unwrap().length += (chunk_size / TradeLog::STABLE_MEMORY_SERIALIZE_SIZE) as u64;
                                    cm_data.trade_log_storage_buffer.drain(..chunk_size);
                                });
                            },
                            Err(flush_error) => match flush_error {
                                FlushError::StorageIsFull => {
                                    with_mut(&CM_DATA, |cm_data| {
                                        cm_data.trade_log_storage_canisters.last_mut().unwrap().is_full = true;
                                    });
                                    break;
                                }
                            }
                        }
                        Err(flush_call_error) => {
                            with_mut(&CM_DATA, |data| {
                                data.flush_trade_log_storage_errors.push((FlushTradeLogStorageError::TradeLogStorageCanisterCallError(call_error_as_u32_and_string(flush_call_error)), time_nanos_u64()));
                            });
                            break;
                        }
                    }
                }

                with_mut(&CM_DATA, |data| {
                    data.trade_log_storage_flush_lock = false;
                });
            }
            
        },
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| {
                cm_data.do_payouts_errors.push(call_error_as_u32_and_string(call_error));
            });
        }
    }
}

#[update]
pub async fn do_payouts_public_method() {
    if [ic_cdk::api::id(), with(&CM_DATA, |cm_data| { cm_data.cts_id })].contains(&caller()) == false {
        trap("caller without the authorization.");
    }
    
    _do_payouts().await;

}


// -------------------------------------------------------------
















// -------------------------------------------------------------


type CreateCyclesPositionResult = Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>;


#[update(manual_reply = true)]
pub async fn create_cycles_position(q: CreateCyclesPositionQuest) { // -> CreateCyclesPositionResult {

    let positor: Principal = caller();

    let r: CreateCyclesPositionResult = create_cycles_position_(positor, q).await;
    
    reply::<(CreateCyclesPositionResult,)>((r,));
    
    do_payouts().await;
    return;
}


async fn create_cycles_position_(positor: Principal, q: CreateCyclesPositionQuest) -> CreateCyclesPositionResult {

    if q.minimum_purchase > q.cycles {
        return Err(CreateCyclesPositionError::MinimumPurchaseMustBeEqualOrLessThanTheCyclesPosition);
    }

    if q.cycles < MINIMUM_CYCLES_POSITION {
        return Err(CreateCyclesPositionError::MinimumCyclesPosition(MINIMUM_CYCLES_POSITION));
    }
    
    if q.minimum_purchase == 0 {
        return Err(CreateCyclesPositionError::MinimumPurchaseCannotBeZero);
    }

    if q.cycles % q.cycles_per_token_rate != 0 {
        return Err(CreateCyclesPositionError::CyclesMustBeAMultipleOfTheCyclesPerTokenRate);
    }

    if q.minimum_purchase % q.cycles_per_token_rate != 0 {
        return Err(CreateCyclesPositionError::MinimumPurchaseMustBeAMultipleOfTheCyclesPerTokenRate);
    }

    let msg_cycles_quirement: Cycles = CREATE_POSITION_FEE.saturating_add(q.cycles); 

    if msg_cycles_available128() < msg_cycles_quirement {
        return Err(CreateCyclesPositionError::MsgCyclesTooLow{ create_position_fee: CREATE_POSITION_FEE });
    }

    if canister_balance128().checked_add(msg_cycles_quirement).is_none() {
        return Err(CreateCyclesPositionError::CyclesMarketIsFull);
    }

    
    with_mut(&CM_DATA, |cm_data| {
        if cm_data.cycles_positions.len() >= MAX_CYCLES_POSITIONS {
            if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                return Err(CreateCyclesPositionError::CyclesMarketIsBusy);
            }
            // new
            // :highest-cost-position of the cycles-positions.
            let (
                cycles_position_with_the_lowest_cycles_per_token_rate_cycles_positions_i,
                cycles_position_with_the_lowest_cycles_per_token_rate_ref
            ): (usize, &CyclesPosition) = { 
                cm_data.cycles_positions.iter()
                    .enumerate()
                    .min_by_key(|(_,cycles_position): &(usize,&CyclesPosition)| { cycles_position.cycles_per_token_rate })
                    .unwrap()
            };
            if q.cycles_per_token_rate > cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles_per_token_rate 
            && q.cycles >= cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles {
                // bump
                
                std::mem::drop(cycles_position_with_the_lowest_cycles_per_token_rate_ref);
                
                let cycles_position_lowest_cycles_per_token_rate: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_with_the_lowest_cycles_per_token_rate_cycles_positions_i);
                
                let cycles_position_lowest_cycles_per_token_rate_void_cycles_positions_insertion_i = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_cycles_per_token_rate.id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_lowest_cycles_per_token_rate_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_lowest_cycles_per_token_rate.id,
                        positor:        cycles_position_lowest_cycles_per_token_rate.positor,
                        cycles:         cycles_position_lowest_cycles_per_token_rate.cycles,
                        cycles_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time_nanos()
                    }
                );
                Ok(())
            } else {
                return Err(CreateCyclesPositionError::CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles_per_token_rate + 1, minimum_cycles_position_for_a_bump: cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles });
            }
        } else {
            Ok(())
        }
    })?;
    
    let position_id: PositionId = with_mut(&CM_DATA, |cm_data| {
        let id: PositionId = new_id(&mut cm_data.positions_id_counter); 
        cm_data.cycles_positions.push(
            CyclesPosition{
                id,   
                positor,
                cycles: q.cycles,
                minimum_purchase: q.minimum_purchase,
                cycles_per_token_rate: q.cycles_per_token_rate,
                timestamp_nanos: time_nanos(),
            }
        );
        id
    });
    
    msg_cycles_accept128(msg_cycles_quirement);

    Ok(CreateCyclesPositionSuccess{
        position_id
    })
    
}





// ------------------


type CreateTokenPositionResult = Result<CreateTokenPositionSuccess, CreateTokenPositionError>;


#[update(manual_reply = true)]
pub async fn create_token_position(q: CreateTokenPositionQuest) { // -> CreateTokenPositionResult {

    let positor: Principal = caller();

    let r: CreateTokenPositionResult = create_token_position_(positor, q).await;
    
    reply::<(CreateTokenPositionResult,)>((r,));
    
    do_payouts().await;
    return;

}

async fn create_token_position_(positor: Principal, q: CreateTokenPositionQuest) -> CreateTokenPositionResult {

    if q.minimum_purchase > q.tokens {
        return Err(CreateTokenPositionError::MinimumPurchaseMustBeEqualOrLessThanTheTokenPosition);
    }

    if q.tokens < MINIMUM_TOKEN_POSITION {
        return Err(CreateTokenPositionError::MinimumTokenPosition(MINIMUM_TOKEN_POSITION));
    }
    
    if q.minimum_purchase == 0 {
        return Err(CreateTokenPositionError::MinimumPurchaseCannotBeZero);
    }


    let msg_cycles_quirement: Cycles = CREATE_POSITION_FEE; 

    if msg_cycles_available128() < msg_cycles_quirement {
        return Err(CreateTokenPositionError::MsgCyclesTooLow{ create_position_fee: CREATE_POSITION_FEE  });
    }

    if canister_balance128().checked_add(msg_cycles_quirement).is_none() {
        return Err(CreateTokenPositionError::CyclesMarketIsFull);
    }

    
    with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_token_balance_locks.len() >= MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS {
            return Err(CreateTokenPositionError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_token_balance_locks.contains(&positor) {
            return Err(CreateTokenPositionError::CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall);
        }
        cm_data.mid_call_user_token_balance_locks.insert(positor);
        Ok(())
    })?;
    
    // check token balance and make sure to unlock the user on returns after here 
    let user_token_ledger_balance: Tokens = match check_user_cycles_market_token_ledger_balance(&positor).await {
        Ok(token_ledger_balance) => token_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&positor); });
            return Err(CreateTokenPositionError::CheckUserCyclesMarketTokenLedgerBalanceError((call_error.0 as u32, call_error.1)));
        }
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &positor) });
    
    let usable_user_token_balance: Tokens = user_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock);
    
    if usable_user_token_balance < q.tokens + ( q.tokens / q.minimum_purchase * localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) ) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&positor); });
        return Err(CreateTokenPositionError::UserTokenBalanceTooLow{ user_token_balance: usable_user_token_balance });
    }
    
    
    with_mut(&CM_DATA, |cm_data| {
        if cm_data.token_positions.len() >= MAX_TOKEN_POSITIONS {            
            if cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS {
                cm_data.mid_call_user_token_balance_locks.remove(&positor);
                return Err(CreateTokenPositionError::CyclesMarketIsBusy);
            }
            // new
            let (
                token_position_with_the_highest_cycles_per_token_rate_token_positions_i,
                token_position_with_the_highest_cycles_per_token_rate_ref
            ): (usize, &TokenPosition) = {
                cm_data.token_positions.iter()
                    .enumerate()
                    .max_by_key(|(_, token_position): &(usize, &TokenPosition)| { token_position.cycles_per_token_rate })
                    .unwrap()
            };
            if q.cycles_per_token_rate < token_position_with_the_highest_cycles_per_token_rate_ref.cycles_per_token_rate 
            && q.tokens >= token_position_with_the_highest_cycles_per_token_rate_ref.tokens {
                // bump
                
                std::mem::drop(token_position_with_the_highest_cycles_per_token_rate_ref);
                
                let token_position_highest_cycles_per_token_rate: TokenPosition = cm_data.token_positions.remove(token_position_with_the_highest_cycles_per_token_rate_token_positions_i);                
                
                let token_position_highest_cycles_per_token_rate_void_token_positions_insertion_i = { 
                    cm_data.void_token_positions.binary_search_by_key(
                        &token_position_highest_cycles_per_token_rate.id,
                        |void_token_position| { void_token_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_token_positions.insert(
                    token_position_highest_cycles_per_token_rate_void_token_positions_insertion_i,
                    VoidTokenPosition{                        
                        position_id:    token_position_highest_cycles_per_token_rate.id,
                        positor:        token_position_highest_cycles_per_token_rate.positor,
                        tokens:         token_position_highest_cycles_per_token_rate.tokens,
                        timestamp_nanos: time_nanos(),
                        token_payout_lock: false,
                        token_payout_data: TokenPayoutData{
                            token_transfer: Some(TokenTransferBlockHeightAndTimestampNanos{
                                block_height: None,
                                timestamp_nanos: time_nanos(),
                            }),
                            cm_message_call_success_timestamp_nanos: None,
                            cm_message_callback_complete: None            
                        }
                    }
                ); 
                Ok(())
            } else {
                cm_data.mid_call_user_token_balance_locks.remove(&positor);
                return Err(CreateTokenPositionError::CyclesMarketIsFull_MaximumRateAndMinimumTokenPositionForABump{ maximum_rate_for_a_bump: token_position_with_the_highest_cycles_per_token_rate_ref.cycles_per_token_rate - 1, minimum_token_position_for_a_bump: token_position_with_the_highest_cycles_per_token_rate_ref.tokens });
            }
        } else {
            Ok(())    
        }
    })?;
    
    let position_id: PositionId = with_mut(&CM_DATA, |cm_data| {
        let id: PositionId = new_id(&mut cm_data.positions_id_counter); 
        cm_data.token_positions.push(
            TokenPosition{
                id,   
                positor,
                tokens: q.tokens,
                minimum_purchase: q.minimum_purchase,
                cycles_per_token_rate: q.cycles_per_token_rate,
                timestamp_nanos: time_nanos(),
            }
        );
        id
    });
    
    msg_cycles_accept128(msg_cycles_quirement);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&positor); });
    
    Ok(CreateTokenPositionSuccess{
        position_id
    })
}


// ------------------


type PurchaseCyclesPositionResult = Result<PurchaseCyclesPositionSuccess, PurchaseCyclesPositionError>;


#[update(manual_reply = true)]
pub async fn purchase_cycles_position(q: PurchaseCyclesPositionQuest) { // -> PurchaseCyclesPositionResult
    
    let purchaser: Principal = caller();
    
    let r: PurchaseCyclesPositionResult = purchase_cycles_position_(purchaser, q).await;
    
    reply::<(PurchaseCyclesPositionResult,)>((r,));

    do_payouts().await;
    return;
}

async fn purchase_cycles_position_(purchaser: Principal, q: PurchaseCyclesPositionQuest) -> PurchaseCyclesPositionResult {

    if msg_cycles_available128() < PURCHASE_POSITION_FEE {
        return Err(PurchaseCyclesPositionError::MsgCyclesTooLow{ purchase_position_fee: PURCHASE_POSITION_FEE });
    }
    
    with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_token_balance_locks.len() >= MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_token_balance_locks.contains(&purchaser) {
            return Err(PurchaseCyclesPositionError::CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall);
        }
        cm_data.mid_call_user_token_balance_locks.insert(purchaser);
        Ok(())
    })?;
    
    // check token balance and make sure to unlock the user on returns after here 
    let user_token_ledger_balance: Tokens = match check_user_cycles_market_token_ledger_balance(&purchaser).await {
        Ok(token_ledger_balance) => token_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&purchaser); });
            return Err(PurchaseCyclesPositionError::CheckUserCyclesMarketTokenLedgerBalanceError((call_error.0 as u32, call_error.1)));            
        }
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &purchaser) });
    
    let usable_user_token_balance: Tokens = user_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock);

    let cycles_position_purchase_id: PurchaseId = match with_mut(&CM_DATA, |cm_data| {
        if cm_data.trade_logs.len() >= MAX_TRADE_LOGS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
        let cycles_position_cycles_positions_i: usize = match cm_data.cycles_positions.binary_search_by_key(
            &q.cycles_position_id,
            |cycles_position| { cycles_position.id }
        ) {
            Ok(i) => i,
            Err(_) => { return Err(PurchaseCyclesPositionError::CyclesPositionNotFound); }
        };
        let cycles_position_ref: &CyclesPosition = &cm_data.cycles_positions[cycles_position_cycles_positions_i];
        if q.cycles % cycles_position_ref.cycles_per_token_rate as u128 != 0 {
            return Err(PurchaseCyclesPositionError::PurchaseCyclesMustBeAMultipleOfTheCyclesPerTokenRate);
        }
        if cycles_position_ref.cycles < q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: cycles_position_ref.cycles });
        }
        if cycles_position_ref.minimum_purchase > q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: cycles_position_ref.minimum_purchase });
        }        
        
        if usable_user_token_balance < cycles_transform_tokens(q.cycles, cycles_position_ref.cycles_per_token_rate) + localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) {
            return Err(PurchaseCyclesPositionError::UserTokenBalanceTooLow{ user_token_balance: usable_user_token_balance });
        }
        
        if cycles_position_ref.cycles - q.cycles < cycles_position_ref.minimum_purchase 
        && cycles_position_ref.cycles - q.cycles != 0
        && cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
                
        let cycles_position_purchase_id: PurchaseId = new_id(&mut cm_data.trade_logs_id_counter);
        cm_data.trade_logs.push(
            TradeLog{
                position_id: cycles_position_ref.id,
                id: cycles_position_purchase_id,
                positor: cycles_position_ref.positor, 
                purchaser, 
                tokens: cycles_transform_tokens(q.cycles, cycles_position_ref.cycles_per_token_rate),
                cycles: q.cycles,
                cycles_per_token_rate: cycles_position_ref.cycles_per_token_rate,
                position_kind: PositionKind::Cycles,
                timestamp_nanos: time_nanos(),
                cycles_payout_lock: false,
                token_payout_lock: false,
                cycles_payout_data: CyclesPayoutData::new(),
                token_payout_data: TokenPayoutData{
                    token_transfer: None,
                    cm_message_call_success_timestamp_nanos: None,
                    cm_message_callback_complete: None    
                }
            }
        );

        std::mem::drop(cycles_position_ref);
        cm_data.cycles_positions[cycles_position_cycles_positions_i].cycles -= q.cycles;
        if cm_data.cycles_positions[cycles_position_cycles_positions_i].cycles < cm_data.cycles_positions[cycles_position_cycles_positions_i].minimum_purchase {
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_cycles_positions_i);
            if cycles_position_for_the_void.cycles != 0 {
                let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_for_the_void.id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_for_the_void_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_for_the_void.id,
                        positor:        cycles_position_for_the_void.positor,
                        cycles:         cycles_position_for_the_void.cycles,
                        cycles_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time_nanos()
                    }
                );
            }
        }   
        
        Ok(cycles_position_purchase_id)
    }) {
        Ok(cycles_position_purchase_id) => cycles_position_purchase_id,
        Err(purchase_cycles_position_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&purchaser); });
            return Err(purchase_cycles_position_error);
        }
    };
    
    msg_cycles_accept128(PURCHASE_POSITION_FEE);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&purchaser); });
    
    Ok(PurchaseCyclesPositionSuccess{
        purchase_id: cycles_position_purchase_id
    })

}



// -------------------



type PurchaseTokenPositionResult = Result<PurchaseTokenPositionSuccess, PurchaseTokenPositionError>;


#[update(manual_reply = true)]
pub async fn purchase_token_position(q: PurchaseTokenPositionQuest) { // -> PurchaseTokenPositionResult 

    let purchaser: Principal = caller();
    
    let r: PurchaseTokenPositionResult = purchase_token_position_(purchaser, q).await;

    reply::<(PurchaseTokenPositionResult,)>((r,));
    
    do_payouts().await;
    return;
}

async fn purchase_token_position_(purchaser: Principal, q: PurchaseTokenPositionQuest) -> PurchaseTokenPositionResult {

    let token_position_purchase_id: PurchaseId = with_mut(&CM_DATA, |cm_data| {
        if cm_data.trade_logs.len() >= MAX_TRADE_LOGS {
            return Err(PurchaseTokenPositionError::CyclesMarketIsBusy);            
        }
        let token_position_token_positions_i: usize = match cm_data.token_positions.binary_search_by_key(
            &q.token_position_id,
            |token_position| { token_position.id }
        ) {
            Ok(i) => i,
            Err(_) => { return Err(PurchaseTokenPositionError::TokenPositionNotFound); }
        };
        let token_position_ref: &TokenPosition = &cm_data.token_positions[token_position_token_positions_i];
        if token_position_ref.tokens < q.tokens {
            return Err(PurchaseTokenPositionError::TokenPositionTokensIsLessThanThePurchaseQuest{ token_position_tokens: token_position_ref.tokens.clone() });
        }
        if token_position_ref.minimum_purchase > q.tokens {
            return Err(PurchaseTokenPositionError::TokenPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ token_position_minimum_purchase: token_position_ref.minimum_purchase.clone() });
        }        

        if &token_position_ref.tokens - &q.tokens < token_position_ref.minimum_purchase 
        && token_position_ref.tokens - q.tokens != 0
        && cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS {
            return Err(PurchaseTokenPositionError::CyclesMarketIsBusy);
        }
        

        let msg_cycles_quirement: Cycles = PURCHASE_POSITION_FEE + tokens_transform_cycles(q.tokens, token_position_ref.cycles_per_token_rate); 
        if msg_cycles_available128() < msg_cycles_quirement {
            return Err(PurchaseTokenPositionError::MsgCyclesTooLow{ purchase_position_fee: PURCHASE_POSITION_FEE });
        }
        msg_cycles_accept128(msg_cycles_quirement);
                
        let token_position_purchase_id: PurchaseId = new_id(&mut cm_data.trade_logs_id_counter);
        
        cm_data.trade_logs.push(
            TradeLog{
                position_id: token_position_ref.id,
                id: token_position_purchase_id,
                positor: token_position_ref.positor,
                purchaser,
                tokens: q.tokens,
                cycles: tokens_transform_cycles(q.tokens, token_position_ref.cycles_per_token_rate),
                cycles_per_token_rate: token_position_ref.cycles_per_token_rate,
                position_kind: PositionKind::Token,
                timestamp_nanos: time_nanos(),
                cycles_payout_lock: false,
                token_payout_lock: false,
                cycles_payout_data: CyclesPayoutData::new(),
                token_payout_data: TokenPayoutData{
                    token_transfer: None,
                    cm_message_call_success_timestamp_nanos: None,
                    cm_message_callback_complete: None    
                }
            }
        );

        std::mem::drop(token_position_ref);
        cm_data.token_positions[token_position_token_positions_i].tokens -= q.tokens;
        if cm_data.token_positions[token_position_token_positions_i].tokens < cm_data.token_positions[token_position_token_positions_i].minimum_purchase {            
            let token_position_for_the_void: TokenPosition = cm_data.token_positions.remove(token_position_token_positions_i);
            if token_position_for_the_void.tokens != 0 {
                let token_position_for_the_void_void_token_positions_insertion_i: usize = { 
                    cm_data.void_token_positions.binary_search_by_key(
                        &token_position_for_the_void.id,
                        |void_token_position| { void_token_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_token_positions.insert(
                    token_position_for_the_void_void_token_positions_insertion_i,
                    VoidTokenPosition{
                        position_id:    token_position_for_the_void.id,
                        positor:        token_position_for_the_void.positor,
                        tokens:            token_position_for_the_void.tokens,
                        timestamp_nanos: time_nanos(),
                        token_payout_lock: false,
                        token_payout_data: TokenPayoutData{
                            token_transfer: Some(TokenTransferBlockHeightAndTimestampNanos{
                                block_height: None,
                                timestamp_nanos: time_nanos(),
                            }),
                            cm_message_call_success_timestamp_nanos: None,
                            cm_message_callback_complete: None            
                        }
                    }
                );
            }    
            
        }
        
        Ok(token_position_purchase_id)
    })?;
    
    Ok(PurchaseTokenPositionSuccess{
        purchase_id: token_position_purchase_id
    })
    
}




// --------------------------



#[update(manual_reply = true)]
pub async fn void_position(q: VoidPositionQuest) { // -> VoidPositionResult
    let caller: Principal = caller();
    
    let r: VoidPositionResult = void_position_(caller, q);
    
    reply::<(VoidPositionResult,)>((r,));
    
    do_payouts().await;
    return; 

}   
    
fn void_position_(caller: Principal, q: VoidPositionQuest) -> VoidPositionResult {
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(cycles_position_i) = cm_data.cycles_positions.binary_search_by_key(&q.position_id, |cycles_position| { cycles_position.id }) {
            if cm_data.cycles_positions[cycles_position_i].positor != caller {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(cm_data.cycles_positions[cycles_position_i].timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: cm_data.cycles_positions[cycles_position_i].timestamp_nanos/NANOS_IN_A_SECOND });
            }  
            if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                return Err(VoidPositionError::CyclesMarketIsBusy);
            }
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_i);
            let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = cm_data.void_cycles_positions.binary_search_by_key(&cycles_position_for_the_void.id, |vcp| { vcp.position_id }).unwrap_err();
            cm_data.void_cycles_positions.insert(
                cycles_position_for_the_void_void_cycles_positions_insertion_i,
                VoidCyclesPosition{
                    position_id:    cycles_position_for_the_void.id,
                    positor:        cycles_position_for_the_void.positor,
                    cycles:         cycles_position_for_the_void.cycles,
                    cycles_payout_lock: false,
                    cycles_payout_data: CyclesPayoutData::new(),
                    timestamp_nanos: time_nanos()                
                }
            );
            Ok(())
        } else if let Ok(token_position_i) = cm_data.token_positions.binary_search_by_key(&q.position_id, |token_position| { token_position.id }) {
            if cm_data.token_positions[token_position_i].positor != caller {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(cm_data.token_positions[token_position_i].timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: cm_data.token_positions[token_position_i].timestamp_nanos/NANOS_IN_A_SECOND });
            }
            if cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS {
                return Err(VoidPositionError::CyclesMarketIsBusy);
            }
            let token_position_for_the_void: TokenPosition = cm_data.token_positions.remove(token_position_i);
            let token_position_for_the_void_void_token_positions_insertion_i: usize = cm_data.void_token_positions.binary_search_by_key(&token_position_for_the_void.id, |vip| { vip.position_id }).unwrap_err();
            cm_data.void_token_positions.insert(
                token_position_for_the_void_void_token_positions_insertion_i,
                VoidTokenPosition{
                    position_id:    token_position_for_the_void.id,
                    positor:        token_position_for_the_void.positor,
                    tokens:            token_position_for_the_void.tokens,
                    timestamp_nanos: time_nanos(),
                    token_payout_lock: false,
                    token_payout_data: TokenPayoutData{
                        token_transfer: Some(TokenTransferBlockHeightAndTimestampNanos{
                            block_height: None,
                            timestamp_nanos: time_nanos(),
                        }),
                        cm_message_call_success_timestamp_nanos: None,
                        cm_message_callback_complete: None            
                    }
                }
            );
            Ok(())
        } else {
            return Err(VoidPositionError::PositionNotFound);
        }
    })
    
}


// ----------------

#[derive(CandidType, Deserialize)]
pub struct ViewTokenLockQuest {
    principal_id: Principal,
}

#[query]
pub fn view_token_lock(q: ViewTokenLockQuest) -> Tokens {
    with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &q.principal_id) })
}


// ----------------


#[update(manual_reply = true)]
pub async fn transfer_token_balance(q: TransferTokenBalanceQuest) { // -> TransferTokenBalanceResult {
    
    let user_id: Principal = caller();
    
    let r: TransferTokenBalanceResult = transfer_token_balance_(user_id, q).await;
    
    reply::<(TransferTokenBalanceResult,)>((r,));
    
    do_payouts().await;
    return;
}    
    
async fn transfer_token_balance_(user_id: Principal, q: TransferTokenBalanceQuest) -> TransferTokenBalanceResult {
    
    if msg_cycles_available128() < TRANSFER_TOKEN_BALANCE_FEE {
        return Err(TransferTokenBalanceError::MsgCyclesTooLow{ transfer_token_balance_fee: TRANSFER_TOKEN_BALANCE_FEE });
    }

    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_token_balance_locks.len() >= MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS {
            return Err(TransferTokenBalanceError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_token_balance_locks.contains(&user_id) {
            return Err(TransferTokenBalanceError::CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall);
        }
        cm_data.mid_call_user_token_balance_locks.insert(user_id);
        Ok(())
    }) {
        Ok(()) => {},
        Err(transfer_token_balance_error) => {
            return Err(transfer_token_balance_error);
        }
    }
    
    // check token balance and make sure to unlock the user on returns after here 
    let user_token_ledger_balance: Tokens = match check_user_cycles_market_token_ledger_balance(&user_id).await {
        Ok(token_ledger_balance) => token_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });
            return Err(TransferTokenBalanceError::CheckUserCyclesMarketTokenLedgerBalanceCallError((call_error.0 as u32, call_error.1)));            
        }
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &user_id) });
    
    let usable_user_token_balance: Tokens = user_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock);
    
    if usable_user_token_balance < q.tokens.saturating_add(q.token_fee) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });
        return Err(TransferTokenBalanceError::UserTokenBalanceTooLow{ user_token_balance: usable_user_token_balance });          
    }

    
    let token_transfer_result = token_transfer(
        TokenTransferArg {
            memo: Some(IcrcMemo(ByteBuf::from(*TRANSFER_TOKEN_BALANCE_MEMO))),
            amount: q.tokens.into(),
            fee: Some(q.token_fee.into()),
            from_subaccount: Some(principal_token_subaccount(&user_id)),
            to: q.to,
            created_at_time: Some(q.created_at_time.unwrap_or(time_nanos_u64()))
        }   
    ).await;
    
    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });

    match token_transfer_result {
        Ok(token_transfer_result) => match token_transfer_result {
            Ok(token_transfer_block_height) => {
                msg_cycles_accept128(TRANSFER_TOKEN_BALANCE_FEE);
                return Ok(token_transfer_block_height);
            },
            Err(token_transfer_error) => {
                match token_transfer_error {
                    TokenTransferError::BadFee{ .. } => {
                        msg_cycles_accept128(TRANSFER_TOKEN_BALANCE_FEE);
                    },
                    _ => {}
                }
                return Err(TransferTokenBalanceError::TokenTransferError(token_transfer_error));
            }
        },
        Err(token_transfer_call_error) => {
            return Err(TransferTokenBalanceError::TokenTransferCallError(token_transfer_call_error));
        }
    }

}



// --------------- VIEW-POSITONS -----------------

const VIEW_POSITIONS_CHUNK_SIZE: usize = 1000;

#[derive(CandidType, Deserialize)]
pub struct ViewPositionsQuest {
    opt_start_after_position_id: Option<PositionId>, // if none, start at the earliest position-id
}

#[derive(CandidType)]
pub struct ViewPositionsSponse<'a, T: 'a> {
    positions: &'a [T],
    is_last_chunk: bool // true if there are no current positions
}


#[query(manual_reply = true)]
pub fn view_cycles_positions(q: ViewPositionsQuest) {
    with(&CM_DATA, |cm_data| {
        view_positions(q, &cm_data.cycles_positions);
    });
}

#[query(manual_reply = true)]
pub fn view_token_positions(q: ViewPositionsQuest) {
    with(&CM_DATA, |cm_data| {
        view_positions(q, &cm_data.token_positions);
    });
}


fn view_positions<T: CandidType + PositionTrait>(q: ViewPositionsQuest, positions: &Vec<T>) {
    
    let mut positions_chunk: &[T] = &[];
    let mut is_last_chunk = true;
    
    if positions.len() > 0 {
        let start_position_i: usize = match q.opt_start_after_position_id {
            None => 0,
            Some(start_after_position_id) => {
                match positions.binary_search_by_key(&start_after_position_id, |p| p.id()) {
                    Ok(i) => i + 1,
                    Err(i) => i
                }
            }
        };
        let positions_with_start = &positions[start_position_i..]; 
        let min_of_position_with_start_len_and_chunk_size: usize = std::cmp::min(positions_with_start.len(), VIEW_POSITIONS_CHUNK_SIZE); 
        positions_chunk = &positions_with_start[..min_of_position_with_start_len_and_chunk_size];    
        is_last_chunk = min_of_position_with_start_len_and_chunk_size == positions_with_start.len(); 
    }
    

    reply::<(ViewPositionsSponse<T>,)>((
        ViewPositionsSponse{
            positions: positions_chunk,
            is_last_chunk: is_last_chunk
        }
    ,));
    
}



// --------------- VIEW-TRADE-LOGS -----------------


const VIEW_TRADE_LOGS_ON_THIS_CANISTER_CHUNK_SIZE: usize = (1*MiB + 512*KiB) / TradeLog::STABLE_MEMORY_SERIALIZE_SIZE;
const VIEW_TRADE_LOGS_ON_THIS_CANISTER_CHUNK_SIZE_BYTES: usize = VIEW_TRADE_LOGS_ON_THIS_CANISTER_CHUNK_SIZE * TradeLog::STABLE_MEMORY_SERIALIZE_SIZE;


#[derive(CandidType, Deserialize)]
pub struct ViewLatestTradeLogsQuest {
    opt_start_before_id: Option<PurchaseId>
}

#[derive(CandidType)]
pub struct ViewLatestTradeLogsSponse<'a> {
    trade_logs_len: u128, // the last trade_log_id + 1
    logs: &'a Bytes, // a list of the latest ( before the q.opt_start_before if Some) TradeLogs that are still on this canister
    storage_canisters: Vec<StorageCanister>, // list of the storage-canisters and their logs ranges
}


#[derive(CandidType, Deserialize)]
pub struct StorageCanister {
    // The id of the first log in this storage-canister
    first_log_id : u128,
    // The numbe8r of logs in this storage-canister
    length : u128,
    // the size of the log-serialization-format in this storage-canister. // backwards compatible bc the log will be extended by appending new bytes.
    // so clients can know where each log starts and finishes but if only knows about previous versions will still be able to decode the begining data of each log. 
    log_size: u32,
    // Callback to fetch the storage logs in this storage canister.
    callback : candid::Func,
}

//candid::define_function!(pub StorageSeeTradeLogsFunction : (SeeTradeLogsQuest) -> (StorageLogs) query);

#[query(manual_reply = true)]
pub fn view_trade_logs(q: ViewLatestTradeLogsQuest) { // -> ViewLatestTradeLogsSponse {
        

    with(&CM_DATA, |cm_data| {
        
        let trade_logs_len: u128 = cm_data.trade_logs_id_counter;        
        
        let mut logs_bytes: Vec<u8> = Vec::new();

        let first_trade_log_id_on_this_canister: Option<PurchaseId>/*none if there are no trade-logs on this canister*/ = if cm_data.trade_log_storage_buffer.len() >= TradeLog::STABLE_MEMORY_SERIALIZE_SIZE {
            Some(u128::from_be_bytes((&cm_data.trade_log_storage_buffer[16..32]).try_into().unwrap()))
        } else {
            cm_data.trade_logs.first().map(|l| l.id)
        };
        if let Some(first_trade_log_id_on_this_canister) = first_trade_log_id_on_this_canister {
            
            let last_trade_log_id_on_this_canister: PurchaseId = {
                cm_data.trade_logs.last().map(|p| p.id)
                .unwrap_or(u128::from_be_bytes((&cm_data.trade_log_storage_buffer[
                    cm_data.trade_log_storage_buffer.len()-TradeLog::STABLE_MEMORY_SERIALIZE_SIZE+16
                    ..
                    cm_data.trade_log_storage_buffer.len()-TradeLog::STABLE_MEMORY_SERIALIZE_SIZE+32
                ]).try_into().unwrap())) // we know there is at least one trade-log on this canister at this point and if it's not in the cm_data.trade_logs it must be in the flush buffer
            };
            let start_before_id: PurchaseId = match q.opt_start_before_id {
                None => {
                    last_trade_log_id_on_this_canister + 1
                }
                Some(q_start_before_id) => {
                    if q_start_before_id > last_trade_log_id_on_this_canister {
                        last_trade_log_id_on_this_canister + 1
                    } else {
                        q_start_before_id
                    }
                }
            };
            if start_before_id > first_trade_log_id_on_this_canister {
                let cm_data_trade_logs_till_i: usize = {
                    match cm_data.trade_logs.binary_search_by_key(&start_before_id, |l| l.id) {
                        Ok(i) => i,
                        Err(i) => {
                            if i == cm_data.trade_logs.len() {
                                cm_data.trade_logs.len()
                            } else {
                                0
                            }
                        }
                    }
                };
                let cm_data_trade_logs_till_stop = &cm_data.trade_logs[..cm_data_trade_logs_till_i];
                if cm_data_trade_logs_till_stop.len() > 0 {
                    let cm_data_trade_logs_bytes: Vec<u8> = cm_data_trade_logs_till_stop.rchunks(VIEW_TRADE_LOGS_ON_THIS_CANISTER_CHUNK_SIZE).next().unwrap()
                        .iter().map(|tl| { tl.stable_memory_serialize() })
                        .collect::<Vec<[u8; TradeLog::STABLE_MEMORY_SERIALIZE_SIZE]>>()
                        .concat();
                    logs_bytes = cm_data_trade_logs_bytes;
                }
                if logs_bytes.len() < VIEW_TRADE_LOGS_ON_THIS_CANISTER_CHUNK_SIZE_BYTES 
                && cm_data.trade_log_storage_buffer.len() >= TradeLog::STABLE_MEMORY_SERIALIZE_SIZE {
                    let trade_log_storage_buffer_first_log_id: PurchaseId = first_trade_log_id_on_this_canister; // since we are in cm_data.trade_log_storage_buffer.len() > 0 we know that the first_trade_log_id_on_this_canister is in the trade_log_storage_buffer  
                    let trade_log_storage_buffer_trade_logs_len: usize = cm_data.trade_log_storage_buffer.len() / TradeLog::STABLE_MEMORY_SERIALIZE_SIZE;
                    let trade_log_storage_buffer_till_i: usize = {
                        if start_before_id >= trade_log_storage_buffer_first_log_id + trade_log_storage_buffer_trade_logs_len as u128 {
                            cm_data.trade_log_storage_buffer.len()
                        } else {
                            // start_before_id must by within [1..] trade-logs in the trade_log_storage_buffer
                            (start_before_id - trade_log_storage_buffer_first_log_id) as usize * TradeLog::STABLE_MEMORY_SERIALIZE_SIZE
                        }
                    };
                    logs_bytes = vec![
                        cm_data.trade_log_storage_buffer[..trade_log_storage_buffer_till_i].rchunks(VIEW_TRADE_LOGS_ON_THIS_CANISTER_CHUNK_SIZE_BYTES - logs_bytes.len()).next().unwrap(), // unwrap is safe here bc we know that trade_log_storage_buffer is not empty and we know that start_before_id > first_trade_log_id_on_this_canister so trade_log_storage_buffer_till_i cannot be zero
                        &logs_bytes
                    ].concat();
                }
            }
        }
        
        let mut storage_canisters: Vec<StorageCanister> = Vec::new();
        for storage_canister in cm_data.trade_log_storage_canisters.iter() {
            storage_canisters.push(
                StorageCanister{
                    first_log_id : storage_canister.first_log_id,
                    length: storage_canister.length as u128,
                    log_size: storage_canister.log_size,
                    callback : candid::Func{ principal: storage_canister.canister_id, method: "view_trade_logs".to_string() }
                }
            );
        }
        
        reply::<(ViewLatestTradeLogsSponse,)>((
            ViewLatestTradeLogsSponse{
                trade_logs_len,
                logs: &Bytes::new(&logs_bytes),
                storage_canisters          
            }
        ,));
    })

}











// ------------------ CMCALLER-CALLBACKS -----------------------

#[update(manual_reply = true)]
pub async fn cm_message_cycles_position_purchase_purchaser_cmcaller_callback(q: CMCallbackQuest) -> () {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(cycles_position_purchase_cycles_positions_purchases_i) = cm_data.trade_logs.binary_search_by_key(&q.cm_call_id, |cycles_position_purchase| { cycles_position_purchase.id }) {
            cm_data.trade_logs[cycles_position_purchase_cycles_positions_purchases_i]
            .cycles_payout_data
            .cmcaller_cycles_payout_callback_complete = Some((cycles_transfer_refund, q.opt_call_error));
        }
    });

    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_cycles_position_purchase_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(cycles_position_purchase_cycles_positions_purchases_i) = cm_data.trade_logs.binary_search_by_key(&q.cm_call_id, |cycles_position_purchase| { cycles_position_purchase.id }) {
            cm_data.trade_logs[cycles_position_purchase_cycles_positions_purchases_i]
            .token_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_token_position_purchase_purchaser_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
        
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(token_position_purchase_token_positions_purchases_i) = cm_data.trade_logs.binary_search_by_key(&q.cm_call_id, |token_position_purchase| { token_position_purchase.id }) {
            cm_data.trade_logs[token_position_purchase_token_positions_purchases_i]
            .token_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_token_position_purchase_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(token_position_purchase_token_positions_purchases_i) = cm_data.trade_logs.binary_search_by_key(&q.cm_call_id, |token_position_purchase| { token_position_purchase.id }) {
            cm_data.trade_logs[token_position_purchase_token_positions_purchases_i]
            .cycles_payout_data
            .cmcaller_cycles_payout_callback_complete = Some((cycles_transfer_refund, q.opt_call_error));
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_void_cycles_position_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
    
    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(void_cycles_position_void_cycles_positions_i) = cm_data.void_cycles_positions.binary_search_by_key(&q.cm_call_id, |void_cycles_position| { void_cycles_position.position_id }) {
            cm_data.void_cycles_positions[void_cycles_position_void_cycles_positions_i]
            .cycles_payout_data
            .cmcaller_cycles_payout_callback_complete = Some((cycles_transfer_refund, q.opt_call_error));
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_void_token_position_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    with_mut(&CM_DATA, |cm_data| {
        if let Ok(void_token_position_void_token_positions_i) = cm_data.void_token_positions.binary_search_by_key(&q.cm_call_id, |void_token_position| { void_token_position.position_id }) {
            cm_data.void_token_positions[void_token_position_void_token_positions_i]
            .token_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}




// --------------- STOP-CALLS-FLAG --------------------

#[update]
pub fn controller_set_stop_calls_flag(stop_calls_flag: bool) {
    caller_is_controller_gaurd(&caller());
    
    localkey::cell::set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn controller_see_stop_calls_flag() -> bool {
    caller_is_controller_gaurd(&caller());
    
    localkey::cell::get(&STOP_CALLS)
}




// --------------- PAYOUTS-ERRORS -------------------

#[query(manual_reply = true)]
pub fn controller_see_payouts_errors(chunk_i: u32) {
    caller_is_controller_gaurd(&caller());
    
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[CallError]>,)>((cm_data.do_payouts_errors.chunks(100).nth(chunk_i as usize),));
    });
}

#[update]
pub fn controller_clear_payouts_errors() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.do_payouts_errors = Vec::new();
    });    
}




