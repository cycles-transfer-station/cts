use global_allocator_counter::get_allocated_bytes_count;
use cts_lib::{
    types::{
        UserData,
        CyclesBankPurchaseLog,
        CyclesTransferPurchaseLog,
        UserCanisterInit,

    },
    tools::{
        localkey_refcell::{with, with_mut},
    }
};
use std::{
    cell::{RefCell,Cell},
    collections::HashMap,
};
use ic_cdk_macros::{update, query, init, pre_upgrade, post_upgrade};
use ic_cdk::{
    api::{
        trap,
        caller,


    },
    export::{
        Principal,
        candid::{
            CandidType,
            Deserialize,
        },
    }
};






thread_local! {
    static USER_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static USER_DATA: RefCell<UserData> = RefCell::new(UserData::new());    
    static CALLERS_WHITELIST: RefCell<Vec<Principal>> = RefCell::new(Vec::new());

}



fn get_user_id() -> Principal {
    USER_ID.with(|u| { u.get() })
}



#[init]
fn init(user_canister_init: UserCanisterInit) {
    USER_ID.with(|u| { u.set(user_canister_init.user); });
    with_mut(&CALLERS_WHITELIST, |cw| { *cw = user_canister_init.callers_whitelist; }); // if move error: do a  { cw.append(user_canister_init.callers_whitelist) }

}


#[pre_upgrade]
fn pre_upgrade() {

}



#[post_upgrade]
fn post_upgrade() {

}



#[update]
fn h() {
    
}


fn check_caller() {
    if !with(&CALLERS_WHITELIST, |cw| { cw.contains(&caller()) }) {
        trap("caller not authorized")
    }
}




#[export_name = "canister_query see_cycles_transfer_purchases"]
pub fn see_cycles_transfer_purchases<'a>() -> () {
    check_caller();

    let (param,): (u128,) = ic_cdk::api::call::arg_data::<(u128,)>(); 
    
    let user_cycles_transfer_purchases: *const Vec<CyclesTransferPurchaseLog> = with(&USER_DATA, |user_data| { 
        (&user_data.cycles_transfer_purchases) as *const Vec<CyclesTransferPurchaseLog>
    });

    // check if drop gets called after this call
    ic_cdk::api::call::reply::<(&'a Vec<CyclesTransferPurchaseLog>,)>((unsafe { &*user_cycles_transfer_purchases },))
}



#[export_name = "canister_query see_cycles_bank_purchases"]
pub fn see_cycles_bank_purchases<'a>() -> () {
    check_caller();

    let (param,): (u128,) = ic_cdk::api::call::arg_data::<(u128,)>(); 

    let user_cycles_bank_purchases: *const Vec<CyclesBankPurchaseLog> = with(&USER_DATA, |user_data| { 
        (&user_data.cycles_bank_purchases) as *const Vec<CyclesBankPurchaseLog>
    });

    ic_cdk::api::call::reply::<(&'a Vec<CyclesBankPurchaseLog>,)>((unsafe { &*user_cycles_bank_purchases },))

}
