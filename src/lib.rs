




mod tools;
use tools::{
    user_icp_balance_id,
    user_cycles_balance_topup_memo_bytes,
    check_user_icp_balance,
    check_user_cycles_balance,

    
};








struct UserData {
    pub cycles_balance: u128,
    pub cycles_transfer_purchases: Vec<CyclesTransferPurchaseLog>, // 
}


thread_local! {
    static USERS_DATA = RefCell::new(HashMap::<Principal, UserData>::new());    
}






#[derive(CandidType, Deserialize)]
pub enum CyclesTransferMemo {
    text(String),
    nat64(u64),
    blob(Vec<u8>)
}

#[derive(CandidType, Deserialize)]
pub struct CyclesTransfer {
    memo: CyclesTransferMemo
}



#[update]
pub fn cycles_transfer(CyclesTransfer) -> () {

}






type IcpId = ic_ledger_types::AccountIdentifier;

type IcpIdSub = ic_ledger_types::Subaccount;

type IcpTokens = ic_ledger_types::Tokens;


#[derive(CandidType, Deserialize)]
struct TopUpCyclesBalanceData {
    topup_cycles_transfer_memo: CyclesTransferMemo
} 

#[derive(CandidType, Deserialize)]
struct TopUpIcpBalanceData {
    topup_icp_id: IcpId
} 

#[derive(CandidType, Deserialize)]
struct TopUpBalanceData {
    topup_cycles_balance: TopUpCyclesBalanceData, 
    topup_icp_balance: TopUpIcpBalanceData,
}


#[update]
pub fn topup_balance() -> TopUpBalanceData {
    TopUpBalanceData {
        topup_cycles_balance: TopUpCyclesBalanceData {
            topup_cycles_transfer_memo: CyclesTransferMemo::blob(user_cycles_balance_topup_memo_bytes(&caller()).to_vec())
        },
        topup_icp_balance: TopUpIcpBalanceData {
            topup_icp_id: user_icp_balance_id(&caller())
        }
    }
}



#[derive(CandidType, Deserialize)]
struct UserBalance {
    cycles_balance: u128,
    icp_balance: IcpTokens, 
}

#[derive(CandidType, Deserialize)]
enum SeeBalanceError {
    IcpLedgerCheckBalanceError(String),
    

}

type SeeBalanceSponse = Result<UserBalance, SeeBalanceError>;

#[update]
pub async fn see_balance() -> SeeBalanceSponse {
    Ok(UserBalance {
        cycles_balance: check_user_cycles_balance(&caller()),
        icp_balance: match check_user_icp_balance(&caller()).await {
            Ok(icp_tokens) => icp_tokens,
            Err(e) => return Err(SeeBalanceError::IcpLedgerCheckBalanceError(format!("{:?}", e)));
        }
    })
}



#[derive(CandidType, Deserialize)]
struct IcpPayoutQuest {
    amount: IcpTokens,
    payout_icp_id: IcpId
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
enum IcpPayoutError {
    IcpLedgerCheckBalanceError(String),
    NotEnoughBalance { icp_balance: IcpTokens },


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
pub async fn collect_balance(q: CollectBalanceQuest) -> CollectBalanceSponse {
    match q {
        CollectBalanceQuest::icp_payout(icp_payout_quest) => {
            let user_icp_balance: IcpTokens = match check_user_icp_balance(&caller()).await {
                Ok(icp_tokens) => icp_tokens,
                Err(e) => return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerCheckBalanceError(format!("{:?}", e))));
            };
            if icp_payout_quest.amount + icp_transfer_fee + icp_payout_fee > user_icp_balance {
                return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::NotEnoughBalance { icp_balance: user_icp_balance }));
            }
            // payout
            // pay self for the icp_payout_fee


        },
        CollectBalanceQuest::cycles_payout(cycles_payout_quest) => {

        }
    }
}



#[derive(CandidType, Deserialize)]
struct ConvertIcpBalanceForCyclesWithTheCmcRateQuest {
    amount: IcpTokens
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








#[no_mangle]
pub fn canister_inspect_message() {
    // caution: this function is only called for ingress messages 
    
    if ["topup_balance", "see_balance", "collect_balance", ].contains(method_name()) {
        if caller() == Principal::anonymous() { // check '==' plementation is correct otherwise caller().as_slice() == Principal::anonymous().as_slice()
            trap("caller cannot be anonymous for this method.")
        }
    }
}



