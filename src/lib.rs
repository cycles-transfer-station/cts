







type ICPID = ic_ledger_types::AccountIdentifier;

type ICPTokens = ic_ledger_types::Tokens;





#[derive(CandidType, Deserialize)]
struct TopUpCyclesBalanceData {
    topup_cycles_transfer_memo: CyclesTransferMemo
} 

#[derive(CandidType, Deserialize)]
struct TopUpIcpBalanceData {
    topup_icp_id: ICPID
} 

#[derive(CandidType, Deserialize)]
struct TopUpBalanceData {
    topup_cycles_balance: TopUpCyclesBalanceData, 
    topup_icp_balance: TopUpIcpBalanceData,
}


#[update]
pub fn topup_balance() -> TopUpBalanceData {

}



#[derive(CandidType, Deserialize)]
struct SeeBalanceData {
    cycles_balance: u128,
    icp_balance: ICPTokens, 
}

#[update]
pub fn see_balance() -> SeeBalanceData {

}



#[derive(CandidType, Deserialize)]
struct IcpPayoutQuest {
    amount: ICPTokens,
    payout_icp_id: ICPID
}

#[derive(CandidType, Deserialize)]
struct CyclesPayoutQuest {
    amount: u128,
    payout_cycles_transfer_canister: Principal         // the memo is: cts-payout    
}

#[derive(CandidType, Deserialize)]
enum CollectBalanceQuest {
    icp_payout(IcpPayoutQuest),
    cycles_payout(CyclesPayoutQuest)
}

#[derive(CandidType, Deserialize)]
enum IcpPayoutError {    // make this the ic_ledger_types::TransferResult ?
  
}

#[derive(CandidType, Deserialize)]
enum CyclesPayoutError {

}

type IcpPayoutSponse = Result< , IcpPayoutError>;

type CyclesPayoutSponse = Result<u128, CyclesPayoutError>;

#[derive(CandidType, Deserialize)]
enum CollectBalanceSponse {
    icp_payout(IcpPayoutSponse),
    cycles_payout(CyclesPayoutSponse)
}

#[update]
pub async fn collect_balance(CollectBalanceQuest) -> CollectBalanceSponse {

}



#[derive(CandidType, Deserialize)]
struct ConvertIcpBalanceForCyclesWithTheCmcRateQuest {
    amount: ICPTokens
}

#[derive(CandidType, Deserialize)]
enum ConvertIcpBalanceForCyclesWithTheCmcRateError {

}


#[update]
pub async fn convert_icp_balance_for_cycles_with_the_cmc_rate(ConvertIcpBalanceForCyclesWithTheCmcRateQuest) -> Result<u128, ConvertIcpBalanceForCyclesWithTheCmcRateError> {

}



#[derive(CandidType, Deserialize)]
struct PurchaseCyclesTransferQuest {
    r#for: Principal,
    cycles: u128,
    cycles_transfer_memo: CyclesTransferMemo,
    public: bool,
}

#[derive(CandidType, Deserialize)]
enum PurchaseCyclesTransferError {
    CanisterDoesNotExist,
    NoCyclesTransferMethodOnTheCanister,
    BalanceTooLow
}

#[update]
pub async fn purchase_cycles_transfer(PurchaseCyclesTransferQuest) -> Result<u128, PurchaseCyclesTransferError> {

}





#[derive(CandidType, Deserialize)]
struct PurchaseCyclesBankQuest {

}

#[derive(CandidType, Deserialize)]
struct CyclesBankPurchaseLog {
    cycles_bank_principal: Principal,
    cost_cycles: u64, // 64? or 128
    timestamp: u64
}

#[derive(CandidType, Deserialize)]
enum PurchaseCyclesBankError {

}

#[update]
pub async fn purchase_cycles_bank(PurchaseCyclesBankQuest) -> Result<CyclesBankPurchaseLog, PurchaseCyclesBankError> {

}




#[update]
pub see_cycles_transfer_purchases(page: u)
