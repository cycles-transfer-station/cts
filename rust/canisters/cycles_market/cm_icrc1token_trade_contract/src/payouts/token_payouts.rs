
use crate::*;



pub enum TokenTransferErrorType {
    TokenTransferError(TokenTransferError),
    TokenTransferCallError((u32, String))
}

pub enum CMMessageErrorType {
    CMCallQuestCandidEncodeError(CandidError),
    CMCallQuestPutBytesCandidEncodeError(CandidError),
    CMCallerCallError(CMCallError),
    CMCallerCallSponseCandidDecodeError(CandidError),
    CMCallerCallCallError((u32, String))
}

pub enum DoTokenPayoutSponse {
    TokenTransferError(TokenTransferErrorType),
    TokenTransferSuccessAndCMMessageError(TokenTransferBlockHeightAndTimestampNanos, CMMessageErrorType),
    TokenTransferSuccessAndCMMessageSuccess(TokenTransferBlockHeightAndTimestampNanos, u128),
    //CMMessageError(CMMessageErrorType),
    //CMMessageSuccess(u128),
    NothingForTheDo,
}

pub async fn do_token_payout<T: TokenPayoutDataTrait>(q: T) -> DoTokenPayoutSponse {
    
    let token_payout_data_token_transfer: TokenTransferBlockHeightAndTimestampNanos = match q.token_payout_data().token_transfer {
        Some(token_transfer_data) => token_transfer_data,
        None => {
            let token_transfer_created_at_time: u64 = time_nanos_u64()-NANOS_IN_A_SECOND as u64;
            match token_transfer(
                TokenTransferArg{
                    memo: q.token_transfer_memo(),
                    amount: q.tokens().into(),
                    fee: Some(q.token_transfer_fee().into()),
                    from_subaccount: Some(principal_token_subaccount(&q.token_payout_payor())),
                    to: IcrcId{owner: ic_cdk::api::id(), subaccount: Some(principal_token_subaccount(&q.token_payout_payee()))},
                    created_at_time: Some(token_transfer_created_at_time)
                }
            ).await {
                Ok(token_transfer_result) => match token_transfer_result {
                    Ok(block_height) => {
                        TokenTransferBlockHeightAndTimestampNanos{
                            block_height: Some(block_height),
                            timestamp_nanos: token_transfer_created_at_time as u128
                        }
                    },
                    Err(token_transfer_error) => {
                        return DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType::TokenTransferError(token_transfer_error));
                    }
                },
                Err(token_transfer_call_error) => {
                    return DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType::TokenTransferCallError(token_transfer_call_error));
                }
            }
        }
    };
    
    match q.token_payout_data().cm_message_call_success_timestamp_nanos {
        Some(_cm_message_call_success_timestamp_nanos) => return DoTokenPayoutSponse::NothingForTheDo,
        None => {
            let call_future = call_raw128(
                with(&CM_DATA, |cm_data| { cm_data.cm_caller }),
                "cm_call",
                &match encode_one(
                    CMCallQuest{
                        cm_call_id: q.cm_call_id(),
                        for_the_canister: q.token_payout_payee(),
                        method: q.token_payout_payee_method().to_string(),
                        put_bytes: match q.token_payout_payee_method_quest_bytes(token_payout_data_token_transfer.clone()) {
                            Ok(b) => b,
                            Err(candid_error) => {
                                return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallQuestPutBytesCandidEncodeError(candid_error));     
                            }
                        },
                        cycles: 0,
                        cm_callback_method: q.cm_call_callback_method().to_string(),
                    }
                ) {
                    Ok(b) => b,
                    Err(candid_error) => {
                        return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallQuestCandidEncodeError(candid_error));
                    }
                },
                0 + 10_000_000_000 // for the cm_caller
            );
            match call_future.await {
                Ok(b) => match decode_one::<CMCallResult>(&b) {
                    Ok(cm_call_sponse) => match cm_call_sponse {
                        Ok(()) => {
                            return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageSuccess(token_payout_data_token_transfer, time_nanos());
                        },
                        Err(cm_call_error) => {
                            return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallerCallError(cm_call_error));
                        }
                    },
                    Err(candid_error) => {
                        return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallerCallSponseCandidDecodeError(candid_error));                    
                    }
                },
                Err(call_error) => {
                    return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallerCallCallError((call_error.0 as u32, call_error.1)));                    
                } 
            }
        }
    }
    
}

