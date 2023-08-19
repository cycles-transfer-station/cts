use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    time::Duration,
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
    },
    consts::{
        KiB,
        MiB,
        MANAGEMENT_CANISTER_ID,
        NANOS_IN_A_SECOND,
        SECONDS_IN_AN_HOUR,
        TRILLION,
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
            is_controller,
           
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
use payouts::do_payouts;

mod flush_trade_logs;
use flush_trade_logs::FlushTradeLogStorageError;

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
    trade_logs: VecDeque<TradeLog>,
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
    /*
    position_log_storage_canisters: Vec<PositionLogStorageCanisterData>,
    #[serde(with = "serde_bytes")]
    position_log_storage_buffer: Vec<u8>,
    position_log_storage_flush_lock: bool,
    create_position_log_storage_canister_temp_holder: Option<Principal>,
    flush_position_log_storage_errors: Vec<(FlushPositionLogStorageError, u64/*timestamp_nanos*/)>,
    position_log_storage_canister_code: CanisterCode, 
    */
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
            trade_logs: VecDeque::new(),
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






const STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION: MemoryId = MemoryId::new(0);





// 0.5% fee for maker and taker orders the same. <= till total-trade-volume-cycles on the position: 100_000-T
// 0 < 0.5% <= 100_000;
// 100_000 < 0.3% <= 500_000;
// 500_000 < 0.1% <= 1_000_000;
// 1_000_000 < 0.05% <= 5_000_000;
// 5_000_000 < 0.01%;


//pub const TRADE_FEE_TEN_THOUSANDTHS: u128 = 50;

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
    // trade_mount / 10_000 * TRADE_FEE_TEN_THOUSANDTHS
    
        
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
    
    
    /*
    let trade_fee_ten_thousandths: u128 = match current_position_trade_volume_cycles / TRILLION {
        0..=100_000             => 50,
        100_001..=500_000       => 30,
        500_001..=1_000_000     => 10,
        1_000_001..=5_000_000   => 5,
        _                       => 1,
    };
    */
}



// 




pub fn minimum_tokens_match() -> Tokens {
    10_000/*for the fee ten-thousandths*/ + get(&TOKEN_LEDGER_TRANSFER_FEE) * 100
}




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


mod token_transfer_memo_mod {
    use crate::{PositionKind, PurchaseId};
    const TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-TPP-";
    const CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-CPP-";
    
    const TOKEN_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-TPPF";
    const CYCLES_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START: &[u8; 8] = b"CTS-CPPF";
    
    pub fn position_purchase_token_transfer_memo(position_kind: PositionKind, purchase_id: PurchaseId) -> [u8; 24] {
        create_position_purchase_token_transfer_memo(
            match position_kind {
                PositionKind::Cycles => CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START,
                PositionKind::Token => TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO_START,
            },
            purchase_id   
        )    
    }
    pub fn position_purchase_token_fee_collection_transfer_memo(position_kind: PositionKind, purchase_id: PurchaseId) -> [u8; 24] {
        create_position_purchase_token_transfer_memo(
            match position_kind {
                PositionKind::Cycles => CYCLES_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START,
                PositionKind::Token => TOKEN_POSITION_PURCHASE_TOKEN_FEE_COLLECTION_TRANSFER_MEMO_START,
            },
            purchase_id
        )
        
    }
    fn create_position_purchase_token_transfer_memo(memo_start: &[u8; 8], purchase_id: PurchaseId) -> [u8; 24] {
        let mut b: [u8; 24] = [0u8; 24];
        b[..8].copy_from_slice(memo_start);
        b[8..24].copy_from_slice(&purchase_id.to_be_bytes());
        return b;
    }
}
use token_transfer_memo_mod::*;
    

const TRANSFER_TOKEN_BALANCE_MEMO: &[u8; 29] = b"CTS-CM-TOKEN-BALANCE-TRANSFER";

const MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS: usize = 500;



const FLUSH_TRADE_LOGS_STORAGE_BUFFER_AT_SIZE: usize = 1 * MiB; // can make this bigger 5 or 10 MiB, the flush logic handles flush chunks.

const FLUSH_TRADE_LOGS_STORAGE_BUFFER_CHUNK_SIZE: usize = {
    let before_modulo = 1*MiB+512*KiB; 
    before_modulo - (before_modulo % TradeLog::STABLE_MEMORY_SERIALIZE_SIZE)
};

const CREATE_TRADE_LOG_STORAGE_CANISTER_CYCLES: Cycles = 10_000_000_000_000;


pub const VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS: u128 = SECONDS_IN_AN_HOUR * 1;









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
        "",
        "",        
    ].contains(&&method_name()[..]) {
        accept_message();
    } else {
        trap("this method must be call by a canister or a query call.");
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
            localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, expected_fee.0.clone().try_into().unwrap_or(Tokens::MAX));
            with_mut(&CM_DATA, |cm_data| {
                cm_data.icrc1_token_ledger_transfer_fee = expected_fee.0.clone().try_into().unwrap_or(Tokens::MAX);
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
                cummulator += tl.tokens.saturating_sub(tl.tokens_payout_fee).saturating_sub(tl.token_ledger_transfer_fee()); 
            }
            if tl.token_payout_data.token_fee_collection.is_none() {
                cummulator += tl.tokens_payout_fee.saturating_add(tl.token_ledger_transfer_fee())
            }
            cummulator
        })
}





// -----------------





pub type BuyTokensQuest = MatchTokensQuest;

#[derive(CandidType, Deserialize)]
pub enum BuyTokensError {
    BuyTokensMinimum(Tokens),
    RateCannotBeZero,
    MsgCyclesTooLow,
    CyclesMarketIsFull,
    CyclesMarketIsBusy,
}


#[derive(CandidType, Deserialize)]
pub enum CreateCyclesPositionError{
    CyclesMarketIsBusy,
    PositionsAreFullBumpData{ 
        cycles_positions_lowest_rate: CyclesPerToken, 
        cycles_positions_lowest_rate_tokens: Tokens  
    },
}

#[derive(CandidType, Deserialize)]
pub struct CreateCyclesPositionSuccess {
    pub position_id: PositionId,
}


pub type CreateCyclesPositionResult = Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>;

pub type BuyTokensResult = Result<(Vec<PurchaseId>, Option<CreateCyclesPositionResult>), BuyTokensError>;



#[update(manual_reply = true)]
pub fn buy_tokens(q: BuyTokensQuest) { // -> BuyTokensResult
 
    let caller: Principal = caller();
    
    let buy_tokens_result: BuyTokensResult = buy_tokens_(caller, q);
    
    reply::<(BuyTokensResult,)>((buy_tokens_result,));
    
    ic_cdk::spawn(do_payouts());
    return;   
}



fn buy_tokens_(caller: Principal, q: BuyTokensQuest) -> BuyTokensResult {
    
    if q.tokens < minimum_tokens_match() {
        return Err(BuyTokensError::BuyTokensMinimum(minimum_tokens_match()));
    }    
    
    if q.cycles_per_token_rate == 0 {
        return Err(BuyTokensError::RateCannotBeZero);
    }
    
    let minimum_msg_cycles: Cycles = tokens_transform_cycles(q.tokens, q.cycles_per_token_rate);
    if msg_cycles_available128() < minimum_msg_cycles {
        return Err(BuyTokensError::MsgCyclesTooLow);
    }    
    
    if canister_balance128().checked_add(minimum_msg_cycles).is_none() {
        return Err(BuyTokensError::CyclesMarketIsFull);
    }
    
    with_mut(&CM_DATA, |cm_data| {
        
        if cm_data.trade_logs.len() >= MAX_TRADE_LOGS - 1000 {
            return Err(BuyTokensError::CyclesMarketIsBusy);            
        }
        
        if cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS - 1000 {
            return Err(BuyTokensError::CyclesMarketIsBusy);
        }
        
        let cycles_position_id: PositionId = new_id(&mut cm_data.positions_id_counter); 
        
        let (matches_trade_logs_ids, cycles_position): (Vec<PurchaseId>, CyclesPosition) = match_trades(
            caller, 
            CyclesPosition{
                id: cycles_position_id,
                positor: caller,
                match_tokens_quest: q.clone(),
                current_position_cycles: minimum_msg_cycles,
                purchases_rates_times_cycles_quantities_sum: 0,
                tokens_payouts_fees_sum: 0,
                timestamp_nanos: time_nanos(),
            },
            &mut cm_data.token_positions, 
            &mut cm_data.void_token_positions, 
            &mut cm_data.trade_logs,
            &mut cm_data.trade_logs_id_counter
        ); 
        
        msg_cycles_accept128(minimum_msg_cycles - cycles_position.current_position_cycles);
        
        let mut opt_create_cycles_position_result: Option<CreateCyclesPositionResult> = None;
        
        if cycles_position.current_position_tokens(cycles_position.current_position_available_cycles_per_token_rate()) >= minimum_tokens_match() {
            // create cycles_position. with the match_tokens_mainder
            let create_cycles_position_result: CreateCyclesPositionResult = 'create_cycles_position_block: {
                if cm_data.cycles_positions.len() >= MAX_CYCLES_POSITIONS {
                    if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                        break 'create_cycles_position_block Err(CreateCyclesPositionError::CyclesMarketIsBusy);
                    }
                    // new
                    match cm_data.cycles_positions
                        .iter()
                        .enumerate()
                        .find(
                            |(_i, cp)| { 
                                (
                                    cp.match_tokens_quest.cycles_per_token_rate < cycles_position.match_tokens_quest.cycles_per_token_rate
                                    && cp.current_position_cycles <= cycles_position.current_position_cycles
                                )
                                ||
                                (
                                    cp.match_tokens_quest.cycles_per_token_rate <= cycles_position.match_tokens_quest.cycles_per_token_rate
                                    && cp.current_position_cycles < cycles_position.current_position_cycles 
                                )
                            }
                        )
                        .map(|(i, _cp)| i)
                    {
                        None => {
                            let (cps_lowest_rate, cps_lowest_rate_cycles): (CyclesPerToken, Cycles) = {
                                let p = cm_data.cycles_positions.iter()
                                    .min_by_key(|cycles_position: &&CyclesPosition| { cycles_position.match_tokens_quest.cycles_per_token_rate })
                                    .unwrap();
                                (p.match_tokens_quest.cycles_per_token_rate, p.current_position_cycles)
                            };
                            // higher than the lowest rate and at least as many cycles or higher number cycles and at least the lowest rate will bump
                            break 'create_cycles_position_block Err(CreateCyclesPositionError::PositionsAreFullBumpData{ 
                                cycles_positions_lowest_rate: cps_lowest_rate, 
                                cycles_positions_lowest_rate_tokens: cycles_transform_tokens(cps_lowest_rate_cycles, cps_lowest_rate)
                            });
                        },
                        Some(bump_i) => {
                            // bump,
                            let cycles_position_bump: CyclesPosition = cm_data.cycles_positions.remove(bump_i);
                        
                            let cycles_position_bump_void_cycles_positions_insertion_i = { 
                                cm_data.void_cycles_positions.binary_search_by_key(
                                    &cycles_position_bump.id,
                                    |vcp| { vcp.position_id }
                                ).unwrap_err()
                            };
                            cm_data.void_cycles_positions.insert(
                                cycles_position_bump_void_cycles_positions_insertion_i,
                                cycles_position_bump.into_void_position_type()
                            );
                            
                        }
                    
                    }
                }
                                
                msg_cycles_accept128(cycles_position.current_position_cycles);                                
                                
                cm_data.cycles_positions.push(cycles_position);
                
                
                Ok(CreateCyclesPositionSuccess{
                    position_id: cycles_position_id
                })
            
            };
            
            opt_create_cycles_position_result = Some(create_cycles_position_result);
            
        }
        
        Ok((matches_trade_logs_ids, opt_create_cycles_position_result))

    })
    
}




pub type SellTokensQuest = MatchTokensQuest;

#[derive(CandidType, Deserialize)]
pub enum SellTokensError {
    SellTokensMinimum(Tokens),
    RateCannotBeZero,
    CallerIsInTheMiddleOfADifferentCallThatLocksTheTokenBalance,
    CyclesMarketIsBusy,
    CheckUserCyclesMarketTokenLedgerBalanceError(CallError),
    UserTokenBalanceTooLow{ user_token_balance: Tokens },
}

#[derive(CandidType, Deserialize)]
pub enum CreateTokenPositionError {
    CyclesMarketIsBusy,
    PositionsAreFullBumpData{ 
        token_positions_highest_rate: CyclesPerToken, 
        token_positions_highest_rate_tokens: Tokens  
    }
}

#[derive(CandidType, Deserialize)]
pub struct CreateTokenPositionSuccess {
    position_id: PositionId   
}

pub type CreateTokenPositionResult = Result<CreateTokenPositionSuccess, CreateTokenPositionError>;

pub type SellTokensResult = Result<(Vec<PurchaseId>, Option<CreateTokenPositionResult>), SellTokensError>;


#[update(manual_reply = true)]
pub async fn sell_tokens(q: SellTokensQuest) { // -> SellTokensResult
 
    let caller: Principal = caller();
    
    let sell_tokens_result: SellTokensResult = sell_tokens_(caller, q).await;
    
    reply::<(SellTokensResult,)>((sell_tokens_result,));
    
    ic_cdk::spawn(do_payouts());
    return;   
}



async fn sell_tokens_(caller: Principal, q: SellTokensQuest) -> SellTokensResult {

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
        
        if cm_data.trade_logs.len() >= MAX_TRADE_LOGS - 1000 {
            return Err(SellTokensError::CyclesMarketIsBusy);            
        }

        if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS - 1000 {
            return Err(SellTokensError::CyclesMarketIsBusy);
        }
        
        let token_position_id: PositionId = new_id(&mut cm_data.positions_id_counter); 
    
        let (matches_trade_logs_ids, token_position): (Vec<PurchaseId>, TokenPosition) = match_trades(
            caller, 
            TokenPosition{
                id: token_position_id,
                positor: caller,
                match_tokens_quest: q.clone(),
                current_position_tokens: q.tokens,
                purchases_rates_times_token_quantities_sum: 0,
                cycles_payouts_fees_sum: 0,
                timestamp_nanos: time_nanos(),                
            },
            &mut cm_data.cycles_positions, 
            &mut cm_data.void_cycles_positions, 
            &mut cm_data.trade_logs,
            &mut cm_data.trade_logs_id_counter
        ); 
      
        let mut opt_create_token_position_result: Option<CreateTokenPositionResult> = None;
        
        if token_position.current_position_tokens >= minimum_tokens_match() {
            // create token_position. with the match_tokens_mainder
            let create_token_position_result: CreateTokenPositionResult = 'create_token_position_block: {
            
                if cm_data.token_positions.len() >= MAX_TOKEN_POSITIONS {
                    if cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS {
                        break 'create_token_position_block Err(CreateTokenPositionError::CyclesMarketIsBusy);
                    }
                    // new
                    match cm_data.token_positions
                        .iter()
                        .enumerate()
                        .find(
                            |(_i, tp)| { 
                                (
                                    tp.match_tokens_quest.cycles_per_token_rate > token_position.match_tokens_quest.cycles_per_token_rate
                                    && tp.current_position_tokens <= token_position.current_position_tokens
                                )
                                ||
                                (
                                    tp.match_tokens_quest.cycles_per_token_rate >= token_position.match_tokens_quest.cycles_per_token_rate
                                    && tp.current_position_tokens < token_position.current_position_tokens 
                                )
                            }
                        )
                        .map(|(i, _tp)| i)
                    {
                        None => {
                            let (tps_highest_rate, tps_highest_rate_tokens): (CyclesPerToken, Tokens) = {
                                let p = cm_data.token_positions.iter()
                                    .max_by_key(|token_position: &&TokenPosition| { token_position.match_tokens_quest.cycles_per_token_rate })
                                    .unwrap();
                                (p.match_tokens_quest.cycles_per_token_rate, p.current_position_tokens)
                            };
                            // lower than the highest rate and at least as many tokens or higher number tokens and at most the highest rate will bump
                            break 'create_token_position_block Err(CreateTokenPositionError::PositionsAreFullBumpData{ 
                                token_positions_highest_rate: tps_highest_rate, 
                                token_positions_highest_rate_tokens: tps_highest_rate_tokens  
                            });
                        },
                        Some(bump_i) => {
                            // bump,
                            let token_position_bump: TokenPosition = cm_data.token_positions.remove(bump_i);
                        
                            let token_position_bump_void_token_positions_insertion_i = { 
                                cm_data.void_token_positions.binary_search_by_key(
                                    &token_position_bump.id,
                                    |vtp| { vtp.position_id }
                                ).unwrap_err()
                            };
                            cm_data.void_token_positions.insert(
                                token_position_bump_void_token_positions_insertion_i,
                                token_position_bump.into_void_position_type()
                            );
                        }
                    }
                }
                
                cm_data.token_positions.push(token_position);
                
                Ok(CreateTokenPositionSuccess{
                    position_id: token_position_id
                })
            
            };
            
            opt_create_token_position_result = Some(create_token_position_result);  
            
        }        
        
        Ok((matches_trade_logs_ids, opt_create_token_position_result))
    });
    
    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&caller); });
    
    r
}



#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct MatchTokensQuest {
    tokens: Tokens,
    cycles_per_token_rate: CyclesPerToken
}

fn match_trades<CallerPositionType: CurrentPositionTrait, MatcheePositionType: CurrentPositionTrait>(
    caller: Principal, 
    mut caller_position: CallerPositionType,  
    potential_matches_positions: &mut Vec<MatcheePositionType>, 
    void_positions: &mut Vec<MatcheePositionType::VoidPositionType>,
    trade_logs: &mut VecDeque<TradeLog>, 
    trade_logs_id_counter: &mut PurchaseId,
) -> (Vec<PurchaseId>, CallerPositionType/*caller_position*/) {
                    
    if CallerPositionType::POSITION_KIND == MatcheePositionType::POSITION_KIND {
        trap("CallerPositionType::POSITION_KIND must be the opposite side of the MatcheePositionType::POSITION_KIND");
    }
                    
    // match positions and create a TradeLog for each match.
    let mut matches_trade_logs_ids: Vec<PurchaseId> = Vec::new();    
    let mut match_rate: CyclesPerToken = caller_position.current_position_available_cycles_per_token_rate();
            
    'outer: loop {
        let mut i: usize = 0;
        while i < potential_matches_positions.len() {
            if let Some(trade_rate) = potential_matches_positions[i].is_this_position_better_than_or_equal_to_the_match_rate(match_rate) {
                let matchee_position: &mut MatcheePositionType = &mut potential_matches_positions[i];
                                        
                let purchase_tokens: Tokens = std::cmp::min(caller_position.current_position_tokens(trade_rate), matchee_position.current_position_tokens(trade_rate));
                let caller_position_payout_fee_cycles: Cycles = caller_position.subtract_tokens(purchase_tokens, trade_rate);
                let matchee_position_payout_fee_cycles: Cycles = matchee_position.subtract_tokens(purchase_tokens, trade_rate);
                                                
                let payment_cycles: Cycles = tokens_transform_cycles(purchase_tokens, trade_rate); 
                
                
                let tokens_payout_fee: Tokens = cycles_transform_tokens(
                    {
                        if let PositionKind::Cycles = CallerPositionType::POSITION_KIND {
                            caller_position_payout_fee_cycles
                        } else {
                            matchee_position_payout_fee_cycles
                        }
                    },
                    trade_rate
                );
                let cycles_payout_fee: Cycles = {
                    if let PositionKind::Token = CallerPositionType::POSITION_KIND {
                        caller_position_payout_fee_cycles
                    } else {
                        matchee_position_payout_fee_cycles
                    }
                };
                
                /*        
                if let PositionKind::Token = T::POSITION_KIND {
                    msg_cycles_accept128(payment_cycles);
                }
                */
                
                let trade_log_id: PurchaseId = new_id(trade_logs_id_counter);
                trade_logs.push_back(
                    TradeLog{
                        position_id: matchee_position.id(),
                        id: trade_log_id,
                        positor: matchee_position.positor(),
                        purchaser: caller,
                        tokens: purchase_tokens,
                        cycles: payment_cycles,
                        cycles_per_token_rate: trade_rate,
                        position_kind: MatcheePositionType::POSITION_KIND,
                        timestamp_nanos: time_nanos(),
                        tokens_payout_fee,
                        tokens_payout_ledger_transfer_fees_sum: 0, // starts at 0, gets updated on the payout
                        cycles_payout_fee,
                        cycles_payout_lock: false,
                        token_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        token_payout_data: TokenPayoutData::new_for_a_trade_log()
                    }
                );
                matches_trade_logs_ids.push(trade_log_id);
                
                if matchee_position.current_position_tokens(matchee_position.current_position_available_cycles_per_token_rate()) < minimum_tokens_match() {            
                    // remove position
                    std::mem::drop(matchee_position);
                    let position_for_the_void: MatcheePositionType = potential_matches_positions.remove(i);
                    if position_for_the_void.is_current_position_quantity_0() == false {
                        // position into void_position                             
                        let position_for_the_void_void_positions_insertion_i: usize = { 
                            void_positions.binary_search_by_key(
                                &position_for_the_void.id(),
                                |void_position| { void_position.position_id() }
                            ).unwrap_err()
                        };
                        void_positions.insert(
                            position_for_the_void_void_positions_insertion_i,
                            position_for_the_void.into_void_position_type()
                        );
                    }
                } else {
                    i = i + 1;
                }
                
                if caller_position.current_position_tokens(caller_position.current_position_available_cycles_per_token_rate()) < minimum_tokens_match() {
                    break 'outer;
                }    
                
            }
        }
        
        // add up wheight[ed] average of the better_rates_trades and set higher match_rate can balance out the better_rates. 
        /*
        let balance_rate: CyclesPerToken = {
            let purchase_tokens_sum = q.tokens - match_tokens_mainder;
            let average_rate_of_purchase_tokens = purchase_rate_times_quantity_sum / purchase_tokens_sum;
            (q.cycles_per_token_rate * q.tokens - (average_rate_of_purchase_tokens * purchase_tokens_sum)) / match_tokens_mainder
        };
        */
        let balance_rate: CyclesPerToken = caller_position.current_position_available_cycles_per_token_rate();
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
        };
    }
                
    (matches_trade_logs_ids, caller_position)
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
            if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                return Err(VoidPositionError::CyclesMarketIsBusy);
            }
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_i);
            let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = cm_data.void_cycles_positions.binary_search_by_key(&cycles_position_for_the_void.id, |vcp| { vcp.position_id }).unwrap_err();
            cm_data.void_cycles_positions.insert(
                cycles_position_for_the_void_void_cycles_positions_insertion_i,
                cycles_position_for_the_void.into_void_position_type()
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
                token_position_for_the_void.into_void_position_type()
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
    /*
    let user_token_ledger_balance: Tokens = match check_user_cycles_market_token_ledger_balance(&user_id).await {
        Ok(token_ledger_balance) => token_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });
            return Err(TransferTokenBalanceError::CheckUserCyclesMarketTokenLedgerBalanceCallError((call_error.0 as u32, call_error.1)));            
        }
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &user_id) });
    
    let usable_user_token_balance: Tokens = user_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock);
    */
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


fn view_positions<T: CandidType + CurrentPositionTrait>(q: ViewPositionsQuest, positions: &Vec<T>) {
    
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
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.trade_logs.make_contiguous();
    });

    with(&CM_DATA, |cm_data| {
        
        let trade_logs_len: u128 = cm_data.trade_logs_id_counter;        
        
        let mut logs_bytes: Vec<u8> = Vec::new();

        let first_trade_log_id_on_this_canister: Option<PurchaseId>/*none if there are no trade-logs on this canister*/ = if cm_data.trade_log_storage_buffer.len() >= TradeLog::STABLE_MEMORY_SERIALIZE_SIZE {
            Some(u128::from_be_bytes((&cm_data.trade_log_storage_buffer[16..32]).try_into().unwrap()))
        } else {
            cm_data.trade_logs.front().map(|l| l.id)
        };
        if let Some(first_trade_log_id_on_this_canister) = first_trade_log_id_on_this_canister {
            
            let last_trade_log_id_on_this_canister: PurchaseId = {
                cm_data.trade_logs.back().map(|p| p.id)
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
                let cm_data_trade_logs_till_stop = &cm_data.trade_logs.as_slices().0[..cm_data_trade_logs_till_i];
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
pub fn cm_message_cycles_position_purchase_purchaser_cmcaller_callback(q: CMCallbackQuest) -> () {
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
    ic_cdk::spawn(do_payouts());
    return;
}

#[update(manual_reply = true)]
pub fn cm_message_cycles_position_purchase_positor_cmcaller_callback(q: CMCallbackQuest) {
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
    ic_cdk::spawn(do_payouts());
    return;
}

#[update(manual_reply = true)]
pub fn cm_message_token_position_purchase_purchaser_cmcaller_callback(q: CMCallbackQuest) {
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
    ic_cdk::spawn(do_payouts());
    return;
}

#[update(manual_reply = true)]
pub fn cm_message_token_position_purchase_positor_cmcaller_callback(q: CMCallbackQuest) {
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
    ic_cdk::spawn(do_payouts());
    return;
}

#[update(manual_reply = true)]
pub fn cm_message_void_cycles_position_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
    
    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(void_cycles_position_void_cycles_positions_i) = cm_data.void_cycles_positions.binary_search_by_key(&q.cm_call_id, |void_cycles_position| { void_cycles_position.position_id }) {
            cm_data.void_cycles_positions[void_cycles_position_void_cycles_positions_i]
            .cycles_payout_data
            .cmcaller_cycles_payout_callback_complete = Some((cycles_transfer_refund, q.opt_call_error));
            
            if cm_data.void_cycles_positions[void_cycles_position_void_cycles_positions_i]
            .cycles_payout_data
            .is_complete() {
                cm_data.void_cycles_positions.remove(void_cycles_position_void_cycles_positions_i);
            }
        }
    });
    
    reply::<()>(());
    ic_cdk::spawn(do_payouts());
    return;
}

#[update(manual_reply = true)]
pub fn cm_message_void_token_position_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    with_mut(&CM_DATA, |cm_data| {
        if let Ok(void_token_position_void_token_positions_i) = cm_data.void_token_positions.binary_search_by_key(&q.cm_call_id, |void_token_position| { void_token_position.position_id }) {
            cm_data.void_token_positions[void_token_position_void_token_positions_i]
            .token_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
            
            if cm_data.void_token_positions[void_token_position_void_token_positions_i]
            .token_payout_data
            .is_complete() {
                cm_data.void_token_positions.remove(void_token_position_void_token_positions_i);
            }
        }
    });
    
    reply::<()>(());
    ic_cdk::spawn(do_payouts());
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




