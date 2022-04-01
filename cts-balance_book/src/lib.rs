use std::cell::RefCell;
use std::collections::HashMap;
use ic_cdk::{
    api::{
        caller, 
        time, 
        trap,
        call::{
            call,
            CallResult,
            RejectionCode,
        },
    },
    export::{
        Principal,
        candid::{
            CandidType,
            Deserialize,
        },
    },
};
use ic_cdk_macros::{update, query};




struct UserBalance {
    pub cycles_balance: u128,
    pub untaken_icp_to_collect: IcpTokens,
    
}


thread_local! {
    static BALANCE_BOOK: RefCell<HashMap<Principal, UserBalance>> = RefCell::new(HashMap::new());
}