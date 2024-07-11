use candid::{Principal, CandidType, Deserialize};
use crate::icrc::{IcrcId, Tokens, Icrc1TransferError, BlockId, IcrcSubaccount};
use crate::types::{Cycles, CallError, canister_code::CanisterCode};
use crate::consts::KiB;
use serde::Serialize;
use std::collections::{HashSet, VecDeque, BTreeMap};

pub mod storage_logs;
use storage_logs::{
    LogStorageInit,
    trade_log::{TradeLog, PayoutData},
    position_log::PositionLog,
};


pub const MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA: usize = 512*KiB*3 / std::mem::size_of::<LatestTradesDataItem>();


pub type PositionId = u128;
pub type PurchaseId = u128;
pub type CyclesPerToken = Cycles;

#[derive(Copy, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum PositionKind {
    Cycles,
    Token
}

#[derive(CandidType, Deserialize)]
pub struct CMIcrc1TokenTradeContractInit {
    pub cts_id: Principal,
    pub cm_main_id: Principal,
    pub icrc1_token_ledger: Principal,
    pub icrc1_token_ledger_transfer_fee: Tokens,
    pub cycles_bank_id: Principal,
    pub cycles_bank_transfer_fee: Cycles,
    pub trades_storage_canister_code: CanisterCode,
    pub positions_storage_canister_code: CanisterCode,
    pub shareholder_payouts_canister_id: Principal,
}

// ----

#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct TradeCyclesQuest {
    pub cycles: Cycles,
    pub cycles_per_token_rate: CyclesPerToken,
    pub posit_transfer_ledger_fee: Option<Cycles>,
    pub return_cycles_to_subaccount: Option<IcrcSubaccount>,
    pub payout_tokens_to_subaccount: Option<IcrcSubaccount>,
}

#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct TradeTokensQuest {
    pub tokens: Tokens,
    pub cycles_per_token_rate: CyclesPerToken,
    pub posit_transfer_ledger_fee: Option<Tokens>,
    pub return_tokens_to_subaccount: Option<IcrcSubaccount>,
    pub payout_cycles_to_subaccount: Option<IcrcSubaccount>,
}

#[derive(CandidType, Deserialize)]
pub struct TradeSuccess {
    pub position_id: PositionId,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum TradeError {
    MinimumPosition{ minimum_cycles: Cycles, minimum_tokens: Tokens},
    RateCannotBeZero,
    CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance,
    CyclesMarketIsBusy,
    CreatePositionLedgerTransferCallError(CallError),
    CreatePositionLedgerTransferError(Icrc1TransferError)
}

pub type TradeResult = Result<TradeSuccess, TradeError>;

// ---

#[derive(CandidType, Deserialize, Debug)]
pub struct VoidPositionQuest {
    pub position_id: PositionId
}

#[derive(CandidType, Deserialize)]
pub enum VoidPositionError {
    WrongCaller,
    MinimumWaitTime{ minimum_wait_time_seconds: u128, position_creation_timestamp_seconds: u128 },
    CyclesMarketIsBusy,
    PositionNotFound,
}

pub type VoidPositionResult = Result<(), VoidPositionError>;

// ----

#[derive(CandidType, Deserialize, Debug)]
pub struct TransferBalanceQuest {
    pub amount: u128,
    pub ledger_transfer_fee: Option<u128>,   
    pub to: IcrcId
}

#[derive(CandidType, Deserialize)]
pub enum TransferBalanceError {
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance,
    TransferCallError(CallError),
    TransferError(Icrc1TransferError)
}

pub type TransferBalanceResult = Result<BlockId, TransferBalanceError>;

// ----

#[derive(CandidType, Deserialize)]
pub struct ViewPositionBookQuest {
    pub opt_start_greater_than_rate: Option<CyclesPerToken>
}
#[derive(CandidType, Deserialize)]
pub struct ViewPositionBookSponse {
    pub positions_quantities: Vec<(CyclesPerToken, u128)>, 
    pub is_last_chunk: bool,
}

// ------

#[derive(CandidType, Deserialize)]
pub struct ViewLatestTradesQuest {
    pub opt_start_before_id: Option<PurchaseId>,
}

pub type LatestTradesDataItem = (PurchaseId, Tokens, CyclesPerToken, u64);

#[derive(CandidType, Deserialize)]
pub struct ViewLatestTradesSponse {
    pub trades_data: Vec<LatestTradesDataItem>, 
    pub is_last_chunk_on_this_canister: bool,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct StorageCanister {
    // The id of the first log in this storage-canister
    pub first_log_id : u128,
    // The number of logs in this storage-canister
    pub length : u128,
    // the size of the log-serialization-format in this storage-canister.  
    pub log_size: u32,
    pub canister_id : Principal,
}

// ---------

#[derive(Default, Serialize, Deserialize, CandidType, Clone, Debug, PartialEq, Eq)]
pub struct Candle {
    pub time_nanos: u64, // of the time-period start
    pub volume_cycles: Cycles,
    pub volume_tokens: Tokens,
    pub open_rate: CyclesPerToken,
    pub high_rate: CyclesPerToken,
    pub low_rate: CyclesPerToken,
    pub close_rate: CyclesPerToken,
}

#[derive(CandidType, Deserialize)]
pub struct ViewCandlesQuest {
    pub opt_start_before_time_nanos: Option<u64>,
}

#[derive(CandidType)]
pub struct ViewCandlesSponse<'a> {
    pub candles: &'a [Candle],
    pub is_earliest_chunk: bool,
}

#[derive(CandidType, Deserialize)]
pub struct ViewCandlesSponseOwned {
    pub candles: Vec<Candle>,   
    pub is_earliest_chunk: bool,
}

#[derive(CandidType, Deserialize)]
pub struct ViewVolumeStatsSponse {
    pub volume_cycles: Volume,
    pub volume_tokens: Volume,
}
#[derive(CandidType, Deserialize)]
pub struct Volume{
    pub volume_24_hour: u128,
    pub volume_7_day: u128,
    pub volume_30_day: u128,
    pub volume_sum: u128,
}

#[derive(CandidType, Deserialize)]
pub struct ShareholderPayoutsCollectTradeFeesSponse {
    pub cb_cycles_sent: Cycles, // transferred at the cts-cycles-bank.
    pub tokens_sent: Tokens,
}

// ---------

#[derive(CandidType, Serialize, Deserialize)]
pub struct CMData {
    pub cts_id: Principal,
    pub cm_main_id: Principal,
    pub icrc1_token_ledger: Principal,
    pub icrc1_token_ledger_transfer_fee: Tokens,
    pub cycles_bank_id: Principal,
    pub cycles_bank_transfer_fee: Cycles,
    pub positions_id_counter: u128,
    pub trade_logs_id_counter: u128,
    pub mid_call_user_cycles_balance_locks: HashSet<Principal>,
    pub mid_call_user_token_balance_locks: HashSet<Principal>,
    pub cycles_positions: BTreeMap<PositionId, CyclesPosition>,
    pub token_positions: BTreeMap<PositionId, TokenPosition>,
    pub trade_logs: VecDeque<TradeLogAndTemporaryData>,
    pub void_cycles_positions: BTreeMap<PositionId, VoidCyclesPosition>,
    pub void_token_positions: BTreeMap<PositionId, VoidTokenPosition>,
    pub do_payouts_errors: Vec<CallError>,
    pub candle_counter: CandleCounter,
    pub shareholder_payouts_canister_id: Principal,
    pub trade_fees_collection_counter: TradeFeesCollectionCounter, 
}

impl CMData {
    pub fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            cm_main_id: Principal::from_slice(&[]),
            icrc1_token_ledger: Principal::from_slice(&[]),
            icrc1_token_ledger_transfer_fee: 0,
            cycles_bank_id: Principal::from_slice(&[]),
            cycles_bank_transfer_fee: 0,
            positions_id_counter: 0,
            trade_logs_id_counter: 0,
            mid_call_user_cycles_balance_locks: HashSet::new(),
            mid_call_user_token_balance_locks: HashSet::new(),
            cycles_positions: BTreeMap::new(),
            token_positions: BTreeMap::new(),
            trade_logs: VecDeque::new(),
            void_cycles_positions: BTreeMap::new(),
            void_token_positions: BTreeMap::new(),
            do_payouts_errors: Vec::new(),
            candle_counter: CandleCounter::default(),
            shareholder_payouts_canister_id: Principal::from_slice(&[]),
            trade_fees_collection_counter: TradeFeesCollectionCounter::default(),
        }
    }
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct LogStorageData {
    pub storage_canisters: Vec<StorageCanisterData>,
    #[serde(with = "serde_bytes")]
    pub storage_buffer: Vec<u8>,
    pub storage_flush_lock: bool,
    pub create_storage_canister_temp_holder: Option<Principal>,
    pub flush_storage_errors: Vec<(FlushLogsStorageError, u64/*timestamp_nanos*/)>,
    pub storage_canister_code: CanisterCode,
    pub storage_canister_init: LogStorageInit,
}
impl LogStorageData {
    pub fn new(storage_canister_init: LogStorageInit) -> Self {
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
#[derive(CandidType, Serialize, Deserialize)]
pub struct StorageCanisterData {
    pub log_size: u32,
    pub first_log_id: u128,
    pub length: u64, // number of logs current store on this storage canister
    pub is_full: bool,
    pub canister_id: Principal,
    pub creation_timestamp: u128, // set once when storage canister is create.
    pub module_hash: [u8; 32] // update this field when upgrading the storage canisters.
}

#[derive(CandidType, Serialize, Deserialize)]
pub enum FlushLogsStorageError {
    CreateStorageCanisterError(CreateStorageCanisterError),
    StorageCanisterCallError(CallError),
    NewStorageCanisterIsFull, // when a *new* trade-log-storage-canister returns StorageIsFull on the first flush call. 
}

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub enum CreateStorageCanisterError {
    CyclesBalanceTooLow{ cycles_balance: Cycles },
    CreateCanisterCallError(CallError),
    InstallCodeCandidError(String),
    InstallCodeCallError(CallError),
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct CyclesPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub quest: TradeCyclesQuest,
    pub current_position_cycles: Cycles,
    pub purchases_rates_times_cycles_quantities_sum: u128,
    pub fill_quantity_tokens: Tokens,
    pub tokens_payouts_fees_sum: Tokens,
    pub timestamp_nanos: u128,
}

#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TokenPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub quest: TradeTokensQuest,
    pub current_position_tokens: Tokens,
    pub purchases_rates_times_token_quantities_sum: u128,
    pub cycles_payouts_fees_sum: Cycles,
    pub timestamp_nanos: u128,
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TradeLogTemporaryData {
    pub cycles_payout_lock: bool,
    pub token_payout_lock: bool,
    pub payout_cycles_to_subaccount: Option<IcrcSubaccount>,
    pub payout_tokens_to_subaccount: Option<IcrcSubaccount>,
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TradeLogAndTemporaryData {
    pub log: TradeLog,
    pub temporary_data: TradeLogTemporaryData,
}

impl TradeLogAndTemporaryData {
    pub fn can_move_into_the_stable_memory_for_the_long_term_storage(&self) -> bool {
        self.temporary_data.cycles_payout_lock == false
        && self.temporary_data.token_payout_lock == false
        && self.log.cycles_payout_data.is_some()
        && self.log.token_payout_data.is_some()
    }
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct VPUpdateStoragePositionData {
    pub lock: bool,
    pub status: bool,
    pub update_storage_position_log: PositionLog,
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct VoidCyclesPosition {
    pub position_id: PositionId,
    pub positor: Principal,
    pub cycles: Cycles,
    pub cycles_payout_lock: bool,
    pub cycles_payout_data: Option<PayoutData>,
    pub timestamp_nanos: u128,
    pub update_storage_position_data: VPUpdateStoragePositionData,
    pub return_cycles_to_subaccount: Option<IcrcSubaccount>,
}

#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct VoidTokenPosition {
    pub position_id: PositionId,
    pub tokens: Tokens,
    pub positor: Principal,
    pub token_payout_lock: bool,
    pub token_payout_data: Option<PayoutData>,
    pub timestamp_nanos: u128,
    pub update_storage_position_data: VPUpdateStoragePositionData,    
    pub return_tokens_to_subaccount: Option<IcrcSubaccount>,
}

#[derive(Default, CandidType, Serialize, Deserialize)]
pub struct CandleCounter {
    pub segments_1_minute: Vec<Candle>,   // last item is the latest_one_minute
    pub volume_cycles: Cycles,            // all-time
    pub volume_tokens: Tokens,            // all-time
}


#[derive(Default, CandidType, Serialize, Deserialize)]
pub struct TradeFeesCollectionCounter {
    pub new_token_trade_fees_collection: Tokens, // the token fees collected that have not been transferred to the shareholder-payouts canister yet. 
    pub new_cycles_trade_fees_collection: Cycles, // the cycles fees collected that have not been transferred to the shareholder-payouts canister yet.
    pub total_token_trade_fees_collection: Tokens, // the total amount of token fees collected of all time including those transferred to the shareholder-payouts canister.
    pub total_cycles_trade_fees_collection: Cycles, // the total amount of cycles fees collected of all time including those transferred to the shareholder-payouts canister.
}
impl TradeFeesCollectionCounter {
    pub fn count_trade(&mut self, tl: &TradeLog) {
        self.new_token_trade_fees_collection = self.new_token_trade_fees_collection.saturating_add(tl.tokens_payout_fee);
        self.new_cycles_trade_fees_collection = self.new_cycles_trade_fees_collection.saturating_add(tl.cycles_payout_fee);
        self.total_token_trade_fees_collection = self.total_token_trade_fees_collection.saturating_add(tl.tokens_payout_fee);
        self.total_cycles_trade_fees_collection = self.total_cycles_trade_fees_collection.saturating_add(tl.cycles_payout_fee);
    }
}



