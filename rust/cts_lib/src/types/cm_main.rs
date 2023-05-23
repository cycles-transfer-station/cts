
use crate::ic_cdk::export::{Principal, candid::{CandidType, Deserialize}};


#[derive(CandidType, Deserialize, Hash, PartialEq, Eq, Copy, Clone)]
pub struct Icrc1TokenTradeContract {
    pub icrc1_ledger_canister_id: Principal,
    pub trade_contract_canister_id: Principal,
    pub opt_cm_caller: Option<Principal>
}


