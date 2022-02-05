




mod tools;
use tools::{
    user_icp_balance_id,
    user_cycles_balance_topup_memo_bytes,
    check_user_icp_balance,
    check_user_cycles_balance,

    
};






struct UserData {
    pub cycles_balance: u128,
    //pub cycles_transfer_purchases: Vec<CyclesTransferPurchaseLog>, // 
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

type IcpBlockIndex = ic_ledger_types::BlockIndex;


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
    icp: IcpTokens,
    payout_icp_id: IcpId
}

#[derive(CandidType, Deserialize)]
struct CyclesPayoutQuest {
    cycles: u128,
    payout_cycles_transfer_canister: Principal         // the memo is: cts-payout    
}

#[derive(CandidType, Deserialize)]
enum CollectBalanceQuest {
    icp_payout(IcpPayoutQuest),
    cycles_payout(CyclesPayoutQuest)
}

#[derive(CandidType, Deserialize)]
enum IcpPayoutError {
    // IcpLedgerCheckBalanceError(String),
    // NotEnoughBalance { icp_balance: IcpTokens },
    IcpLedgerTransferError(ic_ledger_types::TransferError),
    IcpLedgerTransferCallError(String),


}

#[derive(CandidType, Deserialize)]
enum CyclesPayoutError {
    BalanceTooLow,
    InvalidCyclesAmount,
    // CanisterDoesNotExist,
    // NoCyclesTransferMethodOnTheCanister,
    CyclesTransferCallError(String),
}

type IcpPayoutSponse = Result<IcpBlockIndex, IcpPayoutError>;

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
            // payout  // test if someone tries to payout the total-balance, will it give an InsufficientFunds Error cause of the icp transfer fee ? 
            // let user_icp_balance: IcpTokens = match check_user_icp_balance(&caller()).await {
            //     Ok(icp_tokens) => icp_tokens,
            //     Err(e) => return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerCheckBalanceError(format!("{:?}", e))));
            // };
            // if icp_payout_quest.amount + icp_transfer_fee + icp_payout_fee > user_icp_balance {
            //     return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::NotEnoughBalance { icp_balance: user_icp_balance }));
            // }
            // ICP_PAYOUT_FEE ? check balance? minimum-transfer? // collect fee ?

            use ic_ledger_types::{transfer, MAINNET_LEDGER_CANISTER_ID, TransferArgs, Memo, DEFAULT_FEE};
            
            let icp_transfer_call: CallResult<TransferResult> = transfer(
                MAINNET_LEDGER_CANISTER_ID,
                TransferArgs {
                    memo: Memo(0),                                                                  // custom-memo? 
                    amount: icp_payout_quest.icp,
                    fee: DEFAULT_FEE,
                    from_subaccount: user_icp_balance_subaccount(&caller()),
                    to: icp_payout_quest.payout_icp_id,                        
                    created_at_time: None
                }
            ).await; 

            match icp_transfer_call {
                Ok(transfer_result) => match transfer_result {
                    Ok(block_index) => CollectBalanceSponse::icp_payout(Ok(block_index)),
                    Err(transfer_error) => CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferError(transfer_error)))
                },
                Err(e) => CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferCallError(format!("{:?}", e))))
            }
        },

        
        CollectBalanceQuest::cycles_payout(cycles_payout_quest) => {

            if cycles_payout_quest.cycles == 0 {
                return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::InvalidCyclesAmount));
            }

            if cycles_payout_quest.cycles + CYCLES_TRANSFER_FEE > check_user_cycles_balance(&caller()) {
                return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::BalanceTooLow));
            }

            let cycles_transfer_call: CallResult<Vec<u8>> = call_raw(
                cycles_payout_quest.payout_cycles_transfer_canister,
                "cycles_transfer",
                encode_one(&CyclesTransfer { memo: CyclesTransferMemo::text("CTS-PAYOUT".to_string()) }).unwrap(),
                cycles_payout_quest.cycles as u64  // as u64 for now till cdk compatible with the u128
            ).await;
            
            let cycles_accepted: u128 = ( cycles_payout_quest.cycles as u64 - msg_cycles_refunded() ) as u128;          // u64 for now till cdk compatible with the u128

            USERS_DATA.with(|ud| {
                ud.borrow().get(user).unwrap().cycles_balance -= cycles_accepted + CYCLES_TRANSFER_FEE;          // can unwrap here because of the checks [a]bove, that the user's-balance is greater than 1
            });

            // check if it is possible for the canister to reject/trap but still keep the cycles. if yes, [re]turn the cycles_accepted in the error. for now, going as if not possible.

            match cycles_transfer_call {
                Ok(_) => CollectBalanceSponse::cycles_payout(Ok(cycles_accepted)),
                Err(cycles_transfer_call_error) => CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::CyclesTransferCallError(format!("{:?}", cycles_transfer_call_error))))
            }
            
            


        }

    }
}



#[derive(CandidType, Deserialize)]
struct ConvertIcpBalanceForCyclesWithTheCmcRateQuest {
    icp: IcpTokens
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
enum PurchaseCyclesTransferError {          // same as CyclesPayoutError ?
    BalanceTooLow,
    CanisterDoesNotExist,
    NoCyclesTransferMethodOnTheCanister,

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



