use crate::{
    ic_cdk::{
        call,
    },
    types::{CallError, Cycles},
    tools::call_error_as_u32_and_string,
};
use candid::{CandidType, Deserialize, Principal};
use serde_bytes::ByteBuf;

pub use icrc_ledger_types::{
    icrc1::{
        account::{
            Account as IcrcId,
            Subaccount as IcrcSub,    
            DEFAULT_SUBACCOUNT as ICRC_DEFAULT_SUBACCOUNT,    
        },
        transfer::{
            Memo as IcrcMemo,
            TransferError as Icrc1TransferError,
        }
    }
};

#[derive(CandidType, Deserialize)]
pub struct Icrc1TransferQuest {
    pub to: IcrcId,
    pub fee: Option<Cycles>,
    pub memo: Option<ByteBuf>,
    pub from_subaccount: Option<IcrcSub>,
    pub created_at_time: Option<u64>,
    pub amount: Cycles,
}

pub use u128 as BlockId;
pub use u128 as Tokens;


pub async fn icrc1_transfer(icrc1_ledger_id: Principal, q: Icrc1TransferQuest) -> Result<Result<BlockId, TokenTransferError>, CallError> {
    call(
        icrc1_ledger_id,
        "icrc1_transfer",
        (q,),
    ).await
    .map_err(call_error_as_u32_and_string)
    .map(|(ir,): (Result<candid::Nat, TokenTransferError>,)| ir.map(|nat| nat.0.try_into().unwrap_or(0)))
}

pub async fn icrc1_balance_of(icrc1_ledger_id: Principal, count_id: IcrcId) -> Result<Tokens, (u32, String)> {
    call(
        icrc1_ledger_id,
        "icrc1_balance_of",
        (count_id,),
    ).await.map_err(|e| (e.0 as u32, e.1)).map(|(s,)| s)
}

