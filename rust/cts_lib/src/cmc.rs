use crate::{
    ic_ledger_types::{
        IcpId,
        IcpIdSub,
        IcpTokens,
        IcpTransferArgs,
        IcpBlockHeight,
        IcpTimestamp,
        icp_transfer,
        IcpTransferError,
        MAINNET_LEDGER_CANISTER_ID,
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        ICP_LEDGER_TRANSFER_DEFAULT_FEE
    },
    types::{CallError, Cycles},
    consts::ICP_LEDGER_TOP_UP_CANISTER_MEMO,
    tools::{
        principal_icp_subaccount,
        time_nanos_u64,
    },
    ic_cdk::api::call::{call_raw128},
};

use candid::{CandidType, Deserialize, Principal, decode_one, encode_one};





#[derive(CandidType, Deserialize)]
pub struct CmcNotifyCreateCanisterQuest {
    pub block_index: IcpBlockHeight,
    pub controller: Principal,
    pub subnet_type: Option<&'static str>
}


#[derive(CandidType, Deserialize)]
struct CmcNotifyTopUpCyclesQuest {
    block_index: IcpBlockHeight,
    canister_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub enum CmcNotifyError {
    Refunded { block_index: Option<IcpBlockHeight>, reason: String },
    InvalidTransaction(String),
    Other{ error_message: String, error_code: u64 },
    Processing,
    TransactionTooOld(IcpBlockHeight),
}

type NotifyTopUpResult = Result<Cycles, CmcNotifyError>;



#[derive(CandidType, Deserialize)]
pub enum LedgerTopupCyclesCmcIcpTransferError {
    IcpTransferCallError(CallError),
    IcpTransferError(IcpTransferError),
}

// make a public method to re-try a block-height
pub async fn ledger_topup_cycles_cmc_icp_transfer(icp: IcpTokens, from_subaccount: Option<IcpIdSub>, topup_canister: Principal) -> Result<IcpBlockHeight, LedgerTopupCyclesCmcIcpTransferError> {

    let cmc_icp_transfer_block_height: IcpBlockHeight = match icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: ICP_LEDGER_TOP_UP_CANISTER_MEMO,
            amount: icp,                              
            fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
            from_subaccount: from_subaccount,
            to: IcpId::new(&MAINNET_CYCLES_MINTING_CANISTER_ID, &principal_icp_subaccount(&topup_canister)),
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time_nanos_u64() })
        }
    ).await {
        Ok(transfer_call_sponse) => match transfer_call_sponse {
            Ok(block_index) => block_index,
            Err(transfer_error) => {
                return Err(LedgerTopupCyclesCmcIcpTransferError::IcpTransferError(transfer_error));
            }
        },
        Err(transfer_call_error) => {
            return Err(LedgerTopupCyclesCmcIcpTransferError::IcpTransferCallError((transfer_call_error.0 as u32, transfer_call_error.1)));
        }
    };
    
    Ok(cmc_icp_transfer_block_height)
}


#[derive(CandidType, Deserialize)]
pub enum LedgerTopupCyclesCmcNotifyError {
    CmcNotifyTopUpQuestCandidEncodeError(String),
    CmcNotifyCallError(CallError),
    CmcNotifySponseCandidDecodeError{candid_error: String, candid_bytes: Vec<u8>},
    CmcNotifyError(CmcNotifyError),
}

pub async fn ledger_topup_cycles_cmc_notify(cmc_icp_transfer_block_height: IcpBlockHeight, topup_canister_id: Principal) -> Result<Cycles, LedgerTopupCyclesCmcNotifyError> {

    let topup_cycles_cmc_notify_call_candid: Vec<u8> = match encode_one(
        & CmcNotifyTopUpCyclesQuest {
            block_index: cmc_icp_transfer_block_height,
            canister_id: topup_canister_id
        }
    ) {
        Ok(b) => b,
        Err(candid_error) => {
            return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifyTopUpQuestCandidEncodeError(format!("{}", candid_error)));
        }
    };

    let cycles: Cycles = match call_raw128(
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        "notify_top_up",
        &topup_cycles_cmc_notify_call_candid,
        0
    ).await {
        Ok(candid_bytes) => match decode_one::<NotifyTopUpResult>(&candid_bytes) {
            Ok(notify_topup_result) => match notify_topup_result {
                Ok(cycles) => cycles,
                Err(cmc_notify_error) => {
                    return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifyError(cmc_notify_error));
                }
            },
            Err(candid_error) => {
                return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifySponseCandidDecodeError{candid_error: format!("{}", candid_error), candid_bytes: candid_bytes});
            }
        },
        Err(notify_call_error) => {
            return Err(LedgerTopupCyclesCmcNotifyError::CmcNotifyCallError((notify_call_error.0 as u32, notify_call_error.1)));
        }
    };

    Ok(cycles)
}



