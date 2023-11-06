// do not borrow or borrow_mut the CM_DATA.
use crate::*;


/*
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
*/
pub type DoTokenPayoutSponse = TokenPayoutData;

pub async fn do_token_payout<T: TokenPayoutTrait>(q: T) -> TokenPayoutData {
    
    let mut token_payout_data: TokenPayoutData = q.token_payout_data();
    
    if let None = token_payout_data.token_transfer {
        let token_transfer_created_at_time: u64 = time_nanos_u64()-NANOS_IN_A_SECOND as u64;
        let ledger_transfer_fee: Tokens = q.token_ledger_transfer_fee(); 
        match token_transfer(
            TokenTransferArg{
                memo: q.token_transfer_memo(),
                amount: {
                    q.tokens()
                        .saturating_sub(q.tokens_payout_fee())
                        .saturating_sub(ledger_transfer_fee)
                        .into()
                },
                fee: Some(ledger_transfer_fee.into()),
                from_subaccount: Some(principal_token_subaccount(&q.token_payout_payor())),
                to: IcrcId{owner: q.token_payout_payee(), subaccount: None},
                created_at_time: Some(token_transfer_created_at_time)
            }
        ).await {
            Ok(token_transfer_result) => match token_transfer_result {
                Ok(block_height) => {
                    token_payout_data.token_transfer = Some(
                        TokenTransferData{
                            block_height: Some(block_height),
                            timestamp_nanos: token_transfer_created_at_time as u128,
                            ledger_transfer_fee: ledger_transfer_fee,
                        }
                    )
                },
                Err(token_transfer_error) => {
                    // log // return DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType::TokenTransferError(token_transfer_error));
                    ic_cdk::println!("token payout transfer error {:?}", token_transfer_error);
                    return token_payout_data;
                }
            },
            Err(token_transfer_call_error) => {
                // log // return DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType::TokenTransferCallError(token_transfer_call_error));
                ic_cdk::println!("token payout transfer call error {:?}", token_transfer_call_error);
                return token_payout_data;
            }
        }
        
    }
    
    if let None = token_payout_data.token_fee_collection {
        let created_at_time: u64 = time_nanos_u64()-NANOS_IN_A_SECOND as u64;
        let ledger_transfer_fee: Tokens = q.token_ledger_transfer_fee();         
        match token_transfer(
            TokenTransferArg{
                memo: q.token_fee_collection_transfer_memo(),
                amount: q.tokens_payout_fee()
                            .saturating_sub(ledger_transfer_fee)
                            .into(),
                fee: Some(ledger_transfer_fee.into()),
                from_subaccount: Some(principal_token_subaccount(&q.token_payout_payor())),
                to: IcrcId{owner: ic_cdk::api::id(), subaccount: None},
                created_at_time: Some(created_at_time)
            }
        ).await {
            Ok(token_transfer_result) => match token_transfer_result {
                Ok(block_height) => {
                    token_payout_data.token_fee_collection = Some(
                        TokenTransferData{
                            block_height: Some(block_height),
                            timestamp_nanos: created_at_time as u128,
                            ledger_transfer_fee,
                        }
                    )
                },
                Err(token_transfer_error) => {
                    ic_cdk::println!("token payout fee collection transfer error {:?}", token_transfer_error);
                    return token_payout_data;
                }
            },
            Err(token_transfer_call_error) => {
                ic_cdk::println!("token payout fee collection transfer call error {:?}", token_transfer_call_error);
                return token_payout_data;
            }
        }
        
    }
   
    if let None = token_payout_data.cm_message_call {
        let call_future = call_raw128(
            q.token_payout_payee(),
            q.token_payout_payee_method(),
            match q.token_payout_payee_method_quest_bytes(token_payout_data.token_transfer.as_ref().unwrap().clone()) {
                Ok(b) => b,
                Err(_candid_error) => {
                    // log // return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallQuestPutBytesCandidEncodeError(candid_error));     
                    return token_payout_data;
                }
            },
            0
        );
        match call_future.await {
            Ok(_b) => {
                token_payout_data.cm_message_call = Some(None);
            },
            Err(_call_error) => {
                /*
                if canister-module-is-empty {
                    token_payout_data.cm_message_call = Some(Some(call_error_as_u32_and_string(call_error)));
                }
                */
                return token_payout_data;
            }
        }
    }
    
    return token_payout_data;
}


