use candid::{Principal, CandidType, Deserialize};
use crate::icrc::{IcrcId, Tokens, Icrc1TransferError, BlockId, IcrcSubaccount};
use crate::types::{Cycles, CallError, canister_code::CanisterCode};
use crate::consts::{KiB, MINUTES_IN_A_HOUR};
use serde::Serialize;


pub mod storage_logs;

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

// number is the length in minutes of this segment
#[derive(CandidType, Deserialize, PartialEq, Eq, Copy, Clone)]
#[repr(u64)]
pub enum ViewCandlesSegmentLength {
    OneMinute = 1,
    FiveMinute = 5,
    FifteenMinute = 15,
    ThirtyMinute = 30,
    OneHour = MINUTES_IN_A_HOUR as u64,
    TwoHour = MINUTES_IN_A_HOUR as u64 * 2,
    SixHour = MINUTES_IN_A_HOUR as u64 * 6,
    TwentyFourHour = MINUTES_IN_A_HOUR as u64 * 24,    
}

#[derive(CandidType, Deserialize)]
pub struct ViewCandlesQuest {
    pub segment_length: ViewCandlesSegmentLength,
    pub opt_start_before_time_nanos: Option<u64>,
}

#[derive(CandidType)]
pub struct ViewCandlesSponse<'a> {
    pub candles: &'a [Candle],
}

#[derive(CandidType, Deserialize)]
pub struct ViewCandlesSponseOwned {
    pub candles: Vec<Candle>,   
}

// ---------