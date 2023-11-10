// positions can be cancel with a 30-days of a lack of a match.

use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    //time::Duration,
    thread::LocalKey,
};
use cts_lib::{
    tools::{
        localkey::{
            self,
            refcell::{with, with_mut},
            cell::{get}
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
        sha256,
        upgrade_canisters::{
            upgrade_canisters_, 
            ControllerUpgradeCSQuest, 
            UpgradeOutcome,
        },        
    },
    cts_cb_authorizations::is_cts_cb_authorization_valid,
    consts::{
        KiB,
        MiB,
        NANOS_IN_A_SECOND,
        SECONDS_IN_A_HOUR,
        TRILLION,
    },
    types::{
        Cycles,
        CallError,
        canister_code::CanisterCode,
        cycles_market::{*, tc::{*, trade_log}},
        cts::UserAndCB,
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
                call_raw128,
                reply,
                reply_raw,
                msg_cycles_refunded128,
                msg_cycles_available128,
                msg_cycles_accept128,
            },
            canister_balance128,
            is_controller,
           
        },
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    },
};
use canister_tools::{self, MemoryId};
use candid::{
    self, 
    Principal,
    CandidType,
    Deserialize,
    utils::{encode_one},
    error::Error as CandidError,
};

use cm_storage_lib::LogStorageInit;


use serde_bytes::{ByteBuf};
use serde::Serialize;

// -------

mod types;
use types::*;

mod payouts;
use payouts::do_payouts;

mod flush_logs;
use flush_logs::FlushLogsStorageError;

mod candle_counter;
use candle_counter::CandleCounter;

// ---------------



#[derive(Serialize, Deserialize)]
struct CMData {
    cts_id: Principal,
    cm_main_id: Principal,
    icrc1_token_ledger: Principal,
    icrc1_token_ledger_transfer_fee: Tokens,
    positions_id_counter: u128,
    trade_logs_id_counter: u128,
    mid_call_user_token_balance_locks: HashSet<Principal>,
    cycles_positions: Vec<CyclesPosition>,
    token_positions: Vec<TokenPosition>,
    trade_logs: VecDeque<TradeLog>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    void_token_positions: Vec<VoidTokenPosition>,
    do_payouts_errors: Vec<CallError>,
    ongoing_buy_calls: u32,
    ongoing_sell_calls: u32,
    candle_counter: CandleCounter,
}

impl CMData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            cm_main_id: Principal::from_slice(&[]),
            icrc1_token_ledger: Principal::from_slice(&[]),
            icrc1_token_ledger_transfer_fee: 0,
            positions_id_counter: 0,
            trade_logs_id_counter: 0,
            mid_call_user_token_balance_locks: HashSet::new(),
            cycles_positions: Vec::new(),
            token_positions: Vec::new(),
            trade_logs: VecDeque::new(),
            void_cycles_positions: Vec::new(),
            void_token_positions: Vec::new(),
            do_payouts_errors: Vec::new(),
            ongoing_buy_calls: 0,
            ongoing_sell_calls: 0,
            candle_counter: CandleCounter::default(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LogStorageData {
    storage_canisters: Vec<StorageCanisterData>,
    #[serde(with = "serde_bytes")]
    storage_buffer: Vec<u8>,
    storage_flush_lock: bool,
    create_storage_canister_temp_holder: Option<Principal>,
    flush_storage_errors: Vec<(FlushLogsStorageError, u64/*timestamp_nanos*/)>,
    storage_canister_code: CanisterCode,
    storage_canister_init: LogStorageInit,
}
impl LogStorageData {
    fn new(storage_canister_init: LogStorageInit) -> Self {
        Self {
            storage_canisters: Vec::new(),
            storage_buffer: Vec::new(),
            storage_flush_lock: false,
            create_storage_canister_temp_holder: None,
            flush_storage_errors: Vec::new(),
            storage_canister_code: CanisterCode::empty(),
            storage_canister_init,
        }
    }
}


#[derive(Serialize, Deserialize)]
pub struct StorageCanisterData {
    log_size: u32,
    first_log_id: u128,
    length: u64, // number of logs current store on this storage canister
    is_full: bool,
    canister_id: Principal,
    creation_timestamp: u128, // set once when storage canister is create.
    module_hash: [u8; 32] // update this field when upgrading the storage canisters.
}












const STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION: MemoryId = MemoryId::new(0);
const TRADES_STORAGE_DATA_MEMORY_ID: MemoryId = MemoryId::new(1);
const POSITIONS_STORAGE_DATA_MEMORY_ID: MemoryId = MemoryId::new(2);




struct TradeFeeTier {
    // the max volume (in-clusive) of the trade fees of this tier. anything over this amount is the next tier
    volume_tcycles: u128,
    trade_fee_ten_thousandths: u128,   
}
impl TradeFeeTier {
    fn volume_cycles(&self) -> Cycles {
        self.volume_tcycles.saturating_mul(TRILLION)
    }
}

#[allow(non_upper_case_globals)]
const trade_fees_tiers: &[TradeFeeTier; 5] = &[
        TradeFeeTier{
            volume_tcycles: 100_000,
            trade_fee_ten_thousandths: 50,
        },
        TradeFeeTier{
            volume_tcycles: 500_000,
            trade_fee_ten_thousandths: 30,
        },
        TradeFeeTier{
            volume_tcycles: 1_000_000,
            trade_fee_ten_thousandths: 10,
        },
        TradeFeeTier{
            volume_tcycles: 5_000_000,
            trade_fee_ten_thousandths: 5,
        },
        TradeFeeTier{
            volume_tcycles: u128::MAX,
            trade_fee_ten_thousandths: 1,
        },
    ]; 

fn calculate_trade_fee(current_position_trade_volume_cycles: Cycles, trade_cycles: Cycles) -> Cycles/*fee-cycles*/ {    
    let mut trade_cycles_mainder: Cycles = trade_cycles;
    let mut fee_cycles: Cycles = 0;
    for i in 0..trade_fees_tiers.len() {
        if current_position_trade_volume_cycles + trade_cycles - trade_cycles_mainder + 1/*plus one for start with the fee tier for the current-trade-mount*/ 
        <= trade_fees_tiers[i].volume_cycles() {
            let trade_cycles_in_the_current_tier: Cycles = std::cmp::min(
                trade_cycles_mainder,
                trade_fees_tiers[i].volume_cycles().saturating_sub(current_position_trade_volume_cycles + trade_cycles - trade_cycles_mainder), 
            );
            trade_cycles_mainder -= trade_cycles_in_the_current_tier;
            fee_cycles += trade_cycles_in_the_current_tier / 10_000 * trade_fees_tiers[i].trade_fee_ten_thousandths; 
            
            if trade_cycles_mainder == 0 {
                break;
            }
        } 
    } 
    
    fee_cycles
}





pub fn minimum_tokens_match() -> Tokens {
    10_000/*for the fee ten-thousandths*/ + get(&TOKEN_LEDGER_TRANSFER_FEE) * 1000
}




#[allow(non_upper_case_globals)]
mod memory_location {
    use crate::*;
    
    pub const CANISTER_DATA_STORAGE_SIZE_MiB: usize = TC_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB / 3 - 20/*memory-size at the start [re]placement*/; 

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


const DO_VOID_POSITIONS_PAYOUTS_CHUNK_SIZE: usize = 5;
const DO_VOID_POSITIONS_UPDATE_STORAGE_POSITION_CHUNK_SIZE: usize = 5;
const DO_TRADE_LOGS_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_TRADE_LOGS_TOKEN_PAYOUTS_CHUNK_SIZE: usize = 10;


const CM_MESSAGE_METHOD_VOID_CYCLES_POSITION_POSITOR: &'static str       = "cm_message_void_cycles_position_positor";
const CM_MESSAGE_METHOD_VOID_TOKEN_POSITION_POSITOR: &'static str          = "cm_message_void_token_position_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR: &'static str   = "cm_message_cycles_position_purchase_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_cycles_position_purchase_purchaser";
const CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_POSITOR: &'static str      = "cm_message_token_position_purchase_positor";
const CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_PURCHASER: &'static str    = "cm_message_token_position_purchase_purchaser";


mod token_transfer_memo_mod {
    use crate::{PositionKind, PurchaseId};
    const TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-TPP-";
    const CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-CPP-";
    
    const TOKEN_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-TPPF";
    const CYCLES_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-CPPF";
    
    const VOID_TOKEN_POSITION_MEMO_START: &[u8; 8] = b"CTS-VTP-";
    
    
    pub fn position_purchase_token_transfer_memo(position_kind: PositionKind, purchase_id: PurchaseId) -> [u8; 24] {
        create_token_transfer_memo_(
            match position_kind {
                PositionKind::Cycles => CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START,
                PositionKind::Token => TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START,
            },
            purchase_id   
        )    
    }
    pub fn position_purchase_token_fee_collection_transfer_memo(position_kind: PositionKind, purchase_id: PurchaseId) -> [u8; 24] {
        create_token_transfer_memo_(
            match position_kind {
                PositionKind::Cycles => CYCLES_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START,
                PositionKind::Token => TOKEN_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START,
            },
            purchase_id
        )
        
    }
    fn create_token_transfer_memo_(memo_start: &[u8; 8], id: u128) -> [u8; 24] {
        let mut b: [u8; 24] = [0u8; 24];
        b[..8].copy_from_slice(memo_start);
        b[8..24].copy_from_slice(&id.to_be_bytes());
        return b;
    }
    pub fn create_void_token_position_transfer_memo(position_id: u128) -> [u8; 24] {
        create_token_transfer_memo_(VOID_TOKEN_POSITION_MEMO_START, position_id)
    }
}
use token_transfer_memo_mod::*;
    

const TRANSFER_TOKEN_BALANCE_MEMO: &[u8; 29] = b"CTS-CM-TOKEN-BALANCE-TRANSFER";

const MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS: usize = 500;



pub const VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS: u128 = SECONDS_IN_A_HOUR * 1;









thread_local! {
    
    static CM_DATA: RefCell<CMData> = RefCell::new(CMData::new()); 
    
    static POSITIONS_STORAGE_DATA: RefCell<LogStorageData> = RefCell::new(LogStorageData::new(
        LogStorageInit{ log_size: PositionLog::STABLE_MEMORY_SERIALIZE_SIZE as u32 }
    ));
    static TRADES_STORAGE_DATA: RefCell<LogStorageData> = RefCell::new(LogStorageData::new(
        LogStorageInit{ log_size: TradeLog::STABLE_MEMORY_SERIALIZE_SIZE as u32 }
    ));
    
    // not save through the upgrades
    static TOKEN_LEDGER_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static TOKEN_LEDGER_TRANSFER_FEE: Cell<Tokens> = Cell::new(0);
    static STOP_CALLS: Cell<bool> = Cell::new(false);   
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
    });
    
    with_mut(&TRADES_STORAGE_DATA, |trades_storage_data| {
        trades_storage_data.storage_canister_code = cm_init.trades_storage_canister_code;
    });
    with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| {
        positions_storage_data.storage_canister_code = cm_init.positions_storage_canister_code;
    });
    
    localkey::cell::set(&TOKEN_LEDGER_ID, cm_init.icrc1_token_ledger);
    localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, cm_init.icrc1_token_ledger_transfer_fee);
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
        localkey::cell::set(&CTS_ID, cm_data.cts_id);    
    });
    
}




// -------------------------------------------------------------

#[no_mangle]
fn canister_inspect_message() {
    use cts_lib::ic_cdk::api::call::{method_name, accept_message};
    
    if [
        "",
        "local_put_ic_root_key",        
    ].contains(&&method_name()[..]) {
        accept_message();
    } else {
        trap(&format!("this method {} must be call by a canister or a query call.", method_name()));
    }
    
}


// -------------------------------------------------------------


fn new_id(cm_data_id_counter: &mut u128) -> u128 {
    let id: u128 = cm_data_id_counter.clone();
    *(cm_data_id_counter) += 1;
    id
}


async fn token_transfer(q: TokenTransferArg) -> Result<Result<BlockId, TokenTransferError>, CallError> {
    let r = icrc1_transfer(localkey::cell::get(&TOKEN_LEDGER_ID), q).await;
    if let Ok(ref tr) = r {
        if let Err(TokenTransferError::BadFee { ref expected_fee }) = tr {
            localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, expected_fee.0.clone().try_into().unwrap_or(0));
            with_mut(&CM_DATA, |cm_data| {
                cm_data.icrc1_token_ledger_transfer_fee = expected_fee.0.clone().try_into().unwrap_or(0);
            });
        }
    } 
    r
}

async fn token_balance(count_id: IcrcId) -> Result<Tokens, CallError> {
    icrc1_balance_of(localkey::cell::get(&TOKEN_LEDGER_ID), count_id).await
}


async fn check_usable_user_token_balance(user_id: &Principal) -> Result<Tokens, CallError> {
    let user_cm_token_ledger_balance: Tokens = {
        token_balance(
            IcrcId{
                owner: ic_cdk::api::id(),
                subaccount: Some(principal_token_subaccount(user_id))
            }
        ).await?  
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| check_user_token_balance_in_the_lock(cm_data, user_id));
    
    Ok(user_cm_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock))
    
}


// it is possible that this can be more than the user's-subaccount token ledger balance. cause when doing the payouts it logs the payout transfer success after the payout batch is done.
// always use saturating_sub when subtracting this output from the user's-subaccount token ledger balance.
fn check_user_token_balance_in_the_lock(cm_data: &CMData, user_id: &Principal) -> Tokens {
    cm_data.token_positions.iter()
        .filter(|token_position: &&TokenPosition| { token_position.positor == *user_id })
        .fold(0, |cummulator: Tokens, user_token_position: &TokenPosition| {
            cummulator + user_token_position.current_position_tokens
        })
    +
    cm_data.trade_logs.iter()
        .filter(|tl: &&TradeLog| {
            tl.token_payout_payor() == *user_id && ( tl.token_payout_data.token_transfer.is_none() || tl.token_payout_data.token_fee_collection.is_none() )
        })
        .fold(0, |mut cummulator: Tokens, tl: &TradeLog| {
            if tl.token_payout_data.token_transfer.is_none() {
                cummulator += tl.tokens.saturating_sub(tl.tokens_payout_fee);//.saturating_sub(tl.token_ledger_transfer_fee()); 
            }
            if tl.token_payout_data.token_fee_collection.is_none() {
                cummulator += tl.tokens_payout_fee;//.saturating_add(tl.token_ledger_transfer_fee())
            }
            cummulator
        })
}





// -----------------




fn plus_one_ongoing_buy_call(cm_data: &mut CMData) {
    cm_data.ongoing_buy_calls = cm_data.ongoing_buy_calls.checked_add(1).unwrap();
}
fn minus_one_ongoing_buy_call(cm_data: &mut CMData) {
    cm_data.ongoing_buy_calls = cm_data.ongoing_buy_calls.saturating_sub(1);
}


#[update(manual_reply = true)]
pub fn trade_cycles(q: BuyTokensQuest, (user_of_the_cb, cts_cb_authorization): (Principal, Vec<u8>)) { // -> BuyTokensResult
    
    let caller: Principal = caller();
    
    if is_cts_cb_authorization_valid(
        localkey::cell::get(&CTS_ID),        
        UserAndCB{
            user_id: user_of_the_cb,
            cb_id: caller,
        },
        cts_cb_authorization
    ) == false {
        trap("Caller must be a CTS-CYCLES-BANK.");
    }
    
    with_mut(&CM_DATA, |cm_data| { plus_one_ongoing_buy_call(cm_data); });
    
    let buy_tokens_result: BuyTokensResult = buy_tokens_(caller, q);
    
    with_mut(&CM_DATA, |cm_data| { minus_one_ongoing_buy_call(cm_data); });
    
    reply::<(BuyTokensResult,)>((buy_tokens_result,));
    
    ic_cdk::spawn(do_payouts());
    return;   
}



fn buy_tokens_(caller: Principal, q: BuyTokensQuest) -> BuyTokensResult {
    
    if q.cycles / q.cycles_per_token_rate < minimum_tokens_match() {
        return Err(BuyTokensError::BuyTokensMinimumTokens(minimum_tokens_match()));
    }    
    
    if q.cycles_per_token_rate == 0 {
        return Err(BuyTokensError::RateCannotBeZero);
    }
    
    if msg_cycles_available128() < q.cycles {
        return Err(BuyTokensError::MsgCyclesTooLow);
    }    
    
    if canister_balance128().checked_add(q.cycles).is_none() {
        return Err(BuyTokensError::CyclesMarketIsBusy);
    }
    
    with_mut(&CM_DATA, |cm_data| {
        
        if cm_data.cycles_positions.len().saturating_add(cm_data.ongoing_buy_calls as usize) >= MAX_CYCLES_POSITIONS.saturating_sub(10)
        || MAX_VOID_CYCLES_POSITIONS
            .saturating_sub(cm_data.void_cycles_positions.len())
            .saturating_sub(cm_data.cycles_positions.len().saturating_add(cm_data.ongoing_buy_calls as usize))
             < 10 {
            // check for a bump?
            return Err(BuyTokensError::CyclesMarketIsBusy);
        }
        
        let cycles_position_id: PositionId = new_id(&mut cm_data.positions_id_counter); 
        
        let cycles_position: CyclesPosition = CyclesPosition{
            id: cycles_position_id,
            positor: caller,
            quest: q.clone(),
            current_position_cycles: q.cycles,
            purchases_rates_times_cycles_quantities_sum: 0,
            fill_quantity_tokens: 0,
            tokens_payouts_fees_sum: 0,
            timestamp_nanos: time_nanos(),
        };
          
        msg_cycles_accept128(q.cycles);
          
        with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| {
            positions_storage_data.storage_buffer.extend(cycles_position.as_stable_memory_position_log(None).stable_memory_serialize());  
        });
        
        cm_data.cycles_positions.push(cycles_position);  
        
        match_trades(
            cm_data.cycles_positions.len() - 1,
            &mut cm_data.cycles_positions,  
            &mut cm_data.token_positions, 
            &mut cm_data.void_cycles_positions,
            &mut cm_data.void_token_positions,
            &mut cm_data.trade_logs,
            &mut cm_data.trade_logs_id_counter,
            &mut cm_data.candle_counter,
        );
        
        Ok(BuyTokensSuccess{
            position_id: cycles_position_id
        })

    })
    
}




fn plus_one_ongoing_sell_call(cm_data: &mut CMData) {
    cm_data.ongoing_sell_calls = cm_data.ongoing_sell_calls.checked_add(1).unwrap();
}
fn minus_one_ongoing_sell_call(cm_data: &mut CMData) {
    cm_data.ongoing_sell_calls = cm_data.ongoing_sell_calls.saturating_sub(1);
}

#[update(manual_reply = true)]
pub async fn trade_tokens(q: SellTokensQuest, (user_of_the_cb, cts_cb_authorization): (Principal, Vec<u8>)) { // -> SellTokensResult
 
    let caller: Principal = caller();
    
    if is_cts_cb_authorization_valid(
        localkey::cell::get(&CTS_ID),
        UserAndCB{
            user_id: user_of_the_cb,
            cb_id: caller,
        },
        cts_cb_authorization
    ) == false {
        trap("Caller must be a CTS-CYCLES-BANK.");
    }
    
    with_mut(&CM_DATA, |cm_data| { plus_one_ongoing_sell_call(cm_data); });
    
    let sell_tokens_result: SellTokensResult = sell_tokens_(caller, q).await;
    
    with_mut(&CM_DATA, |cm_data| { minus_one_ongoing_sell_call(cm_data); });
    
    reply::<(SellTokensResult,)>((sell_tokens_result,));
    
    ic_cdk::spawn(do_payouts());
    return;   
}



async fn sell_tokens_(caller: Principal, q: SellTokensQuest) -> SellTokensResult {
    // make sure it is a canister
    if caller.as_slice().len() == 29 {
        trap("Caller must be a canister");
    }
    
    if q.tokens < minimum_tokens_match() {
        return Err(SellTokensError::SellTokensMinimum(minimum_tokens_match()));
    }
    
    if q.cycles_per_token_rate == 0 {
        return Err(SellTokensError::RateCannotBeZero);
    }    
    
    with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_token_balance_locks.contains(&caller) {
            return Err(SellTokensError::CallerIsInTheMiddleOfADifferentCallThatLocksTheTokenBalance);
        }
        if cm_data.mid_call_user_token_balance_locks.len() >= MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS {
            return Err(SellTokensError::CyclesMarketIsBusy);
        }
        cm_data.mid_call_user_token_balance_locks.insert(caller);
        Ok(())
    })?;    
        
    // check token balance and make sure to unlock the user on returns after here 
    
    let usable_user_token_balance: Tokens = {
        match check_usable_user_token_balance(&caller).await {
            Ok(t) => t,
            Err(e) => {
                with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&caller); });
                return Err(SellTokensError::CheckUserCyclesMarketTokenLedgerBalanceError(e));        
            }
        }
    };
    
    if usable_user_token_balance < q.tokens {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&caller); });
        return Err(SellTokensError::UserTokenBalanceTooLow{ user_token_balance: usable_user_token_balance });
    }
    
    
        
        
    let r: SellTokensResult = with_mut(&CM_DATA, |cm_data| {
        
        if cm_data.token_positions.len().saturating_add(cm_data.ongoing_sell_calls as usize) >= MAX_TOKEN_POSITIONS.saturating_sub(10)
        || MAX_VOID_TOKEN_POSITIONS
            .saturating_sub(cm_data.void_token_positions.len())
            .saturating_sub(cm_data.token_positions.len().saturating_add(cm_data.ongoing_sell_calls as usize))
             < 10 {
            // check for a bump? 30/90-days of positions without matches get cancel/void. 
            return Err(SellTokensError::CyclesMarketIsBusy);
        }
        
        let token_position_id: PositionId = new_id(&mut cm_data.positions_id_counter); 
    
        let token_position: TokenPosition = TokenPosition{
            id: token_position_id,
            positor: caller,
            quest: q.clone(),
            current_position_tokens: q.tokens,
            purchases_rates_times_token_quantities_sum: 0,
            cycles_payouts_fees_sum: 0,
            timestamp_nanos: time_nanos(),                
        };
        
        with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| {
            positions_storage_data.storage_buffer.extend(token_position.as_stable_memory_position_log(None).stable_memory_serialize());  
        });
        
        cm_data.token_positions.push(token_position);
        
        match_trades(
            cm_data.token_positions.len() - 1,
            &mut cm_data.token_positions,  
            &mut cm_data.cycles_positions, 
            &mut cm_data.void_token_positions,
            &mut cm_data.void_cycles_positions,
            &mut cm_data.trade_logs,
            &mut cm_data.trade_logs_id_counter,
            &mut cm_data.candle_counter,
        );
        
        Ok(SellTokensSuccess{
            position_id: token_position_id,
        })
    });
    
    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&caller); });
    
    r
}




fn match_trades<MatcherPositionType: CurrentPositionTrait, MatcheePositionType: CurrentPositionTrait>(
    start_matcher_positions_i: usize,
    matcher_positions: &mut Vec<MatcherPositionType>,  
    matchee_positions: &mut Vec<MatcheePositionType>, 
    matcher_void_positions: &mut Vec<MatcherPositionType::VoidPositionType>,
    matchee_void_positions: &mut Vec<MatcheePositionType::VoidPositionType>,
    trade_logs: &mut VecDeque<TradeLog>, 
    trade_logs_id_counter: &mut PurchaseId,
    candle_counter: &mut CandleCounter,
) {       
    
    if MatcherPositionType::POSITION_KIND == MatcheePositionType::POSITION_KIND {
        trap("MatcherPositionType::POSITION_KIND must be the opposite side of the MatcheePositionType::POSITION_KIND");
    }
        
    let mut matcher_position_i: usize = start_matcher_positions_i;
        
    let matcher_position_id: PositionId = matcher_positions[matcher_position_i].id();
    let mut match_rate: CyclesPerToken = matcher_positions[matcher_position_i].current_position_available_cycles_per_token_rate();
          
    'outer: loop {
        let mut i: usize = 0;
        while i < matchee_positions.len() {
            if let Some(trade_rate) = matchee_positions[i].is_this_position_better_than_or_equal_to_the_match_rate(match_rate) {
                if trade_logs.len() >= MAX_TRADE_LOGS {
                    break 'outer; // log that this matcher-position still needs matching.
                }
                
                let matchee_position: &mut MatcheePositionType = &mut matchee_positions[i];
                
                let matchee_position_vailable_rate_before_trade: CyclesPerToken = matchee_position.current_position_available_cycles_per_token_rate();
                
                let matcher_position: &mut MatcherPositionType = &mut matcher_positions[matcher_position_i];
                                                                                        
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
                    TradeLog{
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
                        cycles_payout_lock: false,
                        token_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        token_payout_data: TokenPayoutData::new_for_a_trade_log()
                    }
                );
                
                candle_counter.count_trade(trade_logs.back().unwrap());
                
                let mut matcher_position_is_void: bool = false;
                if matcher_position.current_position_tokens(matcher_position.current_position_available_cycles_per_token_rate()) < minimum_tokens_match() { 
                    let matcher_position: MatcherPositionType = matcher_positions.remove(matcher_position_i);
                    matcher_void_positions.insert(
                        matcher_void_positions.binary_search_by_key(&matcher_position_id, |vp| vp.position_id()).unwrap_err(),
                        matcher_position.into_void_position_type(Some(PositionTerminationCause::Fill))
                    );
                    matcher_position_is_void = true;
                }    
                
                if matchee_position.current_position_tokens(matchee_position.current_position_available_cycles_per_token_rate()) < minimum_tokens_match() {            
                    let position_for_the_void: MatcheePositionType = matchee_positions.remove(i);
                    let position_for_the_void_void_positions_insertion_i: usize = { 
                        matchee_void_positions.binary_search_by_key(
                            &position_for_the_void.id(),
                            |void_position| { void_position.position_id() }
                        ).unwrap_err()
                    };
                    matchee_void_positions.insert(
                        position_for_the_void_void_positions_insertion_i,
                        position_for_the_void.into_void_position_type(Some(PositionTerminationCause::Fill))
                    );
                } else if matchee_position.current_position_available_cycles_per_token_rate() != matchee_position_vailable_rate_before_trade { 
                    match_trades(
                        i,
                        matchee_positions,  
                        matcher_positions, 
                        matchee_void_positions,
                        matcher_void_positions,
                        trade_logs, 
                        trade_logs_id_counter,
                        candle_counter,
                    );
                    matcher_position_i = match matcher_positions.binary_search_by_key(&matcher_position_id, |p| p.id()) {
                        Ok(matcher_position_i) => matcher_position_i,
                        Err(_) => break 'outer,
                    };
                    i = 0;
                    match_rate = matcher_positions[matcher_position_i].current_position_available_cycles_per_token_rate();
                } else {
                    i = i + 1;
                }
                
                if matcher_position_is_void {
                    break 'outer;
                }
                
            } else {
                i = i + 1;
            }
        }
        
        let balance_rate: CyclesPerToken = matcher_positions[matcher_position_i].current_position_available_cycles_per_token_rate();
        match MatcheePositionType::POSITION_KIND {
            PositionKind::Token => {
                assert!(balance_rate >= match_rate);
            },
            PositionKind::Cycles => {
                assert!(balance_rate <= match_rate);
            } 
        }
        if balance_rate == match_rate {
            break 'outer;
        } else {
            match_rate = balance_rate;
        }
    }
}








#[derive(CandidType, Deserialize)]
pub struct ViewTokenLockQuest {
    principal_id: Principal,
}

#[query]
pub fn view_token_lock(q: ViewTokenLockQuest) -> Tokens {
    with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &q.principal_id) })
}





#[update(manual_reply = true)]
pub fn void_position(q: VoidPositionQuest) { // -> VoidPositionResult
    let caller: Principal = caller();
    
    let r: VoidPositionResult = void_position_(caller, q);
    
    reply::<(VoidPositionResult,)>((r,));
    
    ic_cdk::spawn(do_payouts());
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
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_i);
            let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = cm_data.void_cycles_positions.binary_search_by_key(&cycles_position_for_the_void.id, |vcp| { vcp.position_id }).unwrap_err();
            cm_data.void_cycles_positions.insert(
                cycles_position_for_the_void_void_cycles_positions_insertion_i,
                cycles_position_for_the_void.into_void_position_type(Some(PositionTerminationCause::UserCallVoidPosition))
            );
            Ok(())
        } else if let Ok(token_position_i) = cm_data.token_positions.binary_search_by_key(&q.position_id, |token_position| { token_position.id }) {
            if cm_data.token_positions[token_position_i].positor != caller {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(cm_data.token_positions[token_position_i].timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: cm_data.token_positions[token_position_i].timestamp_nanos/NANOS_IN_A_SECOND });
            }
            let token_position_for_the_void: TokenPosition = cm_data.token_positions.remove(token_position_i);
            let token_position_for_the_void_void_token_positions_insertion_i: usize = cm_data.void_token_positions.binary_search_by_key(&token_position_for_the_void.id, |vip| { vip.position_id }).unwrap_err();
            cm_data.void_token_positions.insert(
                token_position_for_the_void_void_token_positions_insertion_i,
                token_position_for_the_void.into_void_position_type(Some(PositionTerminationCause::UserCallVoidPosition))
            );
            Ok(())
        } else {
            return Err(VoidPositionError::PositionNotFound);
        }
    })
    
}






#[update(manual_reply = true)]
pub async fn transfer_token_balance(q: TransferTokenBalanceQuest) { // -> TransferTokenBalanceResult {
    
    let user_id: Principal = caller();
    
    let r: TransferTokenBalanceResult = transfer_token_balance_(user_id, q).await;
    
    reply::<(TransferTokenBalanceResult,)>((r,));
    
    do_payouts().await;
    return;
}    
    
async fn transfer_token_balance_(user_id: Principal, q: TransferTokenBalanceQuest) -> TransferTokenBalanceResult {

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
    
    let usable_user_token_balance: Tokens = {
        match check_usable_user_token_balance(&user_id).await {
            Ok(t) => t,
            Err(e) => {
                with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });
                return Err(TransferTokenBalanceError::CheckUserCyclesMarketTokenLedgerBalanceCallError(e));
            }
        }
    };    
  
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
                return Ok(token_transfer_block_height);
            },
            Err(token_transfer_error) => {
                return Err(TransferTokenBalanceError::TokenTransferError(token_transfer_error));
            }
        },
        Err(token_transfer_call_error) => {
            return Err(TransferTokenBalanceError::TokenTransferCallError(token_transfer_call_error));
        }
    }

}



// --------------- VIEW-POSITONS -----------------


#[derive(CandidType, Deserialize)]
pub struct ViewPositionBookQuest {
    opt_start_greater_than_rate: Option<CyclesPerToken>
}
#[derive(CandidType, Deserialize)]
pub struct ViewPositionBookSponse {
    positions_quantities: Vec<(CyclesPerToken, u128)>, 
    is_last_chunk: bool,
}

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


fn view_position_book_<T: CurrentPositionTrait>(q: ViewPositionBookQuest, current_positions: &Vec<T>) -> ViewPositionBookSponse {
    let mut positions_quantities: Vec<(CyclesPerToken, u128)> = vec![]; 
    let mut is_last_chunk: bool = true;
    
    let mut cps_as_rate_and_quantity: Vec<(CyclesPerToken, u128)> = current_positions
        .iter()
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
                
    for (r,q) in cps_as_rate_and_quantity.into_iter() {
        if let Some(latest_r_q) = positions_quantities.last_mut() {
            if latest_r_q.0 == r {
                latest_r_q.1 += q;
                continue;
            } 
        }
        if positions_quantities.len() >= MAX_POSITIONS_QUANTITIES {
            is_last_chunk = false;
            break;
        }
        positions_quantities.push((r,q));
            
    }
    
    ViewPositionBookSponse {
        positions_quantities, 
        is_last_chunk,
    }
    
}




// --------------- VIEW-TRADE-LOGS -----------------


#[query]
pub fn view_latest_trades(q: ViewLatestTradesQuest) -> ViewLatestTradesSponse {
    let mut trades_data: Vec<LatestTradesDataItem> = vec![];
    let mut is_last_chunk_on_this_canister: bool = true;
    with_mut(&CM_DATA, |cm_data| {
        cm_data.trade_logs.make_contiguous(); // since we are using a vecdeque here
        let tls: &[TradeLog] = &cm_data.trade_logs.as_slices().0[..q.opt_start_before_id.map(|sbid| cm_data.trade_logs.binary_search_by_key(&sbid, |tl| tl.id).unwrap_or_else(|e| e)).unwrap_or(cm_data.trade_logs.len())];
        if let Some(tl_chunk) = tls.rchunks(MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA).next() {  
            trades_data = tl_chunk.into_iter().map(|tl| {
                (
                    tl.id,
                    tl.tokens,
                    tl.cycles_per_token_rate,
                    tl.timestamp_nanos as u64,
                    tl.matchee_position_kind,
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
                            trade_log::position_kind_of_the_log_serialization(s),
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
                first_log_id_on_this_canister = Some(cm_data.trade_logs.front().unwrap().id);
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
 






// -----------
// view pending trades for a position
#[query(manual_reply = true)]
pub fn view_position_pending_trades(q: ViewStorageLogsQuest<<TradeLog as StorageLogTrait>::LogIndexKey>) {
    with_mut(&CM_DATA, |cm_data| {
        cm_data.trade_logs.make_contiguous();
    });
    with(&CM_DATA, |cm_data| {
        let logs_b: Vec<Vec<u8>> = {
            let till_i: usize = match q.opt_start_before_id {
                None => cm_data.trade_logs.len(),
                Some(start_before_id) => cm_data.trade_logs.binary_search_by_key(&start_before_id, |tl| tl.id).unwrap_or_else(|i| i),
            }; 
            let mut iter: Box<dyn DoubleEndedIterator<Item=&TradeLog>> = Box::new(cm_data.trade_logs.as_slices().0[..till_i].iter());
            if let Some(index_key) = q.index_key {
                iter = Box::new(iter.filter(move |tl| {
                    tl.position_id_matchee == index_key || tl.position_id_matcher == index_key
                }));
            }
            iter
                .rev()
                .take((1*MiB + 512*KiB)/TradeLog::STABLE_MEMORY_SERIALIZE_SIZE)
                .collect::<Vec<&TradeLog>>()
                .iter()
                .rev()
                .map(|tl| {
                    let mut v = Vec::new();
                    v.extend(tl.stable_memory_serialize());
                    v.extend_from_slice(&[tl.cycles_payout_data.is_complete() as u8, tl.token_payout_data.is_complete() as u8]);
                    v
                })
                .collect()
        };        
        reply_raw(&logs_b.concat());
    });
}





// ---------------
// view user current positions

#[query(manual_reply = true)]
pub fn view_user_current_positions(q: ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>) {
    fn d<T: CurrentPositionTrait>(q: ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>, current_positions: &Vec<T>) -> Box<dyn Iterator<Item=PositionLog> + '_> {
        Box::new(current_positions[
            ..
            match q.opt_start_before_id {
                None => current_positions.len(),
                Some(start_before_id) => current_positions.binary_search_by_key(&start_before_id, |p| p.id()).unwrap_or_else(|i| i)
            }
        ]
            .iter()
            .filter(move |p| p.positor() == q.index_key.unwrap_or(p.positor()))
            .map(|p| p.as_stable_memory_position_log(None)))
    }
    with(&CM_DATA, |cm_data| {
        let mut v: Vec<PositionLog> = d(q.clone(), &cm_data.cycles_positions).chain(d(q, &cm_data.token_positions)).collect();
        v.sort_by_key(|pl| pl.id);
        v.drain(..v.len().saturating_sub((1*MiB + 512*KiB)/PositionLog::STABLE_MEMORY_SERIALIZE_SIZE));
        let logs_b: Vec<Vec<u8>> = {
            v.into_iter()
            .map(|pl| pl.stable_memory_serialize())
            .collect()
        };
        reply_raw(&logs_b.concat());
    })
}



// ------

// only the logs, does not return current positions data
#[query(manual_reply = true)]
pub fn view_user_positions_logs(q: ViewStorageLogsQuest<<PositionLog as StorageLogTrait>::LogIndexKey>) {
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

// only the payout-complete logs, does not return trade-logs pending payouts or other tasks
#[query(manual_reply = true)]
pub fn view_position_purchases_logs(q: ViewStorageLogsQuest<<TradeLog as StorageLogTrait>::LogIndexKey>) {
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





#[derive(CandidType, Deserialize, Clone)]
pub struct ViewStorageLogsQuest<LogIndexKey> {
    opt_start_before_id: Option<u128>,
    index_key: Option<LogIndexKey>
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
                        
                if match_logs.len() >= max_logs { //* LogType::STABLE_MEMORY_SERIALIZE_SIZE >= 1*MiB + 512*KiB {
                    break;
                }
            }
            
            return Some(Box::new(match_logs.into_iter().rev()));

        } else {
            return None;
        }

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
    canister_id : Principal,
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





// --------------- STOP-CALLS-FLAG --------------------

#[update]
pub fn controller_set_stop_calls_flag(stop_calls_flag: bool) {
    caller_is_controller_gaurd(&caller());
    
    localkey::cell::set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn controller_view_stop_calls_flag() -> bool {
    caller_is_controller_gaurd(&caller());
    
    localkey::cell::get(&STOP_CALLS)
}




// --------------- PAYOUTS-ERRORS -------------------

#[query(manual_reply = true)]
pub fn controller_view_payouts_errors(chunk_i: u32) {
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



// -------------- UPGRADE STORAGE CANISTERS ---------------


#[update]
pub async fn controller_upgrade_log_storage_canisters(q: ControllerUpgradeCSQuest, log_storage_type: LogStorageType) -> Vec<(Principal, UpgradeOutcome)> {
    caller_is_controller_gaurd(&caller());
    
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
    
    let rs: Vec<(Principal, UpgradeOutcome)> = upgrade_canisters_(cs, &cc, &q.post_upgrade_quest).await; 
    
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












