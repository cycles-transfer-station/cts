use crate::ic_cdk::export::{Principal, candid::{CandidType, Deserialize}};
use crate::types::Cycles;

#[derive(CandidType, Deserialize)]
pub struct CMCallerInit {
    pub cycles_market_token_trade_contract: Principal,
}

#[derive(CandidType, Deserialize)]    
pub struct CMCallQuest{
    pub cm_call_id: u128,
    pub for_the_canister: Principal,
    pub method: String,
    #[serde(with = "serde_bytes")]
    pub put_bytes: Vec<u8>,
    pub cycles: Cycles,
    pub cm_callback_method: String,
}

#[derive(CandidType, Deserialize)]
pub enum CMCallError {
    MaxCalls,
}

pub type CMCallResult = Result<(), CMCallError>;

#[derive(CandidType, Deserialize, Clone)]
pub struct CMCallbackQuest {
    pub cm_call_id: u128,
    pub opt_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)> // None means callstatus == 'replied'
    // sponse_bytes? do i care? CallResult
}

