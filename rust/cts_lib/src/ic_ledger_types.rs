use ic_cdk::{
    api::call::{RejectionCode, CallResult},
};

use candid::Principal;


pub use ic_ledger_types::{
    Memo as IcpMemo,
    AccountIdentifier as IcpId,
    Subaccount as IcpIdSub,
    Tokens as IcpTokens,
    BlockIndex as IcpBlockHeight,
    Timestamp as IcpTimestamp,
    DEFAULT_SUBACCOUNT as ICP_DEFAULT_SUBACCOUNT,
    DEFAULT_FEE as ICP_LEDGER_TRANSFER_DEFAULT_FEE,
    MAINNET_CYCLES_MINTING_CANISTER_ID,
    MAINNET_LEDGER_CANISTER_ID, 
    TransferArgs as IcpTransferArgs, 
    TransferResult as IcpTransferResult, 
    TransferError as IcpTransferError,
    AccountBalanceArgs as IcpAccountBalanceArgs
};


// because of RejectionCode version mismatch

use ic_ledger_types::{
    transfer,
    account_balance,
};

pub async fn icp_transfer(ledger_principal: Principal, icp_transfer_args: IcpTransferArgs) -> CallResult<IcpTransferResult> {
    match transfer(ledger_principal, icp_transfer_args).await {
        Ok(transfer_result) => Ok(transfer_result),
        Err(transfer_call_error) => Err((RejectionCode::from(transfer_call_error.0 as i32), transfer_call_error.1))
    }
}

pub async fn icp_account_balance(ledger_principal: Principal, icp_account_balance_args: IcpAccountBalanceArgs) -> CallResult<IcpTokens> {
    match account_balance(ledger_principal, icp_account_balance_args).await {
        Ok(tokens) => Ok(tokens),
        Err(balance_call_error) => Err((RejectionCode::from(balance_call_error.0 as i32), balance_call_error.1))
    }
}



