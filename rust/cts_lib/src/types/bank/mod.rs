use super::*;
use crate::icrc::{IcrcId, BlockId, IcrcSubaccount};
use serde_bytes::ByteBuf;
use crate::cmc::{LedgerTopupCyclesCmcIcpTransferError, LedgerTopupCyclesCmcNotifyError};

pub mod log_types;


pub const BANK_TRANSFER_FEE: Cycles = 10_000_000_000;


#[derive(CandidType, Deserialize)]
pub struct CyclesInQuest {
    pub cycles: Cycles,
    pub fee: Option<Cycles>,
    pub to: IcrcId,
    pub memo: Option<ByteBuf>,
    pub created_at_time: Option<u64>
}

#[derive(CandidType, Deserialize, Debug, PartialEq, Eq)]
pub enum CyclesInError {
    MsgCyclesTooLow,
    BadFee{ expected_fee: Cycles },
    GenericError{ error_code: u128, message: String },
}

#[derive(CandidType, Deserialize)]
pub struct CyclesOutQuest {
    pub cycles: Cycles,
    pub fee: Option<Cycles>,
    pub from_subaccount: Option<IcrcSubaccount>,
    pub memo: Option<ByteBuf>,
    pub for_canister: Principal,
    pub created_at_time: Option<u64>   
}

#[derive(CandidType, Deserialize, Debug, PartialEq, Eq)]
pub enum CyclesOutError {
    InsufficientFunds{ balance: Cycles },
    BadFee{ expected_fee: Cycles },
    DepositCyclesCallError(CallError),
    GenericError{ error_code: u128, message: String },    
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct MintCyclesQuest {
    pub burn_icp: u128,
    pub burn_icp_transfer_fee: u128,
    pub to: IcrcId,   
    pub fee: Option<Cycles>,
    pub memo: Option<ByteBuf>,    
    pub created_at_time: Option<u64>    
}

#[derive(CandidType, Deserialize, Debug)]
pub enum MintCyclesError {
    UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
    MinimumBurnIcp{ minimum_burn_icp: u128 },
    BadFee{ expected_fee: Cycles },
    GenericError{ error_code: u128, message: String },
    CBIsBusy,
    LedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
    LedgerTopupCyclesCmcNotifyRefund{ block_index: u64, reason: String},
    MidCallError(MintCyclesMidCallError)
}

#[derive(CandidType, Deserialize, Debug)]
pub enum MintCyclesMidCallError {
    LedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct MintCyclesSuccess {
    pub mint_cycles: Cycles,
    pub mint_cycles_block_height: u128,
}

pub type MintCyclesResult = Result<MintCyclesSuccess, MintCyclesError>;

#[derive(CandidType, Deserialize, Debug)]
pub enum UserIsInTheMiddleOfADifferentCall {
    MintCyclesCall{ must_call_complete: bool },
}

#[derive(CandidType, Deserialize)]
pub struct GetLogsBackwardsSponse {
    pub logs: Vec<(BlockId, log_types::Log)>,
    pub is_last_chunk: bool,
}