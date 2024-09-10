use std::{
    cell::{Cell, RefCell},
    collections::{VecDeque, BTreeMap},
    time::Duration,
    thread::LocalKey,
    ops::Bound
};
use std::iter::DoubleEndedIterator;
use cts_lib::{
    tools::{
        localkey::{
            self,
            refcell::{with, with_mut},
        },
        cycles_transform_tokens,
        tokens_transform_cycles,
        principal_token_subaccount,
        time_nanos,
        time_nanos_u64,
        time_seconds,
        caller_is_controller_guard,
        call_error_as_u32_and_string,
        sha256,
        upgrade_canisters::{
            upgrade_canisters, 
            ControllerUpgradeCSQuest, 
            UpgradeOutcome,
        },       
        sns_validation_string 
    },
    consts::{
        KiB,
        MiB,
        NANOS_IN_A_SECOND,
        BILLION,
        TRILLION,
    },
    types::{
        Cycles,
        CallError,
        canister_code::CanisterCode,
        cm::{*, tc::{*, storage_logs::{*, trade_log::*, position_log::*}}},
    },
    management_canister,
    icrc::{
        IcrcId, 
        Tokens,
        Icrc1TransferError,
        Icrc1TransferQuest,
        BlockId,
        icrc1_transfer,
    },
};
use ic_cdk::{
    api::{
        trap,
        caller,
        call::{
            call_raw128,
            arg_data,
            reply,
            reply_raw,
            ArgDecoderConfig,
        },
    },
    update,
    query,
    init,
    pre_upgrade,
    post_upgrade
};
use canister_tools::{self, MemoryId};
use candid::{
    Principal,
    CandidType,
    Deserialize,
};
use serde_bytes::ByteArray;

// -------

mod payouts;
mod flush_logs;
mod candle_counter;
mod ledger_transfer;
mod trade_fee;
mod transfer_memo;
mod traits;

use payouts::do_payouts;
use ledger_transfer::LedgerTransferReturnType;
use traits::{
    TradeQuest,
    CurrentPositionTrait,
    VoidPositionTrait,
    LocalKeyRefCellLogStorageDataTrait,
};

// ---------------


#[allow(non_upper_case_globals)]
mod memory_location {
    use crate::*;
    
    const CANISTER_DATA_STORAGE_SIZE_MiB: usize = {
        (TC_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB / 5)     /*room for upgrade serialization and safety-space*/
        .saturating_sub(MAX_STORAGE_BUFFER_SIZE/MiB * 2)    /*positions & trades*/ 
        .saturating_sub(5 * 2)                              /*storage-canisters-code*/ 
        .saturating_sub(20)                                 /*memory-size at the start*/
    }; 

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


const STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION: MemoryId = MemoryId::new(0);
const POSITIONS_STORAGE_DATA_MEMORY_ID: MemoryId = MemoryId::new(1);
const TRADES_STORAGE_DATA_MEMORY_ID: MemoryId = MemoryId::new(2);

const DO_VOID_POSITIONS_PAYOUTS_CHUNK_SIZE: usize = 5;
const DO_VOID_POSITIONS_UPDATE_STORAGE_POSITION_CHUNK_SIZE: usize = 5;
const DO_TRADE_LOGS_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_TRADE_LOGS_TOKEN_PAYOUTS_CHUNK_SIZE: usize = 10;

const MAX_MID_CALL_USER_BALANCE_LOCKS: usize = 500;

pub const VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS: u128 = 0;

const CREATE_STORAGE_CANISTER_CYCLES: Cycles = 20 * TRILLION;

const POSITIONS_SUBACCOUNT: &[u8; 32] = &[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,5];

const MAX_INSTRUCTIONS_IN_THE_MATCH_TRADES_FN: u64 = 30 * BILLION;


thread_local! {
    
    static CM_DATA: RefCell<CMData> = RefCell::new(CMData::new()); 
    
    static POSITIONS_STORAGE_DATA: RefCell<LogStorageData> = RefCell::new(LogStorageData::new(
        LogStorageInit{ log_size: PositionLog::STABLE_MEMORY_SERIALIZE_SIZE as u32 }
    ));
    static TRADES_STORAGE_DATA: RefCell<LogStorageData> = RefCell::new(LogStorageData::new(
        LogStorageInit{ log_size: TradeLog::STABLE_MEMORY_SERIALIZE_SIZE as u32 }
    ));
    
    static TOKEN_LEDGER_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static TOKEN_LEDGER_TRANSFER_FEE: Cell<Tokens> = Cell::new(0);
    static CYCLES_BANK_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static CYCLES_BANK_TRANSFER_FEE: Cell<Tokens> = Cell::new(0);
    pub static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[])); 
}


// ------------------ INIT ----------------------

#[init]
fn init(cm_init: CMIcrc1TokenTradeContractInit) {
    canister_tools::init(&CM_DATA, STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION);
    canister_tools::init(&POSITIONS_STORAGE_DATA, POSITIONS_STORAGE_DATA_MEMORY_ID);
    canister_tools::init(&TRADES_STORAGE_DATA, TRADES_STORAGE_DATA_MEMORY_ID);
        
    with_mut(&CM_DATA, |cm_data| { 
        cm_data.cts_id = cm_init.cts_id; 
        cm_data.cm_main_id = cm_init.cm_main_id; 
        cm_data.icrc1_token_ledger = cm_init.icrc1_token_ledger; 
        cm_data.icrc1_token_ledger_transfer_fee = cm_init.icrc1_token_ledger_transfer_fee;
        cm_data.cycles_bank_id = cm_init.cycles_bank_id;
        cm_data.cycles_bank_transfer_fee = cm_init.cycles_bank_transfer_fee;
    });
    
    with_mut(&TRADES_STORAGE_DATA, |trades_storage_data| {
        trades_storage_data.storage_canister_code = cm_init.trades_storage_canister_code;
    });
    with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| {
        positions_storage_data.storage_canister_code = cm_init.positions_storage_canister_code;
    });
    
    localkey::cell::set(&TOKEN_LEDGER_ID, cm_init.icrc1_token_ledger);
    localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, cm_init.icrc1_token_ledger_transfer_fee);
    localkey::cell::set(&CYCLES_BANK_ID, cm_init.cycles_bank_id);
    localkey::cell::set(&CYCLES_BANK_TRANSFER_FEE, cm_init.cycles_bank_transfer_fee);
    localkey::cell::set(&CTS_ID, cm_init.cts_id);
} 

// ------------------ UPGRADES ------------------------

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    canister_tools::post_upgrade(&CM_DATA, STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION, None::<fn(CMData) -> CMData>);
    canister_tools::post_upgrade(&POSITIONS_STORAGE_DATA, POSITIONS_STORAGE_DATA_MEMORY_ID, None::<fn(LogStorageData) -> LogStorageData>);
    canister_tools::post_upgrade(&TRADES_STORAGE_DATA, TRADES_STORAGE_DATA_MEMORY_ID, None::<fn(LogStorageData) -> LogStorageData>);
    
    with(&CM_DATA, |cm_data| {
        localkey::cell::set(&TOKEN_LEDGER_ID, cm_data.icrc1_token_ledger);
        localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, cm_data.icrc1_token_ledger_transfer_fee);
        localkey::cell::set(&CYCLES_BANK_ID, cm_data.cycles_bank_id);
        localkey::cell::set(&CYCLES_BANK_TRANSFER_FEE, cm_data.cycles_bank_transfer_fee);
        localkey::cell::set(&CTS_ID, cm_data.cts_id);    
    });
}

// -----------------

fn new_id(cm_data_id_counter: &mut u128) -> u128 {
    let id: u128 = cm_data_id_counter.clone();
    *(cm_data_id_counter) += 1;
    id
}


pub fn minimum_tokens_match() -> Tokens {
    _minimum_match(localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE))
}
pub fn minimum_cycles_match() -> Cycles {
    _minimum_match(localkey::cell::get(&CYCLES_BANK_TRANSFER_FEE))
}


// -----------------


#[update]
pub async fn trade_cycles(q: TradeCyclesQuest) -> TradeResult {
    _trade(caller(), q).await
}

#[update]
pub async fn trade_tokens(q: TradeTokensQuest) -> TradeResult {
    _trade(caller(), q).await
}

async fn _trade<TradeQuestType: TradeQuest>(caller: Principal, q: TradeQuestType) -> TradeResult {
    
    if q.is_less_than_minimum_position() {
        return Err(TradeError::MinimumPosition{ minimum_cycles: minimum_cycles_match(), minimum_tokens: minimum_tokens_match()});
    }
    
    if q.cycles_per_token_rate() == 0 {
        return Err(TradeError::RateCannotBeZero);
    }    
    
    #[allow(non_snake_case)]
    for LOG_STORAGE_DATA in [&POSITIONS_STORAGE_DATA, &TRADES_STORAGE_DATA] {
        with(&LOG_STORAGE_DATA, |log_storage_data| {
            if log_storage_data.storage_buffer.len() >= MAX_STORAGE_BUFFER_SIZE {
                return Err(TradeError::CyclesMarketIsBusy);
            }
            Ok(())
        })?;
    }
    
    with_mut(&CM_DATA, |cm_data| {
        if TradeQuestType::matcher_positions(cm_data).len().saturating_add(TradeQuestType::mid_call_balance_locks(cm_data).len()) >= TradeQuestType::MAX_POSITIONS.saturating_sub(10)         
        || TradeQuestType::MAX_VOID_POSITIONS
            .saturating_sub(TradeQuestType::matcher_void_positions(cm_data).len())
            .saturating_sub(TradeQuestType::matcher_positions(cm_data).len())
            .saturating_sub(TradeQuestType::mid_call_balance_locks(cm_data).len())
             < 10 { 
            return Err(TradeError::CyclesMarketIsBusy);
        }
        if TradeQuestType::mid_call_balance_locks(cm_data).contains(&caller) {
            return Err(TradeError::CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance);
        }
        if TradeQuestType::mid_call_balance_locks(cm_data).len() >= MAX_MID_CALL_USER_BALANCE_LOCKS {
            return Err(TradeError::CyclesMarketIsBusy);
        }
        TradeQuestType::mid_call_balance_locks(cm_data).insert(caller);
        Ok(())
    })?;    
    
    let trade_result: TradeResult = __trade(caller, q).await;
    
    with_mut(&CM_DATA, |cm_data| {
        TradeQuestType::mid_call_balance_locks(cm_data).remove(&caller);
    });
    
    ic_cdk_timers::set_timer(Duration::from_millis(1), || ic_cdk::spawn(do_payouts()));
    
    trade_result        
}

async fn __trade<TradeQuestType: TradeQuest>(caller: Principal, q: TradeQuestType) -> TradeResult {
    
    match TradeQuestType::posit_transfer(
        Icrc1TransferQuest{
            memo: None,
            amount: q.quantity(),
            fee: q.posit_transfer_ledger_fee(),
            from_subaccount: Some(ByteArray::new(principal_token_subaccount(&caller))),
            to: IcrcId{owner: ic_cdk::id(), subaccount: Some(ByteArray::new(*POSITIONS_SUBACCOUNT))},
            created_at_time: None,
        }
    ).await {
        Ok(transfer_result) => match transfer_result {
            Ok(_block_id) => {}
            Err(transfer_error) => {
                return Err(TradeError::CreatePositionLedgerTransferError(transfer_error));
            }
        }
        Err(call_error) => {
            return Err(TradeError::CreatePositionLedgerTransferCallError(call_error));        
        }
    }
    // must be success afner the token transfer.
    
    Ok(with_mut(&CM_DATA, |cm_data| {
        
        let position_id: PositionId = new_id(&mut cm_data.positions_id_counter); 
        ic_cdk::print(&format!("creating position id: {position_id}"));
        
        let position: TradeQuestType::MatcherPositionType = TradeQuestType::create_current_position(q, position_id, caller);
        
        with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| {
            positions_storage_data.storage_buffer.extend(position.as_stable_memory_position_log(None).stable_memory_serialize());  
        });
        
        TradeQuestType::matcher_positions(cm_data).insert(position_id, position);
                
        TradeQuestType::match_trades(cm_data, position_id);
        
        TradeSuccess{
            position_id: position_id,
        }
    }))    
}


// ------


fn match_trades<MatcherPositionType: CurrentPositionTrait, MatcheePositionType: CurrentPositionTrait>(
    matcher_position_id: PositionId,
    matcher_positions: &mut BTreeMap<PositionId, MatcherPositionType>,  
    matchee_positions: &mut BTreeMap<PositionId, MatcheePositionType>, 
    matcher_void_positions: &mut BTreeMap<PositionId, MatcherPositionType::VoidPositionType>,
    matchee_void_positions: &mut BTreeMap<PositionId, MatcheePositionType::VoidPositionType>,
    trade_logs: &mut VecDeque<TradeLogAndTemporaryData>, 
    trade_logs_id_counter: &mut PurchaseId,
    candle_counter: &mut CandleCounter,
) {       
    
    if MatcherPositionType::POSITION_KIND == MatcheePositionType::POSITION_KIND {
        trap("MatcherPositionType::POSITION_KIND must be the opposite side of the MatcheePositionType::POSITION_KIND");
    }
        
    let matcher_position: &mut MatcherPositionType = match matcher_positions.get_mut(&matcher_position_id) {
        Some(p) => p,
        None => return,
    };
    
    let match_rate: CyclesPerToken = matcher_position.current_position_available_cycles_per_token_rate();
    
    let mut remove_matchee_positions_fill: Vec<PositionId> = Vec::new();
    
    for matchee_position in matchee_positions.values_mut() {
        if let Some(trade_rate) = matchee_position.is_this_position_better_than_or_equal_to_the_match_rate(match_rate) {
            if trade_logs.len() >= MAX_TRADE_LOGS {
                break; // we can put a timer to continue looking for matches for this position once there is space in the trade-logs queue, for now it will wait till another compatible position comes.
            }
            if ic_cdk::api::instruction_counter() >= MAX_INSTRUCTIONS_IN_THE_MATCH_TRADES_FN {
                break; // ..
            }
                                                                                    
            let purchase_tokens: Tokens = std::cmp::min(matcher_position.current_position_tokens(trade_rate), matchee_position.current_position_tokens(trade_rate));
            let matcher_position_payout_fee_cycles: Cycles = matcher_position.subtract_tokens(purchase_tokens, trade_rate);
            let matchee_position_payout_fee_cycles: Cycles = matchee_position.subtract_tokens(purchase_tokens, trade_rate);
                                            
            let tokens_payout_fee: Tokens = {
                cycles_transform_tokens(
                    if let PositionKind::Cycles = MatcherPositionType::POSITION_KIND {
                        matcher_position_payout_fee_cycles
                    } else {
                        matchee_position_payout_fee_cycles
                    },
                    trade_rate
                )
            };
            let cycles_payout_fee: Cycles = {
                if let PositionKind::Token = MatcherPositionType::POSITION_KIND {
                    matcher_position_payout_fee_cycles
                } else {
                    matchee_position_payout_fee_cycles
                }
            };
            
            let trade_log_id: PurchaseId = new_id(trade_logs_id_counter);
            trade_logs.push_back(
                TradeLogAndTemporaryData{
                    log: TradeLog{
                        position_id_matcher: matcher_position.id(),
                        position_id_matchee: matchee_position.id(),
                        id: trade_log_id,
                        matchee_position_positor: matchee_position.positor(),
                        matcher_position_positor: matcher_position.positor(),
                        tokens: purchase_tokens,
                        cycles: tokens_transform_cycles(purchase_tokens, trade_rate),
                        cycles_per_token_rate: trade_rate,
                        matchee_position_kind: MatcheePositionType::POSITION_KIND,
                        timestamp_nanos: time_nanos(),
                        tokens_payout_fee,
                        cycles_payout_fee,
                        cycles_payout_data: None,
                        token_payout_data: None,
                    },
                    temporary_data: TradeLogTemporaryData{
                        cycles_payout_lock: false,
                        token_payout_lock: false,
                        payout_cycles_to_subaccount: if let PositionKind::Token = MatcherPositionType::POSITION_KIND {
                            matcher_position.payout_to_subaccount()
                        } else {
                            matchee_position.payout_to_subaccount()
                        },
                        payout_tokens_to_subaccount: if let PositionKind::Cycles = MatcherPositionType::POSITION_KIND {
                            matcher_position.payout_to_subaccount()
                        } else {
                            matchee_position.payout_to_subaccount()
                        },
                    }
                }
            );
            
            candle_counter.count_trade(&trade_logs.back().unwrap().log);
            
            if matchee_position.current_position_tokens(matchee_position.current_position_available_cycles_per_token_rate()) < minimum_tokens_match() 
            || tokens_transform_cycles(matchee_position.current_position_tokens(matchee_position.current_position_available_cycles_per_token_rate()), matchee_position.current_position_available_cycles_per_token_rate()) < minimum_cycles_match() {
                remove_matchee_positions_fill.push(matchee_position.id());
            }
            
            if matcher_position.current_position_tokens(matcher_position.current_position_available_cycles_per_token_rate()) < minimum_tokens_match() 
            || tokens_transform_cycles(matcher_position.current_position_tokens(matcher_position.current_position_available_cycles_per_token_rate()), matcher_position.current_position_available_cycles_per_token_rate()) < minimum_cycles_match() { 
                let matcher_position: MatcherPositionType = match matcher_positions.remove(&matcher_position_id) { Some(p)=>p, None=>break, };
                matcher_void_positions.insert(
                    matcher_position.id(),
                    matcher_position.into_void_position_type(PositionTerminationCause::Fill)
                );
                break;
            }    
        }
    }
    
    for remove_matchee_position_id in remove_matchee_positions_fill.into_iter() {
        let matchee_position: MatcheePositionType = match matchee_positions.remove(&remove_matchee_position_id) {
            Some(p) => p,
            None => continue,
        };
        matchee_void_positions.insert(
            matchee_position.id(),
            matchee_position.into_void_position_type(PositionTerminationCause::Fill)
        );  
    }    
}



#[query]
pub fn sns_validate_trade_cycles(q: TradeCyclesQuest) -> Result<String,String> {
    Ok(sns_validation_string(q))
}

#[query]
pub fn sns_validate_trade_tokens(q: TradeTokensQuest) -> Result<String,String> {
    Ok(sns_validation_string(q))
}





#[update]
pub fn void_position(q: VoidPositionQuest) -> VoidPositionResult {
    let caller: Principal = caller();
    
    let r: VoidPositionResult = void_position_(caller, q);
    
    ic_cdk_timers::set_timer(Duration::from_millis(1), || ic_cdk::spawn(do_payouts()));    
    
    r
}   

#[query]
pub fn sns_validate_void_position(q: VoidPositionQuest) -> Result<String,String> {
    Ok(sns_validation_string(q))
}
    
fn void_position_(caller: Principal, q: VoidPositionQuest) -> VoidPositionResult {
    
    with_mut(&CM_DATA, |cm_data| {
        if let Some(cycles_position) = cm_data.cycles_positions.get(&q.position_id) {
            if cycles_position.positor != caller {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(cycles_position.timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: cycles_position.timestamp_nanos/NANOS_IN_A_SECOND });
            }  
            let cycles_position: CyclesPosition = cm_data.cycles_positions.remove(&cycles_position.id()).unwrap();
            cm_data.void_cycles_positions.insert(
                cycles_position.id(),
                cycles_position.into_void_position_type(PositionTerminationCause::UserCallVoidPosition)
            );
            Ok(())
        } else if let Some(token_position) = cm_data.token_positions.get(&q.position_id) {
            if token_position.positor != caller {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(token_position.timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: token_position.timestamp_nanos/NANOS_IN_A_SECOND });
            }
            let token_position: TokenPosition = cm_data.token_positions.remove(&token_position.id()).unwrap();
            cm_data.void_token_positions.insert(
                token_position.id(),
                token_position.into_void_position_type(PositionTerminationCause::UserCallVoidPosition)
            );
            Ok(())
        } else {
            return Err(VoidPositionError::PositionNotFound);
        }
    })
    
}



#[update]
pub async fn transfer_cycles_balance(q: TransferBalanceQuest) -> TransferBalanceResult {
    _transfer_balance::<TradeCyclesQuest>(caller(), q).await
}

#[update]
pub async fn transfer_token_balance(q: TransferBalanceQuest) -> TransferBalanceResult {
    _transfer_balance::<TradeTokensQuest>(caller(), q).await
}

#[query]
pub fn sns_validate_transfer_cycles_balance(q: TransferBalanceQuest) -> Result<String,String> {
    Ok(sns_validation_string(q))
}

#[query]
pub fn sns_validate_transfer_token_balance(q: TransferBalanceQuest) -> Result<String,String> {
    Ok(sns_validation_string(q))
}

async fn _transfer_balance<TradeQuestType: TradeQuest>(caller: Principal, q: TransferBalanceQuest) -> TransferBalanceResult {

    with_mut(&CM_DATA, |cm_data| {
        if TradeQuestType::mid_call_balance_locks(cm_data).contains(&caller) {
            return Err(TransferBalanceError::CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance);
        }
        if TradeQuestType::mid_call_balance_locks(cm_data).len() >= MAX_MID_CALL_USER_BALANCE_LOCKS {
            return Err(TransferBalanceError::CyclesMarketIsBusy);
        }
        TradeQuestType::mid_call_balance_locks(cm_data).insert(caller);
        Ok(())
    })?;
    
    let transfer_call_result: LedgerTransferReturnType = TradeQuestType::posit_transfer(
        Icrc1TransferQuest {
            memo: None,
            amount: q.amount,
            fee: q.ledger_transfer_fee,
            from_subaccount: Some(ByteArray::new(principal_token_subaccount(&caller))),
            to: q.to,
            created_at_time: None
        }   
    ).await;
    
    with_mut(&CM_DATA, |cm_data| { 
        TradeQuestType::mid_call_balance_locks(cm_data).remove(&caller);
    });

    match transfer_call_result {
        Ok(transfer_result) => match transfer_result {
            Ok(block_height) => {
                Ok(block_height)
            }
            Err(transfer_error) => {
                Err(TransferBalanceError::TransferError(transfer_error))
            }
        }
        Err(transfer_call_error) => {
            Err(TransferBalanceError::TransferCallError(transfer_call_error))
        }
    }
}




// --------------- VIEW-POSITONS -----------------



const MAX_POSITIONS_QUANTITIES: usize = 512*KiB*3 / std::mem::size_of::<(CyclesPerToken, u128)>();


#[query]
pub fn view_cycles_position_book(q: ViewPositionBookQuest) -> ViewPositionBookSponse {
    with(&CM_DATA, |cm_data| {
        view_position_book_(q, &cm_data.cycles_positions)  
    })
}

#[query]
pub fn view_tokens_position_book(q: ViewPositionBookQuest) -> ViewPositionBookSponse {
    with(&CM_DATA, |cm_data| {
        view_position_book_(q, &cm_data.token_positions)  
    })    
}


fn view_position_book_<T: CurrentPositionTrait>(q: ViewPositionBookQuest, current_positions: &BTreeMap<PositionId, T>) -> ViewPositionBookSponse {
    let mut positions_quantities: Vec<(CyclesPerToken, u128)> = vec![]; 
    let mut cps_as_rate_and_quantity: Vec<(CyclesPerToken, u128)> = current_positions
        .values()
        .map(|p| {
            let rate = p.current_position_available_cycles_per_token_rate(); 
            (rate, p.current_position_quantity())
        })
        .collect();
    cps_as_rate_and_quantity.sort_by_key(|d| d.0);
        
    if let Some(start_greater_than_rate) = q.opt_start_greater_than_rate {
        let partition_point: usize = cps_as_rate_and_quantity.partition_point(|(r,_)| { r <= &start_greater_than_rate });
        cps_as_rate_and_quantity.drain(..partition_point);    
    }
                
    for (r,q) in cps_as_rate_and_quantity.iter().copied() {
        if let Some(latest_r_q) = positions_quantities.last_mut() {
            if latest_r_q.0 == r {
                latest_r_q.1 += q;
                continue;
            } 
        }
        if positions_quantities.len() >= MAX_POSITIONS_QUANTITIES {
            break;
        }
        positions_quantities.push((r,q));
    }
    
    ViewPositionBookSponse {
        is_last_chunk: positions_quantities.len() == 0 || positions_quantities.last().unwrap().0 == cps_as_rate_and_quantity.last().unwrap().0,
        positions_quantities, 
    }
    
}




// --------------- VIEW-TRADE-LOGS -----------------


#[query]
pub fn view_latest_trades(q: ViewLatestTradesQuest) -> ViewLatestTradesSponse {
    let mut trades_data: Vec<LatestTradesDataItem> = vec![];
    let mut is_last_chunk_on_this_canister: bool = true;
    with_mut(&CM_DATA, |cm_data| {
        cm_data.trade_logs.make_contiguous(); // since we are using a vecdeque here
        let tls: &[TradeLogAndTemporaryData] = &cm_data.trade_logs.as_slices().0[..q.opt_start_before_id.map(|sbid| cm_data.trade_logs.binary_search_by_key(&sbid, |tl| tl.log.id).unwrap_or_else(|e| e)).unwrap_or(cm_data.trade_logs.len())];
        if let Some(tl_chunk) = tls.rchunks(MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA).next() {  
            trades_data = tl_chunk.into_iter().map(|tl_and_temp| {
                let tl: &TradeLog = &tl_and_temp.log;
                (
                    tl.id,
                    tl.tokens,
                    tl.cycles_per_token_rate,
                    tl.timestamp_nanos as u64,
                )
            }).collect();
        }
        if trades_data.len() < MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA {
            // check-storage_buffer
            with(&TradeLog::LOG_STORAGE_DATA, |log_storage_data| {
                if let Some(iter) = view_storage_logs_::<TradeLog>(
                    ViewStorageLogsQuest{
                        opt_start_before_id: q.opt_start_before_id,
                        index_key: None, 
                    },
                    log_storage_data,
                    MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA - trades_data.len(),
                ) {
                    let mut v = iter.map(|s: &[u8]| {
                        (
                            TradeLog::log_id_of_the_log_serialization(s),
                            trade_log::tokens_quantity_of_the_log_serialization(s),
                            trade_log::rate_of_the_log_serialization(s),
                            trade_log::timestamp_nanos_of_the_log_serialization(s) as u64,
                        )
                    })
                    .collect::<Vec<LatestTradesDataItem>>();
                    
                    v.append(&mut trades_data);
                    
                    trades_data = v;
                }
            });
        }
        // is last chunk
        let mut first_log_id_on_this_canister: Option<u128> = None;
        with(&TradeLog::LOG_STORAGE_DATA, |log_storage_data| {
            if log_storage_data.storage_buffer.len() >= TradeLog::STABLE_MEMORY_SERIALIZE_SIZE {
                first_log_id_on_this_canister = Some(TradeLog::log_id_of_the_log_serialization(&log_storage_data.storage_buffer[..TradeLog::STABLE_MEMORY_SERIALIZE_SIZE]));
            } 
        });
        if let None = first_log_id_on_this_canister {
            if cm_data.trade_logs.len() >= 1 {
                first_log_id_on_this_canister = Some(cm_data.trade_logs.front().unwrap().log.id);
            }
        }
        if trades_data.len() >= 1 && trades_data.first().unwrap().0 != first_log_id_on_this_canister.unwrap() { /*unwrap cause if there is at least one in the sponse we know there is at least one on the canistec*/
            is_last_chunk_on_this_canister = false;
        }
    });
    ViewLatestTradesSponse {
        trades_data, 
        is_last_chunk_on_this_canister,
    }   
}
 



// ---------------
// view user current positions

// frontend method with the custom serialization.
#[export_name = "canister_query view_user_current_positions"]
pub extern "C" fn view_user_current_positions() {
    let (q,): (ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>,) = arg_data(ArgDecoderConfig::default());
    let v: Vec<PositionLog> = _view_current_positions(q);
    let logs_b: Vec<Vec<u8>> = {
        v.into_iter()
        .map(|pl| pl.stable_memory_serialize())
        .collect()
    };
    reply_raw(&logs_b.concat());
}

#[query]
pub fn view_current_positions(q: ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>) -> Vec<PositionLog> {
    _view_current_positions(q)
}    

fn _view_current_positions(q: ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>) -> Vec<PositionLog> {
    fn d<T: CurrentPositionTrait>(q: ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>, current_positions: &BTreeMap<PositionId, T>) -> Box<dyn Iterator<Item=PositionLog> + '_> {
        let mut iter: Box<dyn DoubleEndedIterator<Item=PositionLog>> 
        = Box::new(
            current_positions.range((
                Bound::Unbounded, 
                q.opt_start_before_id.map(|sbid| Bound::Excluded(sbid)).unwrap_or(Bound::Unbounded)
            ))
            .map(|(_k,v)| v)
            .map(|p| p.as_stable_memory_position_log(None))
            .rev());
        if let Some(index_key) = q.index_key {
            iter = Box::new(iter.filter(move |pl| pl.positor == index_key));
        }
        Box::new(
            iter
                .take((1*MiB + 512*KiB)/PositionLog::STABLE_MEMORY_SERIALIZE_SIZE)
                .collect::<Vec<PositionLog>>()
                .into_iter()
                .rev()
        )
    }
    with(&CM_DATA, |cm_data| {
        let mut v: Vec<PositionLog> = d(q.clone(), &cm_data.cycles_positions).chain(d(q, &cm_data.token_positions)).collect();
        v.sort_by_key(|pl| pl.id);
        v.drain(..v.len().saturating_sub((1*MiB + 512*KiB)/PositionLog::STABLE_MEMORY_SERIALIZE_SIZE));
        v
    })
}


// positions pending a void-position-payout and/or update-storage-logs-performance
// extra byte for if the void-position-payout is complete
#[export_name = "canister_query view_void_positions_pending"]
pub extern "C" fn view_void_positions_pending() {
    let (q,): (ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>,) = arg_data(ArgDecoderConfig::default());
    
    fn d<VoidPositionType: VoidPositionTrait>(q: ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>, void_positions: &BTreeMap<PositionId, VoidPositionType>) -> Box<dyn Iterator<Item=(&PositionLog, bool/*is_payout_complete*/)> + '_> {
        let mut iter: Box<dyn DoubleEndedIterator<Item=(&PositionLog, bool)>> 
        = Box::new(
            void_positions.range((
                Bound::Unbounded, 
                q.opt_start_before_id.map(|sbid| Bound::Excluded(sbid)).unwrap_or(Bound::Unbounded)
            ))
            .map(|(_k,v)| v)
            .map(|vp| (&(vp.update_storage_position_data().update_storage_position_log), vp.payout_data().is_some()))
            .rev());
        if let Some(index_key) = q.index_key {
            iter = Box::new(iter.filter(move |(pl, _)| pl.positor == index_key));
        }
        Box::new(
            iter
                .take((1*MiB + 512*KiB)/PositionLog::STABLE_MEMORY_SERIALIZE_SIZE)
                .collect::<Vec<(&PositionLog, bool)>>()
                .into_iter()
                .rev()
        )
    }
    with(&CM_DATA, |cm_data| {
        let mut v: Vec<(&PositionLog, bool)> = d(q.clone(), &cm_data.void_cycles_positions).chain(d(q, &cm_data.void_token_positions)).collect();
        v.sort_by_key(|(pl, _)| pl.id);
        v.drain(..v.len().saturating_sub((1*MiB + 512*KiB)/PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1));
        let logs_b: Vec<Vec<u8>> = {
            v.into_iter()
            .map(|(pl, is_payout_complete)| { 
                let mut v = pl.stable_memory_serialize();
                v.push(is_payout_complete as u8);
                v
            })
            .collect()
        };
        reply_raw(&logs_b.concat());
    })
}





// -----------
// view trades in the trade_logs list
// these trades are pending (a cycles and/or token payout) and/or waiting for being put into the storage-logs.  
#[export_name = "canister_query view_position_pending_trades"]
pub extern "C" fn view_position_pending_trades() {
    let (q,): (ViewStorageLogsQuest<<TradeLog as StorageLogTrait>::LogIndexKey>,) = arg_data(ArgDecoderConfig::default());

    with_mut(&CM_DATA, |cm_data| {
        cm_data.trade_logs.make_contiguous();
    });
    with(&CM_DATA, |cm_data| {
        let logs_b: Vec<Vec<u8>> = {
            let till_i: usize = match q.opt_start_before_id {
                None => cm_data.trade_logs.len(),
                Some(start_before_id) => cm_data.trade_logs.binary_search_by_key(&start_before_id, |tl| tl.log.id).unwrap_or_else(|i| i),
            }; 
            let mut iter: Box<dyn DoubleEndedIterator<Item=&TradeLog>> = Box::new(cm_data.trade_logs.as_slices().0[..till_i].iter().map(|tl_and_temp| &tl_and_temp.log));
            if let Some(index_key) = q.index_key {
                iter = Box::new(iter.filter(move |tl| {
                    tl.position_id_matchee == index_key || tl.position_id_matcher == index_key
                }));
            }
            iter
                .rev()
                .take((1*MiB + 512*KiB)/TradeLog::STABLE_MEMORY_SERIALIZE_SIZE + 2)
                .collect::<Vec<&TradeLog>>()
                .into_iter()
                .rev()
                .map(|tl| {
                    let mut v = Vec::new();
                    v.extend(tl.stable_memory_serialize());
                    v.extend_from_slice(&[tl.cycles_payout_data.is_some() as u8, tl.token_payout_data.is_some() as u8]);
                    v
                })
                .collect()
        };        
        reply_raw(&logs_b.concat());
    });
}



// The position-storage-logs in the buffer on this canister, does not return current positions data or pending-void-positions.
#[export_name = "canister_query view_user_positions_logs"]
pub extern "C" fn view_user_positions_logs() {
    let (q,): (ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>,) = arg_data(ArgDecoderConfig::default());
 
    let mut v: Vec<u8> = Vec::new();
    with(&PositionLog::LOG_STORAGE_DATA, |log_storage_data| {
        if let Some(iter) = view_storage_logs_::<PositionLog>(q, log_storage_data, 1*MiB+512*KiB / PositionLog::STABLE_MEMORY_SERIALIZE_SIZE) {
            for s in iter {
                v.extend_from_slice(s);
            }
        }
    });
    reply_raw(&v);
} 

// The trades-storage-logs in the buffer on this canister. the payout-complete logs, does not return trade-logs pending payouts.
#[export_name = "canister_query view_position_purchases_logs"]
pub extern "C" fn view_position_purchases_logs() {
    let (q,): (ViewStorageLogsQuest<<TradeLog as StorageLogTrait>::LogIndexKey>,) = arg_data(ArgDecoderConfig::default());

    let mut v: Vec<u8> = Vec::new();
    with(&TradeLog::LOG_STORAGE_DATA, |log_storage_data| {
        if let Some(iter) = view_storage_logs_::<TradeLog>(q, log_storage_data, 1*MiB+512*KiB / TradeLog::STABLE_MEMORY_SERIALIZE_SIZE) {
            for s in iter {
                v.extend_from_slice(s);
            }
        }
    });
    reply_raw(&v);
} 



fn view_storage_logs_<'a, LogType: StorageLogTrait>(
    q: ViewStorageLogsQuest<LogType::LogIndexKey>, 
    log_storage_data: &'a LogStorageData,
    max_logs: usize,
) -> Option<Box<dyn Iterator<Item=&'a [u8]> + 'a>> { // none if empty
    let log_storage_buffer = &log_storage_data.storage_buffer;
    
    if log_storage_buffer.len() >= LogType::STABLE_MEMORY_SERIALIZE_SIZE {
        let first_log_id_in_the_storage_buffer: u128 = LogType::log_id_of_the_log_serialization(&log_storage_buffer[..LogType::STABLE_MEMORY_SERIALIZE_SIZE]);
        
        let logs_storage_buffer_till_start_before_id: &[u8] = &log_storage_buffer[
            ..
            q.opt_start_before_id
                .map(|start_before_id| {
                    std::cmp::min(
                        {
                            start_before_id
                                .checked_sub(first_log_id_in_the_storage_buffer)
                                .unwrap_or(0) as usize
                            * 
                            LogType::STABLE_MEMORY_SERIALIZE_SIZE
                        },
                        log_storage_buffer.len()
                    )
                })
                .unwrap_or(log_storage_buffer.len()) 
        ];
            
        let mut match_logs: Vec<&[u8]> = vec![];
        
        for i in 0..logs_storage_buffer_till_start_before_id.len() / LogType::STABLE_MEMORY_SERIALIZE_SIZE {
            
            let log_finish_i: usize = logs_storage_buffer_till_start_before_id.len() - i * LogType::STABLE_MEMORY_SERIALIZE_SIZE;
            
            let log: &[u8] = &logs_storage_buffer_till_start_before_id[
                log_finish_i - LogType::STABLE_MEMORY_SERIALIZE_SIZE
                ..
                log_finish_i
            ];
            
            if let Some(ref index_key) = q.index_key {
                if LogType::index_keys_of_the_log_serialization(log).contains(index_key) == false {
                    continue;
                }
            }
            match_logs.push(log);
                    
            if match_logs.len() >= max_logs {
                break;
            }
        }
        
        return Some(Box::new(match_logs.into_iter().rev()));

    } else {
        return None;
    }
}



#[query]
pub fn view_positions_storage_canisters() -> Vec<StorageCanister> {
    view_log_storage_canisters_(&POSITIONS_STORAGE_DATA) 
}

#[query]
pub fn view_trades_storage_canisters() -> Vec<StorageCanister> {
    view_log_storage_canisters_(&TRADES_STORAGE_DATA) 
}


fn view_log_storage_canisters_(#[allow(non_snake_case)]LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>>) -> Vec<StorageCanister> {
    with(&LOG_STORAGE_DATA, |log_storage_data| {
        let mut storage_canisters: Vec<StorageCanister> = Vec::new();
        for storage_canister in log_storage_data.storage_canisters.iter() {
            storage_canisters.push(
                StorageCanister{
                    first_log_id : storage_canister.first_log_id,
                    length: storage_canister.length as u128,
                    log_size: storage_canister.log_size,
                    canister_id : storage_canister.canister_id,
                }
            );
        }
        storage_canisters
    })
}

// ---- candle-counter ----

use candle_counter::*;


#[export_name = "canister_query view_candles"]
pub extern "C" fn view_candles() {
    let (q,): (ViewCandlesQuest,) = arg_data(ArgDecoderConfig::default());
    
    with(&CM_DATA, |cm_data| {
        reply::<(ViewCandlesSponse,)>((create_candles(&cm_data.candle_counter, q),));
    });
}

#[query]
pub fn view_volume_stats() -> ViewVolumeStatsSponse {
    with(&CM_DATA, |cm_data| {
        create_view_volume_stats(&cm_data.candle_counter)
    })
}




// --------------- PAYOUTS-ERRORS -------------------

#[export_name = "canister_query view_payouts_errors"]
pub extern "C" fn view_payouts_errors() {
    let (chunk_i,): (u32,) = arg_data(ArgDecoderConfig::default());
    
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[CallError]>,)>((cm_data.do_payouts_errors.chunks(100).nth(chunk_i as usize),));
    });
}

#[export_name = "canister_update controller_clear_payouts_errors"]
pub extern "C" fn controller_clear_payouts_errors() {

    caller_is_controller_guard(&caller());
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.do_payouts_errors = Vec::new();
    });    
}



// -------------- UPGRADE STORAGE CANISTERS ---------------


#[update]
pub async fn controller_upgrade_log_storage_canisters(q: ControllerUpgradeCSQuest, log_storage_type: LogStorageType) -> Vec<(Principal, UpgradeOutcome)> {
    caller_is_controller_guard(&caller());
    
    #[allow(non_snake_case)]
    let LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>> = match log_storage_type {
        LogStorageType::Trades => &TRADES_STORAGE_DATA,
        LogStorageType::Positions => &POSITIONS_STORAGE_DATA,
    };
    
    
    let cc: CanisterCode = with_mut(&LOG_STORAGE_DATA, |log_storage_data| {
        if let Some(new_canister_code) = q.new_canister_code {
            if *(new_canister_code.module_hash()) != sha256(new_canister_code.module()) {
                trap("new_canister_code module hash does not match module");
            }
            log_storage_data.storage_canister_code = new_canister_code; 
        }
        log_storage_data.storage_canister_code.clone()
    });
    
    let cs: Vec<Principal> = match q.specific_cs {
        Some(cs) => cs.into_iter().collect(),
        None => {
            with(&LOG_STORAGE_DATA, |log_storage_data| {
                log_storage_data.storage_canisters.iter()
                .filter_map(|sc| {
                    if &sc.module_hash != cc.module_hash() {
                        Some(sc.canister_id.clone())
                    } else {
                        None
                    }
                })
                .take(200)
                .collect()
            })
        }
    };
    
    let rs: Vec<(Principal, UpgradeOutcome)> = upgrade_canisters(cs, &cc, &q.post_upgrade_quest).await; 
    
    // update successes in the main data.
    with_mut(&LOG_STORAGE_DATA, |log_storage_data| {
        for (sc, uo) in rs.iter() {
            if let Some(ref r) = uo.install_code_result {
                if r.is_ok() {
                    if let Some(i) = log_storage_data.storage_canisters.iter_mut().find(|i| i.canister_id == *sc) {
                        i.module_hash = cc.module_hash().clone();
                    } else {
                        ic_cdk::print("check this");
                    } 
                }
            }
        } 
    });
    
    return rs;
    
}



// ----- CONTROLLER_CALL_CANISTER-METHOD --------------------------

#[derive(CandidType, Deserialize)]
pub struct ControllerCallCanisterQuest {
    pub callee: Principal,
    pub method_name: String,
    pub arg_raw: Vec<u8>,
    pub cycles: Cycles
}


#[export_name = "canister_update controller_call_canister"]
pub extern "C" fn controller_call_canister() {
    caller_is_controller_guard(&caller());
    
    let (q,): (ControllerCallCanisterQuest,) = arg_data(ArgDecoderConfig::default());
            
    ic_cdk::spawn(async move {
        let r: Result<Vec<u8>, CallError> = call_raw128(
            q.callee,
            &q.method_name,
            &q.arg_raw,
            q.cycles
        )
        .await
        .map_err(call_error_as_u32_and_string);
        
        reply((r,));
    });
}





ic_cdk::export_candid!();
