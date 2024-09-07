use crate::{
    types::CallError,
    tools::call_error_as_u32_and_string,
};
use ic_cdk::call;
use candid::{CandidType, Deserialize, Principal};
use serde_bytes::ByteBuf;

pub use icrc_ledger_types::{
    icrc1::{
        account::{
            Account as IcrcId,
            Subaccount as IcrcSub,
            Subaccount as IcrcSubaccount,    
            DEFAULT_SUBACCOUNT as ICRC_DEFAULT_SUBACCOUNT,    
        },
        transfer::{
            Memo as IcrcMemo,
            TransferError as Icrc1TransferError,
        }
    },
    icrc::generic_metadata_value::MetadataValue as IcrcMetadataValue,
};

#[derive(CandidType, Deserialize)]
pub struct Icrc1TransferQuest {
    pub to: IcrcId,
    pub fee: Option<u128>,
    pub memo: Option<ByteBuf>,
    pub from_subaccount: Option<IcrcSub>,
    pub created_at_time: Option<u64>,
    pub amount: u128,
}

pub use u128 as BlockId;
pub use u128 as Tokens;


pub async fn icrc1_transfer(icrc1_ledger_id: Principal, q: Icrc1TransferQuest) -> Result<Result<BlockId, Icrc1TransferError>, CallError> {
    call(
        icrc1_ledger_id,
        "icrc1_transfer",
        (q,),
    ).await
    .map_err(call_error_as_u32_and_string)
    .map(|(ir,): (Result<candid::Nat, Icrc1TransferError>,)| ir.map(|nat| nat.0.try_into().unwrap_or(0)))
}

pub async fn icrc1_balance_of(icrc1_ledger_id: Principal, count_id: IcrcId) -> Result<Tokens, (u32, String)> {
    call(
        icrc1_ledger_id,
        "icrc1_balance_of",
        (count_id,),
    ).await.map_err(|e| (e.0 as u32, e.1)).map(|(s,)| s)
}
