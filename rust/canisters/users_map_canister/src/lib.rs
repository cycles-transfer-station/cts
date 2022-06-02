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



thread_local! {
    static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static USERS_MAP: RefCell<UsersMap> = RefCell::new(UsersMap::new());
}



fn cts_id() -> Principal {
    CTS_ID.with(|cts_id| cts_id.get())
}


fn is_full() -> bool {
    get_allocated_bytes_count() > MAX_CANISTER_SIZE
}







#[init]
fn init(users_map_canister_init: UsersMapCanisterInit) -> () {
    CTS_ID.with(|cts_id| { cts_id.set(users_map_canister_init.cts_id); });
}

#[pre_upgrade]
fn pre_upgrade() {

}

#[post_upgrade]
fn post_upgrade() {
    
}









pub enum PutNewUserError {
    CanisterIsFull,
    FoundUser(UserData),
}

#[update]
pub fn put_new_user(user_id: Principal, user_data: UserData) -> Result<(), PutNewUserError> {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    if is_full() {
        return Err(PutNewUserError::CanisterIsFull);
    }
    with_mut(&USERS_MAP, |users_map| {
        match users_map.get(&user_id) {
            Some(user_data) => Err(PutNewUserError::FoundUser(*user_data)),
            None => {
                users_map.insert(user_id, user_data);
                Ok(())
            }
        }
    })
}




pub enum WriteUserDataError {
    UserNotFound,
}

#[update]
pub fn write_user_data(user_id: Principal, user_data: UserData) -> Result<(), WriteUserDataError> {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with_mut(&USERS_MAP, |users_map| {
        match users_map.get(&user_id) {
            Some(ud) => { 
                *ud = user_data;
                Ok(())
            },
            None => Err(WriteUserDataError::UserNotFound)
        }
    })
} 



#[query]
pub fn find_user(user_id: Principal) -> Option<&UserData> {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with(&USERS_MAP, |users_map| users_map.get(&user_id))
}





pub enum PlusUserCyclesBalanceError {
    UserNotFound,
}

#[update]
pub fn plus_user_cycles_balance(user_id: Principal, plus_cycles: Cycles) -> Result<(), PlusUserCyclesBalanceError> {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with_mut(&USERS_MAP, |users_map| {
        match users_map.get_mut(&user_id) {
            Some(user_data) => {
                user_data.cycles_balance += plus_cycles;
                Ok(())
            },
            None => {
                return Err(PlusUserCyclesBalanceError::UserNotFound);
            }
        }
    })
}






