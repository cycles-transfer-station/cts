



struct UserData {
    cycles_balance: u128,
    cycles_transfer_purchases: Vec<CyclesTransferPurchaseLog>, // 
}


thread_local! {
    static USER_DATA = RefCell::new(HashMap::<Principal, UserData>::new());    
}






type ICPID = ic_ledger_types::AccountIdentifier;

type ICPIDSub = ic_ledger_types::Subaccount;

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
    check_caller_is_not_anonymous_caller();

    TopUpBalanceData {
        topup_cycles_balance: TopUpCyclesBalanceData {
            topup_cycles_transfer_memo: CyclesTransferMemo::blob(TP30bytesprincipal)
        },
        topup_icp_balance: TopUpIcpBalanceData {
            topup_icp_id: ICPID::new(&id(), &principal_as_an_icpsubaccount(&caller()))
        }
    }
}



#[derive(CandidType, Deserialize)]
struct UserBalance {
    cycles_balance: u128,
    icp_balance: ICPTokens, 
}

#[update]
pub async fn see_balance() -> UserBalance {
    check_caller_is_not_anonymous_caller();
    
    // :check: ledger for the icp-balance.
    
    UserBalance {
        cycles_balance: 
        icp_balance: 
    }
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
pub async fn purchase_cycles_bank(q: PurchaseCyclesBankQuest) -> Result<CyclesBankPurchaseLog, PurchaseCyclesBankError> {

}




#[derive(CandidType, Deserialize)]
struct CyclesTransferPurchaseLog {
    r#for: principal,
    cycles_sent: u128,
    cycles_accepted: u128; // 64?
    cycles_transfer_memo: CyclesTransferMemo,
    timestamp: u64,
}

#[update]
pub fn see_cycles_transfer_purchases(page: u128) -> Vec<CyclesTransferPurchaseLog> {

}


#[update]
pub fn see_cycles_bank_purchases(page: u128) -> Vec<CyclesBankPurchaseLog> {

}



#[derive(CandidType, Deserialize)]
struct Fees {
    purchase_cycles_bank_cost_cycles: u128,
    purchase_cycles_transfer_cost_cycles: u128
}

#[update]
pub fn see_fees() -> Fees {
    
}