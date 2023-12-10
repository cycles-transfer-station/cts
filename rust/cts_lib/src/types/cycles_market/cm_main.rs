
use candid::{Principal, CandidType, Deserialize};
use serde::Serialize;
use crate::{
    types::{CallError, Cycles}, 
    icrc::Tokens
};


#[derive(CandidType, Serialize, Deserialize, Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub struct TradeContractIdAndLedgerId {
    pub icrc1_ledger_canister_id: Principal,
    pub trade_contract_canister_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub struct CMMainInit {
    pub cts_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub enum MarketCanisterType {
    TradeContract,
    PositionsStorage,
    TradesStorage,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct ControllerIsInTheMiddleOfADifferentCall{
    pub kind: ControllerIsInTheMiddleOfADifferentCallKind,
    pub must_call_continue: bool
}
#[derive(CandidType, Deserialize, Debug)]
pub enum ControllerIsInTheMiddleOfADifferentCallKind {
    ControllerCreateIcrc1TokenTradeContract
}

#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct ControllerCreateIcrc1TokenTradeContractQuest {
    pub icrc1_ledger_id: Principal,
    pub icrc1_ledger_transfer_fee: Tokens,
}

#[derive(CandidType, Deserialize)]
pub struct ControllerCreateIcrc1TokenTradeContractSuccess {
    pub trade_contract_canister_id: Principal,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum ControllerCreateIcrc1TokenTradeContractError {
    ControllerIsInTheMiddleOfADifferentCall(ControllerIsInTheMiddleOfADifferentCall),
    CyclesBalanceTooLow{ cycles_balance: Cycles },
    CreateCanisterIcrc1TokenTradeContractCallError(CallError),
    MidCallError(ControllerCreateIcrc1TokenTradeContractMidCallError),
}

#[derive(CandidType, Deserialize, Debug)]
pub enum ControllerCreateIcrc1TokenTradeContractMidCallError {
    TCInitCandidEncodeError(String),
    InstallCodeIcrc1TokenTradeContractCallError(CallError),
}