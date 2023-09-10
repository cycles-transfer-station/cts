use candid::{Principal, CandidType, Deserialize};
use crate::icrc::{IcrcId, Tokens, TokenTransferError, BlockId};
use crate::types::{Cycles,canister_code::CanisterCode};
use crate::consts::KiB;
use serde::Serialize;

pub type PositionId = u128;
pub type PurchaseId = u128;
pub type CyclesPerToken = Cycles;

#[derive(CandidType, Deserialize)]
pub struct CMIcrc1TokenTradeContractInit {
    pub cts_id: Principal,
    pub cm_main_id: Principal,
    pub cm_caller: Principal,
    pub icrc1_token_ledger: Principal,
    pub icrc1_token_ledger_transfer_fee: Tokens,
    pub trades_storage_canister_code: CanisterCode,
    pub positions_storage_canister_code: CanisterCode,
}

// ----
/*
#[derive(CandidType, Serialize, Deserialize)]
pub struct CreateCyclesPositionQuest {
    pub cycles: Cycles,
    pub minimum_purchase: Cycles,
    pub cycles_per_token_rate: CyclesPerToken,
}

#[derive(CandidType, Deserialize)]
pub enum CreateCyclesPositionError{
    MinimumPurchaseMustBeEqualOrLessThanTheCyclesPosition,
    MsgCyclesTooLow{ create_position_fee: Cycles },
    CyclesMustBeAMultipleOfTheCyclesPerTokenRate,
    MinimumPurchaseMustBeAMultipleOfTheCyclesPerTokenRate,
    CyclesMarketIsBusy,
    CyclesMarketIsFull,
    #[allow(non_camel_case_types)]
    CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: CyclesPerToken, minimum_cycles_position_for_a_bump: Cycles },
    MinimumCyclesPosition(Cycles),
    MinimumPurchaseCannotBeZero
}

#[derive(CandidType, Deserialize)]
pub struct CreateCyclesPositionSuccess {
    pub position_id: PositionId,
}

pub type CreateCyclesPositionResult = Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>;

// ----

#[derive(CandidType, Serialize, Deserialize)]
pub struct CreateTokenPositionQuest {
    pub tokens: Tokens,
    pub minimum_purchase: Tokens,
    pub cycles_per_token_rate: CyclesPerToken,
}

#[derive(CandidType, Deserialize)]
pub enum CreateTokenPositionError {
    MinimumPurchaseMustBeEqualOrLessThanTheTokenPosition,
    MsgCyclesTooLow{ create_position_fee: Cycles },
    CyclesMarketIsFull,
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall,
    CheckUserCyclesMarketTokenLedgerBalanceError((u32, String)),
    UserTokenBalanceTooLow{ user_token_balance: Tokens },
    #[allow(non_camel_case_types)]
    CyclesMarketIsFull_MaximumRateAndMinimumTokenPositionForABump{ maximum_rate_for_a_bump: CyclesPerToken, minimum_token_position_for_a_bump: Tokens },
    MinimumTokenPosition(Tokens),
    MinimumPurchaseCannotBeZero
}

#[derive(CandidType, Deserialize)]
pub struct CreateTokenPositionSuccess {
    pub position_id: PositionId
}
    
pub type CreateTokenPositionResult = Result<CreateTokenPositionSuccess, CreateTokenPositionError>;

// ----

#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesPositionQuest {
    pub cycles_position_id: PositionId,
    pub cycles: Cycles
}

#[derive(CandidType, Deserialize)]
pub enum PurchaseCyclesPositionError {
    MsgCyclesTooLow{ purchase_position_fee: Cycles },
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall,
    CheckUserCyclesMarketTokenLedgerBalanceError((u32, String)),
    UserTokenBalanceTooLow{ user_token_balance: Tokens },
    CyclesPositionNotFound,
    CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: Cycles },
    CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: Cycles },
    PurchaseCyclesMustBeAMultipleOfTheCyclesPerTokenRate,
}

#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesPositionSuccess {
    pub purchase_id: PurchaseId,
}

pub type PurchaseCyclesPositionResult = Result<PurchaseCyclesPositionSuccess, PurchaseCyclesPositionError>;

// ----

#[derive(CandidType, Deserialize)]
pub struct PurchaseTokenPositionQuest {
    pub token_position_id: PositionId,
    pub tokens: Tokens
}

#[derive(CandidType, Deserialize)]
pub enum PurchaseTokenPositionError {
    MsgCyclesTooLow{ purchase_position_fee: Cycles },
    CyclesMarketIsBusy,
    TokenPositionNotFound,
    TokenPositionTokensIsLessThanThePurchaseQuest{ token_position_tokens: Tokens },
    TokenPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ token_position_minimum_purchase: Tokens }
}

#[derive(CandidType, Deserialize)]
pub struct PurchaseTokenPositionSuccess {
    pub purchase_id: PurchaseId
}

pub type PurchaseTokenPositionResult = Result<PurchaseTokenPositionSuccess, PurchaseTokenPositionError>;
*/
// ----

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
    CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall,
    CheckUserCyclesMarketTokenLedgerBalanceCallError((u32, String)),
    UserTokenBalanceTooLow{ user_token_balance: Tokens },
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
pub struct CMVoidTokenPositionPositorMessageQuest {
    pub position_id: PositionId,
    pub void_tokens: Tokens,
    pub timestamp_nanos: u128
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct CMCyclesPositionPurchasePositorMessageQuest {
    pub cycles_position_id: PositionId,
    pub purchase_id: PurchaseId,
    pub purchaser: Principal,
    pub purchase_timestamp_nanos: u128,
    pub cycles_purchase: Cycles,
    pub cycles_position_cycles_per_token_rate: CyclesPerToken,
    pub token_payment: Tokens,
    pub token_transfer_block_height: BlockId,
    pub token_transfer_timestamp_nanos: u128,
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct CMCyclesPositionPurchasePurchaserMessageQuest {
    pub cycles_position_id: PositionId,
    pub cycles_position_positor: Principal,
    pub cycles_position_cycles_per_token_rate: CyclesPerToken,
    pub purchase_id: PurchaseId,
    pub purchase_timestamp_nanos: u128,
    // cycles in the call
    pub token_payment: Tokens,
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct CMTokenPositionPurchasePositorMessageQuest {
    pub token_position_id: PositionId,
    pub token_position_cycles_per_token_rate: CyclesPerToken,
    pub purchaser: Principal,
    pub purchase_id: PurchaseId,
    pub token_purchase: Tokens,
    pub purchase_timestamp_nanos: u128,
    // cycles in the call
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct CMTokenPositionPurchasePurchaserMessageQuest {
    pub token_position_id: PositionId,
    pub purchase_id: PurchaseId, 
    pub positor: Principal,
    pub purchase_timestamp_nanos: u128,
    pub cycles_payment: Cycles,
    pub token_position_cycles_per_token_rate: CyclesPerToken,
    pub token_purchase: Tokens,
    pub token_transfer_block_height: BlockId,
    pub token_transfer_timestamp_nanos: u128,
}


// ---------------





pub mod trade_log; 
pub mod position_log;



pub const MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA: usize = 512*KiB*3 / std::mem::size_of::<LatestTradesDataItem>();



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



