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

pub async fn do_token_payout<T: TokenPayoutTrait>(q: T) -> TokenPayoutData {
    
    let mut token_payout_data: TokenPayoutData = q.token_payout_data();
    
    if token_payout_data.token_transfer.is_none() {
        let ledger_transfer_fee: Tokens = q.token_ledger_transfer_fee(); 
        let tokens_minus_cts_payout_fee = q.tokens().saturating_sub(q.tokens_payout_fee());
        if ledger_transfer_fee < tokens_minus_cts_payout_fee {
            match token_transfer(
                TokenTransferArg{
                    memo: q.token_transfer_memo(),
                    amount: tokens_minus_cts_payout_fee.saturating_sub(ledger_transfer_fee).into(),
                    fee: Some(ledger_transfer_fee.into()),
                    from_subaccount: Some(*POSITIONS_TOKEN_SUBACCOUNT),
                    to: IcrcId{owner: q.token_payout_payee(), subaccount: None},
                    created_at_time: Some(time_nanos_u64())
                }
            ).await {
                Ok(token_transfer_result) => match token_transfer_result {
                    Ok(_block_height) => {
                        token_payout_data.token_transfer = Some(
                            TokenTransferData{
                                did_transfer: true,
                                ledger_transfer_fee: ledger_transfer_fee,
                            }
                        );
                    },
                    Err(token_transfer_error) => {
                        ic_cdk::println!("token payout transfer error {:?}", token_transfer_error);
                        return token_payout_data;
                    }
                },
                Err(token_transfer_call_error) => {
                    ic_cdk::println!("token payout transfer call error {:?}", token_transfer_call_error);
                    return token_payout_data;
                }
            }
        } else {
            token_payout_data.token_transfer = Some(
                TokenTransferData{
                    did_transfer: false,
                    ledger_transfer_fee: ledger_transfer_fee,
                }
            );
        }
    }
    
    return token_payout_data;
}


