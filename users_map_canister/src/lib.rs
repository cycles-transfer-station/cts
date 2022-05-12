use std::{
    cell::RefCell,
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
use ic_certified_map::{RbTree, HashTree};

use global_allocator_counter::get_allocated_bytes_count;

use cts_lib::tools::localkey_refcell::{with, with_mut};



type UsersMap = HashMap<Principal, Principal>;


pub const MAX_CANISTER_SIZE: usize =  1 * 1024*1024*1024;// bytes // 1 GiB


thread_local! {
    pub static USERS_MAP: RefCell<UsersMap> = RefCell::new(UsersMap::new());
    pub static CALLERS_WHITELIST: RefCell<Vec<Principal>> = RefCell::new(Vec::new());
}






fn check_caller(caller: &Principal) {
    if !with(&CALLERS_WHITELIST, |cw| { cw.contains(caller) }) {
        trap("caller not authorized")
    }
}


fn is_full() -> bool {
    get_allocated_bytes_count() > MAX_CANISTER_SIZE
}



#[init]
fn init(callers_whitelist: Vec<Principal>) -> () {
    with_mut(&CALLERS_WHITELIST, |cw| { *cw = callers_whitelist; });
}


#[pre_upgrade]
fn pre_upgrade() {

}

#[post_upgrade]
fn post_upgrade() {
    
}





#[derive(CandidType, Deserialize)]
pub enum PutError {
    CanisterIsFull
}

#[update]
pub fn put(user: Principal, user_canister: Principal) -> Result<(), PutError>{
    check_caller(&caller());

    if is_full() {
        return Err(PutError::CanisterIsFull);
    }

    with_mut(&USERS_MAP, |um| { 
        um.insert(user, user_canister); 
    });

    Ok(())
}


#[update]
pub fn void_user(user: Principal) {
    check_caller(&caller());

    with_mut(&USERS_MAP, |um| { 
        um.remove(&user); 
    });


}



// with the certified-data certificate?
#[query]
pub fn get(user_id: Principal) -> Option<Principal> { // do i want Result and GetError? or -> Option<Principal> by it's self { 
    with(&USERS_MAP, |um| {
        match um.get(&user_id) {
            Some(p) => Some(*p),
            None => None
        }
    })
}












#[query]
pub fn see_allocated_bytes() -> usize {
    get_allocated_bytes_count()
}



