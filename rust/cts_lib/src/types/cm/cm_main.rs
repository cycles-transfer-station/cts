
use candid::{Principal, CandidType, Deserialize};
use serde::Serialize;
use crate::{
    types::{CallError, Cycles}, 
    icrc::Tokens,
    consts::TRILLION,
};


pub const NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES: Cycles = 7 * TRILLION;
pub const MINIMUM_LEFTOVER_CYCLES_ON_THIS_CM_MAIN_CANISTER_AFTER_A_CREATION_OF_A_NEW_ICRC1TOKEN_TRADE_CONTRACT: Cycles = 20 * TRILLION;


#[derive(CandidType, Serialize, Deserialize, Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub struct TradeContractIdAndLedgerId {
    pub icrc1_ledger_canister_id: Principal,
    pub trade_contract_canister_id: Principal,
}

#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct TradeContractData {
    pub tc_module_hash: [u8; 32],
    pub latest_upgrade_timestamp_nanos: u64,
}

#[derive(CandidType, Deserialize)]
pub struct CMMainInit {
    pub cts_id: Principal,
    pub cycles_bank_id: Principal,
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
    TradeContractForTheLedgerAlreadyCreated(TradeContractIdAndLedgerId),
    CyclesBalanceTooLow{ cycles_balance: Cycles },
    CreateCanisterIcrc1TokenTradeContractCallError(CallError),
    MidCallError(ControllerCreateIcrc1TokenTradeContractMidCallError),
}

#[derive(CandidType, Deserialize, Debug)]
pub enum ControllerCreateIcrc1TokenTradeContractMidCallError {
    TCInitCandidEncodeError(String),
    InstallCodeIcrc1TokenTradeContractCallError(CallError),
}
