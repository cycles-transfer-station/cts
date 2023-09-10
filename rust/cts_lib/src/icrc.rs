use crate::{
    ic_cdk::{
        call,
    },
    types::CallError,
    tools::call_error_as_u32_and_string,
};
use candid::Principal;

pub use icrc_ledger_types::{
    icrc1::{
        account::{
            Account as IcrcId,
            Subaccount as IcrcSub,    
            DEFAULT_SUBACCOUNT as ICRC_DEFAULT_SUBACCOUNT,    
        },
        transfer::{
            BlockIndex as BlockId,
            Memo as IcrcMemo,
            TransferArg as TokenTransferArg,
            TransferError as TokenTransferError,
        }
    }
};


pub use u128 as Tokens;
//pub struct Tokens(pub u128);
/*
pub use ic_icrc1::{
    Account as Icrc1Id,
    Subaccount as Icrc1Sub,
    DEFAULT_SUBACCOUNT as ICRC1_DEFAULT_SUBACCOUNT,
    Memo as Icrc1Memo,
    endpoints::{
        NumTokens as Tokens,
        TransferArg as TokenTransferArg,
        TransferError as TokenTransferError,
        BlockIndex as BlockId,
    }
};
*/

pub async fn icrc1_transfer(icrc1_ledger_id: Principal, q: TokenTransferArg) -> Result<Result<BlockId, TokenTransferError>, CallError> {
    call(
        icrc1_ledger_id,
        "icrc1_transfer",
        (q,),
    ).await.map_err(|e| call_error_as_u32_and_string(e)).map(|(s,)| s)
}
/*   
    let client: ICRC1Client<CdkRuntime> = ICRC1Client::<CdkRuntime>{
        runtime: CdkRuntime{},
        ledger_canister_id: icrc1_ledger_id
    };
    
    client.transfer(q)
        .await
        .map(|o| { o.map(|o2| { BlockId::from(o2) }) })
        .map_err(|e| { (e.0 as u32, e.1) })
}
*/

pub async fn icrc1_balance_of(icrc1_ledger_id: Principal, count_id: IcrcId) -> Result<Tokens, (u32, String)> {
    call(
        icrc1_ledger_id,
        "icrc1_balance_of",
        (count_id,),
    ).await.map_err(|e| (e.0 as u32, e.1)).map(|(s,)| s)
}
/*
    let client: ICRC1Client<CdkRuntime> = ICRC1Client::<CdkRuntime>{
        runtime: CdkRuntime{},
        ledger_canister_id: icrc1_ledger_id
    };
    
    client.balance_of(account)
        .await
        .map(|t| { Tokens::from(t) })
        .map_err(|e| { (e.0 as u32, e.1) })

}
*/

