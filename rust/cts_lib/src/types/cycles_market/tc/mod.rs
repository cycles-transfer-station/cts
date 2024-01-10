use candid::{Principal, CandidType, Deserialize};
use crate::icrc::{IcrcId, Tokens, TokenTransferError, BlockId};
use crate::types::{Cycles, CallError, canister_code::CanisterCode};
use crate::consts::KiB;
use serde::Serialize;

pub type PositionId = u128;
pub type PurchaseId = u128;
pub type CyclesPerToken = Cycles;

#[derive(CandidType, Deserialize)]
pub struct CMIcrc1TokenTradeContractInit {
    pub cts_id: Principal,
    pub cm_main_id: Principal,
    pub icrc1_token_ledger: Principal,
    pub icrc1_token_ledger_transfer_fee: Tokens,
    pub trades_storage_canister_code: CanisterCode,
    pub positions_storage_canister_code: CanisterCode,
}

// ----



#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BuyTokensQuest {
    pub cycles: Cycles,
    pub cycles_per_token_rate: CyclesPerToken,
}

pub type TradeCyclesQuest = BuyTokensQuest;

#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct SellTokensQuest {
    pub tokens: Tokens,
    pub cycles_per_token_rate: CyclesPerToken,
    pub posit_transfer_ledger_fee: Option<Tokens>,
}



#[derive(CandidType, Deserialize, Debug)]
pub enum BuyTokensError {
    MinimumPosition{ minimum_cycles: Cycles, minimum_tokens: Tokens},
    RateCannotBeZero,
    MsgCyclesTooLow,
    CyclesMarketIsBusy,
}
pub type TradeCyclesError = BuyTokensError;

#[derive(CandidType, Deserialize)]
pub struct BuyTokensSuccess {
    pub position_id: PositionId,
}
pub type TradeCyclesSuccess = BuyTokensSuccess;

pub type BuyTokensResult = Result<BuyTokensSuccess, BuyTokensError>;

pub type TradeCyclesResult = BuyTokensResult;




#[derive(CandidType, Deserialize, Debug)]
pub enum SellTokensError {
    MinimumPosition{ minimum_cycles: Cycles, minimum_tokens: Tokens},
    RateCannotBeZero,
    CallerIsInTheMiddleOfADifferentCallThatLocksTheTokenBalance,
    CyclesMarketIsBusy,
    CollectTokensForThePositionLedgerTransferCallError(CallError),
    CollectTokensForThePositionLedgerTransferError(TokenTransferError)
}


#[derive(CandidType, Deserialize)]
pub struct SellTokensSuccess {
    pub position_id: PositionId,
    //sell_tokens_so_far: Tokens,
    //cycles_payout_so_far: Cycles,
    //position_closed: bool
}


pub type SellTokensResult = Result<SellTokensSuccess, SellTokensError>;



#[derive(CandidType, Deserialize)]
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

#[derive(CandidType, Deserialize)]
pub struct TransferTokenBalanceQuest {
    pub tokens: Tokens,
    pub token_fee: Tokens, // must set and cant be opt, bc the contract must check that the user has the available balance unlocked and must know the amount + fee is available (not locked) in the account.   
    pub to: IcrcId,
    pub created_at_time: Option<u64>
}

#[derive(CandidType, Deserialize)]
pub enum TransferTokenBalanceError {
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfADifferentCallThatLocksTheTokenBalance,
    TokenTransferCallError((u32, String)),
    TokenTransferError(TokenTransferError)
}

pub type TransferTokenBalanceResult = Result<BlockId, TransferTokenBalanceError>;

// ----

#[derive(CandidType, Serialize, Deserialize)]
pub struct CMVoidCyclesPositionPositorMessageQuest {
    pub position_id: PositionId,
    // cycles in the call
    pub timestamp_nanos: u128
}


#[derive(CandidType, Serialize, Deserialize)]
pub struct CMTradeTokensCyclesPayoutMessageQuest {
    pub token_position_id: PositionId,
    pub purchase_id: PurchaseId,
}


// ---------------





pub mod trade_log; 
pub mod position_log;



pub const MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA: usize = 512*KiB*3 / std::mem::size_of::<LatestTradesDataItem>();



#[derive(Copy, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum PositionKind {
    Cycles,
    Token
}


#[derive(CandidType, Deserialize)]
pub struct ViewLatestTradesQuest {
    pub opt_start_before_id: Option<PurchaseId>,
}

pub type LatestTradesDataItem = (PurchaseId, Tokens, CyclesPerToken, u64, PositionKind);

#[derive(CandidType, Deserialize)]
pub struct ViewLatestTradesSponse {
    pub trades_data: Vec<LatestTradesDataItem>, 
    pub is_last_chunk_on_this_canister: bool,
}



