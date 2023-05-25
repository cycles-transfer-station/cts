pub mod cm_main;
pub mod icrc1token_trade_contract;
pub mod cm_caller;


/*


use super::{CandidType, Deserialize, Cycles, XdrPerMyriadPerIcp};
use crate::ic_ledger_types::{IcpTokens, IcpBlockHeight, IcpTransferError, IcpId};
use ic_cdk::export::Principal;

pub type PositionId = u128;
pub type PurchaseId = u128;

#[derive(CandidType, Deserialize)]
pub struct CreateCyclesPositionQuest {
    pub cycles: Cycles,
    pub minimum_purchase: Cycles,
    pub xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
}

#[derive(CandidType, Deserialize)]
pub enum CreateCyclesPositionError{
    MinimumPurchaseMustBeEqualOrLessThanTheCyclesPosition,
    MsgCyclesTooLow{ create_position_fee: Cycles },
    CyclesMustBeAMultipleOfTheXdrPerMyriadPerIcpRate,
    MinimumPurchaseMustBeAMultipleOfTheXdrPerMyriadPerIcpRate,
    CyclesMarketIsBusy,
    CyclesMarketIsFull,
    #[allow(non_camel_case_types)]
    CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: XdrPerMyriadPerIcp, minimum_cycles_position_for_a_bump: Cycles },
    MinimumCyclesPosition(Cycles),
    MinimumPurchaseCannotBeZero
}

#[derive(CandidType, Deserialize)]
pub struct CreateCyclesPositionSuccess {
    pub position_id: PositionId,
}

pub type CreateCyclesPositionResult = Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>;

#[derive(CandidType, Deserialize)]
pub struct CreateIcpPositionQuest {
    pub icp: IcpTokens,
    pub minimum_purchase: IcpTokens,
    pub xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
}

#[derive(CandidType, Deserialize)]
pub enum CreateIcpPositionError {
    MinimumPurchaseMustBeEqualOrLessThanTheIcpPosition,
    MsgCyclesTooLow{ create_position_fee: Cycles },
    CyclesMarketIsFull,
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
    CheckUserCyclesMarketIcpLedgerBalanceError((u32, String)),
    UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
    #[allow(non_camel_case_types)]
    CyclesMarketIsFull_MaximumRateAndMinimumIcpPositionForABump{ maximum_rate_for_a_bump: XdrPerMyriadPerIcp, minimum_icp_position_for_a_bump: IcpTokens },
    MinimumIcpPosition(IcpTokens),
    MinimumPurchaseCannotBeZero
}

#[derive(CandidType, Deserialize)]
pub struct CreateIcpPositionSuccess {
    pub position_id: PositionId
}
    
pub type CreateIcpPositionResult = Result<CreateIcpPositionSuccess, CreateIcpPositionError>;

#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesPositionQuest {
    pub cycles_position_id: PositionId,
    pub cycles: Cycles
}

#[derive(CandidType, Deserialize)]
pub enum PurchaseCyclesPositionError {
    MsgCyclesTooLow{ purchase_position_fee: Cycles },
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
    CheckUserCyclesMarketIcpLedgerBalanceError((u32, String)),
    UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
    CyclesPositionNotFound,
    CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: Cycles },
    CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: Cycles },
    PurchaseCyclesMustBeAMultipleOfTheXdrPerMyriadPerIcpRate,
}

#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesPositionSuccess {
    pub purchase_id: PurchaseId,
}

pub type PurchaseCyclesPositionResult = Result<PurchaseCyclesPositionSuccess, PurchaseCyclesPositionError>;

#[derive(CandidType, Deserialize)]
pub struct PurchaseIcpPositionQuest {
    pub icp_position_id: PositionId,
    pub icp: IcpTokens
}

#[derive(CandidType, Deserialize)]
pub enum PurchaseIcpPositionError {
    MsgCyclesTooLow{ purchase_position_fee: Cycles },
    CyclesMarketIsBusy,
    IcpPositionNotFound,
    IcpPositionIcpIsLessThanThePurchaseQuest{ icp_position_icp: IcpTokens },
    IcpPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ icp_position_minimum_purchase: IcpTokens }
}

#[derive(CandidType, Deserialize)]
pub struct PurchaseIcpPositionSuccess {
    pub purchase_id: PurchaseId
}

pub type PurchaseIcpPositionResult = Result<PurchaseIcpPositionSuccess, PurchaseIcpPositionError>;

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

#[derive(CandidType, Deserialize)]
pub struct TransferIcpBalanceQuest {
    pub icp: IcpTokens,
    pub icp_fee: IcpTokens,
    pub to: IcpId
}

#[derive(CandidType, Deserialize)]
pub enum TransferIcpBalanceError {
    MsgCyclesTooLow{ transfer_icp_balance_fee: Cycles },
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
    CheckUserCyclesMarketIcpLedgerBalanceCallError((u32, String)),
    UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
    IcpTransferCallError((u32, String)),
    IcpTransferError(IcpTransferError)
}

pub type TransferIcpBalanceResult = Result<IcpBlockHeight, TransferIcpBalanceError>;

#[derive(CandidType, Deserialize)]
pub struct CMVoidCyclesPositionPositorMessageQuest {
    pub position_id: PositionId,
    // cycles in the call
    pub timestamp_nanos: u128
}

#[derive(CandidType, Deserialize)]
pub struct CMVoidIcpPositionPositorMessageQuest {
    pub position_id: PositionId,
    pub void_icp: IcpTokens,
    pub timestamp_nanos: u128
}

#[derive(CandidType, Deserialize)]
pub struct CMCyclesPositionPurchasePositorMessageQuest {
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
pub struct CMCyclesPositionPurchasePurchaserMessageQuest {
    pub cycles_position_id: PositionId,
    pub cycles_position_positor: Principal,
    pub cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    pub purchase_id: PurchaseId,
    pub purchase_timestamp_nanos: u128,
    // cycles in the call
    pub icp_payment: IcpTokens,
}

#[derive(CandidType, Deserialize)]
pub struct CMIcpPositionPurchasePositorMessageQuest {
    pub icp_position_id: PositionId,
    pub icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    pub purchaser: Principal,
    pub purchase_id: PurchaseId,
    pub icp_purchase: IcpTokens,
    pub purchase_timestamp_nanos: u128,
    // cycles in the call
}

#[derive(CandidType, Deserialize)]
pub struct CMIcpPositionPurchasePurchaserMessageQuest {
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


*/
