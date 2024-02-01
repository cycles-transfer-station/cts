use super::*;


pub async fn token_transfer(q: Icrc1TransferQuest) -> LedgerTransferReturnType {
    _ledger_transfer(q, &TOKEN_LEDGER_ID, &TOKEN_LEDGER_TRANSFER_FEE, |cm_data| { &mut cm_data.icrc1_token_ledger_transfer_fee }).await
}
pub async fn cycles_transfer(q: Icrc1TransferQuest) -> LedgerTransferReturnType {
    _ledger_transfer(q, &CYCLES_BANK_ID, &CYCLES_BANK_TRANSFER_FEE, |cm_data| { &mut cm_data.cycles_bank_transfer_fee }).await
}

pub type LedgerTransferReturnType = Result<Result<BlockId, Icrc1TransferError>, CallError>;

async fn _ledger_transfer<F>(q: Icrc1TransferQuest, local_key_cell_ledger: &'static LocalKey<Cell<Principal>>, localkey_cell_ledger_transfer_fee: &'static LocalKey<Cell<u128>>, get_mut_cm_data_ledger_transfer_fee: F) -> LedgerTransferReturnType 
where F: Fn(&mut CMData) -> &mut u128 {
    let r = icrc1_transfer(localkey::cell::get(local_key_cell_ledger), q).await;
    if let Ok(ref tr) = r {
        if let Err(Icrc1TransferError::BadFee { ref expected_fee }) = tr {
            localkey::cell::set(localkey_cell_ledger_transfer_fee, expected_fee.0.clone().try_into().unwrap_or(0));
            with_mut(&CM_DATA, |cm_data| {
                *get_mut_cm_data_ledger_transfer_fee(cm_data) = expected_fee.0.clone().try_into().unwrap_or(0);
            });
        }
    } 
    r
}