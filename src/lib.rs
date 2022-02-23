
// lock each user from making other calls on each async call that awaits, like the collect_balance call, lock the user at the begining and unlock the user at the end. 
// will callbacks (the code after an await) get dropped if the subnet is under heavy load?


#![allow(unused)] // take this out when done


use std::cell::RefCell;
use std::collections::HashMap;
use ic_cdk::{
    api::{
        trap,
        caller, 
        time, 
        call::{
            call_raw,
            RejectionCode,
            method_name,
            msg_cycles_refunded,
            CallResult,
        },
    },
    export::{
        Principal,
        candid::{
            CandidType,
            Deserialize,
            utils::{encode_one, decode_one},
        },
    },
};
use ic_cdk_macros::{update, query};
use ic_ledger_types::{
    Memo as IcpMemo,
    AccountIdentifier as IcpId,
    Subaccount as IcpIdSub,
    Tokens as IcpTokens,
    BlockIndex as IcpBlockIndex,
    Timestamp as IcpTimestamp,
    DEFAULT_SUBACCOUNT as ICP_DEFAULT_SUBACCOUNT,
    DEFAULT_FEE as ICP_LEDGER_TRANSFER_FEE,
};



mod tools;
use tools::{
    user_icp_balance_subaccount,
    user_icp_balance_id,
    user_cycles_balance_topup_memo_bytes,
    check_user_icp_balance,
    check_user_cycles_balance,
    main_cts_icp_id,
    check_lock_and_lock_user,
    unlock_user,
    
};



pub const ICP_PAYOUT_FEE: IcpTokens = IcpTokens::from_e8s(1000000);      // calculate through the xdr conversion rate ?                                               
pub const CYCLES_TRANSFER_FEE: u128 = 300_000_000_000;
pub const ICP_PAYOUT_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-POUT"));                                     // b"CTS-POUT"
pub const ICP_TAKE_PAYOUT_FEE_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-TFEE"));                            // b"CTS-TFEE"



pub struct UserData {
    pub user_lock: UserLock,
    pub cycles_balance: u128,
    pub untaken_icp_to_collect: IcpTokens,
    


    //pub cycles_transfer_purchases: Vec<CyclesTransferPurchaseLog>, // 

}

impl Default for UserData {
    fn default() -> Self {
        UserData {
            user_lock: UserLock {
                lock: false,
                last_lock_time_nanos: 0
            },
            cycles_balance: 0u128,
            untaken_icp_to_collect: IcpTokens::ZERO,
            

        }
    }
}

pub struct UserLock {
    pub lock: bool,
    pub last_lock_time_nanos: u64 
}



thread_local! {
    pub static USERS_DATA: RefCell<HashMap<Principal, UserData>> = RefCell::new(HashMap::<Principal, UserData>::new());    
}










#[derive(CandidType, Deserialize)]
pub enum CyclesTransferMemo {
    Text(String),
    Nat64(u64),
    Blob(Vec<u8>)
}

#[derive(CandidType, Deserialize)]
pub struct CyclesTransfer {
    memo: CyclesTransferMemo
}



#[update]
pub fn cycles_transfer(ct: CyclesTransfer) -> () {

}









#[derive(CandidType, Deserialize)]
pub struct TopUpCyclesBalanceData {
    topup_cycles_transfer_memo: CyclesTransferMemo
} 

#[derive(CandidType, Deserialize)]
pub struct TopUpIcpBalanceData {
    topup_icp_id: IcpId
} 

#[derive(CandidType, Deserialize)]
pub struct TopUpBalanceData {
    topup_cycles_balance: TopUpCyclesBalanceData, 
    topup_icp_balance: TopUpIcpBalanceData,
}


#[update]
pub fn topup_balance() -> TopUpBalanceData {
    TopUpBalanceData {
        topup_cycles_balance: TopUpCyclesBalanceData {
            topup_cycles_transfer_memo: CyclesTransferMemo::Blob(user_cycles_balance_topup_memo_bytes(&caller()).to_vec())
        },
        topup_icp_balance: TopUpIcpBalanceData {
            topup_icp_id: user_icp_balance_id(&caller())
        }
    }
}










#[derive(CandidType, Deserialize)]
pub struct UserBalance {
    cycles_balance: u128,
    icp_balance: IcpTokens, 
}

#[derive(CandidType, Deserialize)]
pub enum SeeBalanceError {
    IcpLedgerCheckBalanceCallError(String),
}

pub type SeeBalanceSponse = Result<UserBalance, SeeBalanceError>;

#[update]
pub async fn see_balance() -> SeeBalanceSponse {
    check_lock_and_lock_user(&caller());
    let cycles_balance: u128 = check_user_cycles_balance(&caller());
    let icp_balance: IcpTokens = match check_user_icp_balance(&caller()).await {
        Ok(tokens) => tokens,
        Err(balance_call_error) => {
            unlock_user(&caller());
            return Err(SeeBalanceError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error)));
        } 
    };
    unlock_user(&caller());
    Ok(UserBalance {
        cycles_balance,
        icp_balance,
    })
}











#[derive(CandidType, Deserialize)]
pub struct IcpPayoutQuest {
    icp: IcpTokens,
    payout_icp_id: IcpId
}

#[derive(CandidType, Deserialize)]
pub struct CyclesPayoutQuest {
    cycles: u128,
    payout_cycles_transfer_canister: Principal         // the memo is: cts-payout    
}

#[derive(CandidType, Deserialize)]
pub enum CollectBalanceQuest {
    icp_payout(IcpPayoutQuest),
    cycles_payout(CyclesPayoutQuest)
}

#[derive(CandidType, Deserialize)]
pub enum IcpPayoutError {
    InvalidIcpAmount,
    IcpLedgerCheckBalanceCallError(String),
    BalanceTooLow { max_icp_payout: IcpTokens },
    IcpLedgerTransferError(ic_ledger_types::TransferError),
    IcpLedgerTransferCallError(String),


}

#[derive(CandidType, Deserialize)]
pub enum CyclesPayoutError {
    InvalidCyclesAmount,
    BalanceTooLow { max_cycles_payout: u128 },
    // CanisterDoesNotExist,
    // NoCyclesTransferMethodOnTheCanister,
    CyclesTransferCallError { call_error: String, paid_fee: bool }, // fee_paid: u128 ??
}

pub type IcpPayoutSponse = Result<IcpBlockIndex, IcpPayoutError>;

pub type CyclesPayoutSponse = Result<u128, CyclesPayoutError>;

#[derive(CandidType, Deserialize)]
pub enum CollectBalanceSponse {
    icp_payout(IcpPayoutSponse),
    cycles_payout(CyclesPayoutSponse)
}

#[update]
pub async fn collect_balance(collect_balance_quest: CollectBalanceQuest) -> CollectBalanceSponse {
    check_lock_and_lock_user(&caller());

    match collect_balance_quest {

        CollectBalanceQuest::icp_payout(icp_payout_quest) => {
            
            if icp_payout_quest.icp == IcpTokens::ZERO {
                unlock_user(&caller());
                return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::InvalidIcpAmount));
            } 
            
            let user_icp_balance: IcpTokens = match check_user_icp_balance(&caller()).await {
                Ok(icp_tokens) => icp_tokens,
                Err(balance_call_error) => {
                    unlock_user(&caller());
                    return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error))));
                }
            };
            
            if icp_payout_quest.icp + ICP_PAYOUT_FEE + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_FEE.e8s() * 2) > user_icp_balance {
                unlock_user(&caller());
                return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::BalanceTooLow { max_icp_payout: user_icp_balance - ICP_PAYOUT_FEE - IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_FEE.e8s() * 2) }));
            }
            
            use ic_ledger_types::{transfer, MAINNET_LEDGER_CANISTER_ID, TransferArgs, TransferResult, TransferError};
            
            let icp_payout_transfer_call: CallResult<TransferResult> = transfer(
                MAINNET_LEDGER_CANISTER_ID,
                TransferArgs {
                    memo: ICP_PAYOUT_MEMO,
                    amount: icp_payout_quest.icp,
                    fee: ICP_LEDGER_TRANSFER_FEE,
                    from_subaccount: Some(user_icp_balance_subaccount(&caller())),
                    to: icp_payout_quest.payout_icp_id,
                    created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
                }
            ).await; 

           let icp_payout_transfer_call_block_index: IcpBlockIndex = match icp_payout_transfer_call {
                Ok(transfer_result) => match transfer_result {
                    Ok(block_index) => block_index,
                    Err(transfer_error) => {
                        unlock_user(&caller());
                        return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferError(transfer_error)));
                    }
                },
                Err(transfer_call_error) => {
                    unlock_user(&caller());
                    return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferCallError(format!("{:?}", transfer_call_error))));
                }
            };

            let icp_payout_take_fee_transfer_call: CallResult<TransferResult> = transfer(
                MAINNET_LEDGER_CANISTER_ID,
                TransferArgs {
                    memo: ICP_TAKE_PAYOUT_FEE_MEMO,
                    amount: ICP_PAYOUT_FEE,
                    fee: ICP_LEDGER_TRANSFER_FEE,
                    from_subaccount: Some(user_icp_balance_subaccount(&caller())),
                    to: main_cts_icp_id(),                        
                    created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
                }
            ).await;             

            match icp_payout_take_fee_transfer_call {
                Ok(transfer_result) => match transfer_result {
                    Ok(block_index) => {},
                    Err(transfer_error) => {
                        USERS_DATA.with(|ud| {
                            ud.borrow_mut().entry(caller()).or_default().untaken_icp_to_collect += ICP_PAYOUT_FEE + ICP_LEDGER_TRANSFER_FEE;
                        });
                    }  // log and take into the count 
                },
                Err(transfer_call_error) => { // log and take into the count
                    USERS_DATA.with(|ud| {
                        ud.borrow_mut().entry(caller()).or_default().untaken_icp_to_collect += ICP_PAYOUT_FEE + ICP_LEDGER_TRANSFER_FEE;
                    });
                }
            }
            unlock_user(&caller());
            return CollectBalanceSponse::icp_payout(Ok(icp_payout_transfer_call_block_index));
        },





        CollectBalanceQuest::cycles_payout(cycles_payout_quest) => {

            if cycles_payout_quest.cycles == 0 {
                unlock_user(&caller());
                return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::InvalidCyclesAmount));
            }

            let user_cycles_balance: u128 = check_user_cycles_balance(&caller());

            if cycles_payout_quest.cycles + CYCLES_TRANSFER_FEE > user_cycles_balance {
                unlock_user(&caller());
                return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::BalanceTooLow { max_cycles_payout: user_cycles_balance - CYCLES_TRANSFER_FEE }));
            }

            let cycles_transfer_call: CallResult<Vec<u8>> = call_raw(
                cycles_payout_quest.payout_cycles_transfer_canister,
                "cycles_transfer",
                encode_one(&CyclesTransfer { memo: CyclesTransferMemo::Text("CTS-PAYOUT".to_string()) }).unwrap(),
                cycles_payout_quest.cycles as u64  // as u64 for now till cdk compatible with the u128
            ).await;
            
            // check if it is possible for the canister to reject/trap but still keep the cycles. if yes, [re]turn the cycles_accepted in the error. for now, going as if not possible.

            match cycles_transfer_call {
                Ok(_) => {
                    let cycles_accepted: u128 = cycles_payout_quest.cycles - msg_cycles_refunded() as u128; 
                    USERS_DATA.with(|ud| { ud.borrow_mut().get_mut(&caller()).unwrap().cycles_balance -= cycles_accepted + CYCLES_TRANSFER_FEE; });          // can unwrap here because of the checks [a]bove, that the user's-balance is greater than 1
                    unlock_user(&caller());
                    return CollectBalanceSponse::cycles_payout(Ok(cycles_accepted));
                },
                Err(cycles_transfer_call_error) => {
                    match cycles_transfer_call_error.0 {
                        RejectionCode::DestinationInvalid | RejectionCode::CanisterReject | RejectionCode::CanisterError => {
                            USERS_DATA.with(|ud| { ud.borrow_mut().get_mut(&caller()).unwrap().cycles_balance -= CYCLES_TRANSFER_FEE; });
                            unlock_user(&caller());
                            return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: true }));
                        },
                        _ => {
                            unlock_user(&caller());
                            return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: false }));
                        }
                    }
                }
            }


            
        }
    }
}



#[derive(CandidType, Deserialize)]
struct ConvertIcpBalanceForTheCyclesWithTheCmcRateQuest {
    icp: IcpTokens
}

#[derive(CandidType, Deserialize)]
enum ConvertIcpBalanceForTheCyclesWithTheCmcRateError {

}


// #[update]
// pub async fn convert_icp_balance_for_the_cycles_with_the_cmc_rate(ConvertIcpBalanceForTheCyclesWithTheCmcRateQuest) -> Result<u128, ConvertIcpBalanceForTheCyclesWithTheCmcRateError> {

// }



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

// #[update]
// pub async fn purchase_cycles_transfer(PurchaseCyclesTransferQuest) -> Result<u128, PurchaseCyclesTransferError> {

// }





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

// #[update]
// pub async fn purchase_cycles_bank(q: PurchaseCyclesBankQuest) -> Result<CyclesBankPurchaseLog, PurchaseCyclesBankError> {

// }




#[derive(CandidType, Deserialize)]
struct CyclesTransferPurchaseLog {
    r#for: Principal,
    cycles_sent: u128,
    cycles_accepted: u128, // 64?
    cycles_transfer_memo: CyclesTransferMemo,
    timestamp: u64,
}

// #[update]
// pub fn see_cycles_transfer_purchases(page: u128) -> Vec<CyclesTransferPurchaseLog> {

// }


// #[update]
// pub fn see_cycles_bank_purchases(page: u128) -> Vec<CyclesBankPurchaseLog> {

// }



#[derive(CandidType, Deserialize)]
struct Fees {
    purchase_cycles_bank_cost_cycles: u128,
    purchase_cycles_transfer_cost_cycles: u128
}

// #[update]
// pub fn see_fees() -> Fees {
    
// }








#[no_mangle]
pub fn canister_inspect_message() {
    // caution: this function is only called for ingress messages 
    
    if ["topup_balance", "see_balance", "collect_balance", ].contains(&&method_name()[..]) {
        if caller() == Principal::anonymous() { // check '==' plementation is correct otherwise caller().as_slice() == Principal::anonymous().as_slice()
            trap("caller cannot be anonymous for this method.")
        }
    }
}



