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

use cts_lib::{
    tools::localkey_refcell::{with, with_mut},
    types::{
        Cycles,
        UserData,
        UserLock
    },
    ic_ledger_types::{
        IcpTokens
    }
};

type UsersMap = HashMap<Principal, (UserData, UserLock)>;




pub const MAX_CANISTER_SIZE: usize =  1 * 1024*1024*1024;// bytes // 1 GiB // each user-map-canister can hold round 10_000_000 users. test

pub const CALLERS_WHITELIST: [1; Principal] = [Principal::from_slice(&[thp4z-canister-bytes])];


thread_local! {
    static USERS_MAP: RefCell<UsersMap> = RefCell::new(UsersMap::new());
    //static CALLERS_WHITELIST: RefCell<Vec<Principal>> = RefCell::new(Vec::new());
}






fn check_caller(caller: &Principal) {
    //if with(&CALLERS_WHITELIST, |cw| { cw.contains(caller) }) == false {
    if CALLERS_WHITELIST.contains(caller) == false {
        trap("unknown caller")
    }
}


fn is_full() -> bool {
    get_allocated_bytes_count() > MAX_CANISTER_SIZE
}



#[init]
fn init(/*callers_whitelist: Vec<Principal>*/) -> () {
    //with_mut(&CALLERS_WHITELIST, |cw| { *cw = callers_whitelist; });
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
pub fn put(user_id: Principal, user_data: UserData, mut user_lock: Option<UserLock>) -> Result<(), PutError>{
    check_caller(&caller());

    with_mut(&USERS_MAP, |um| { 
        if um.contains_key(&user_id) == false {
            if is_full() {
                Err(PutError::CanisterIsFull)
            }
            if user_lock.is_none() {
                user_lock = Some(UserLock::new());
            }   
        } else {
            if user_lock.is_none() {
                user_lock = *um.get(&user_id).unwrap().1;     // can unwrap because of the if containskey check 
            }   
        }
        
        um.insert(user_id, (user_data, user_lock.unwrap())); // unwrap because makes sure the user_lock-option is at this point a Some 
        Ok(())
    })
}

#[derive(CandidType, Deserialize)]
pub enum PlusBalanceError {

}

#[update]
pub fn plus_balance(user_id: Principal, plus_cycles: Option<Cycles>, plus_icp: Option<IcpTokens>) -> Result<(), PlusBalanceError> {
    Ok(())    
} 



#[update]
pub fn void_user(user_id: Principal) -> Option<(UserData, UserLock)> {
    check_caller(&caller());

    with_mut(&USERS_MAP, |um| { 
        um.remove(&user_id)
    })


}


/*
#[query]
pub fn get(user: Principal) -> Option<UserData> {
    check_caller(&caller());
    
    with(&USERS_MAP, |um| {
        match um.get(&user) {
            Some(u) => Some(*u.0),
            None => None
        }
    })
}
*/


#[derive(CandidType, Deserialize)]
pub enum GetAndLockError {
    UserNotFound,
    UserLockIsOn,
}

#[update]
pub fn get_and_lock(user_id:&Principal) -> Result<UserData, GetAndLockError> {
    check_caller(&caller());
    
    with_mut(&USERS_MAP, |um| {
        match um.get_mut(user_id) {
            Some(u) => {
                if u.1.is_lock_on() == true {
                    Err(GetAndLockError::UserLockIsOn)
                }
                u.1.lock();
                Ok(*u.0)
            },
            None => Err(GetAndLockError::UserNotFound)
        }
    })
}










#[query]
pub fn see_allocated_bytes() -> usize {
    check_caller();
    get_allocated_bytes_count()
}



