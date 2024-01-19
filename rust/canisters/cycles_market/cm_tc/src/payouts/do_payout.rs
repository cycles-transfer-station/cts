use super::*;

pub struct DoPayoutQuest {
    pub trade_mount: u128,
    pub cts_payout_fee: u128,
    pub payee: Principal,
    pub memo: ByteBuf,
}

pub async fn do_cycles_payout(q: DoPayoutQuest) -> Option<PayoutData> {
    _do_payout(cycles_transfer, localkey::cell::get(&CYCLES_BANK_TRANSFER_FEE), q).await
}

pub async fn do_token_payout(q: DoPayoutQuest) -> Option<PayoutData> {
    _do_payout(token_transfer, localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE), q).await
}

async fn _do_payout<LedgerTransferFuture: Future<Output=LedgerTransferReturnType>, F>(cycles_or_token_transfer: F, ledger_transfer_fee: u128, q: DoPayoutQuest) -> Option<PayoutData> 
where F: Fn(Icrc1TransferQuest) -> LedgerTransferFuture {
    if ledger_transfer_fee >= q.trade_mount.saturating_sub(q.cts_payout_fee) {
        Some(
            PayoutData{
                did_transfer: false,
                ledger_transfer_fee,
            }
        )
    } else {
        match cycles_or_token_transfer(
            Icrc1TransferQuest{
                to: IcrcId{ owner: q.payee, subaccount: None },
                fee: Some(ledger_transfer_fee),
                memo: Some(q.memo),
                from_subaccount: Some(*POSITIONS_TOKEN_SUBACCOUNT),
                created_at_time: None,
                amount: q.trade_mount.saturating_sub(q.cts_payout_fee).saturating_sub(ledger_transfer_fee),
            }
        ).await {
            Ok(token_transfer_result) => match token_transfer_result {
                Ok(_block_height) => {
                    Some(
                        PayoutData{
                            did_transfer: true,
                            ledger_transfer_fee,
                        }
                    )
                },
                Err(token_transfer_error) => {
                    ic_cdk::println!("payout transfer error {:?}", token_transfer_error);
                    None
                }
            },
            Err(token_transfer_call_error) => {
                ic_cdk::println!("payout transfer call error {:?}", token_transfer_call_error);
                None
            }
        }
    }
}