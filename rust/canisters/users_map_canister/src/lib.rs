use std::{
    cell::RefCell,
    collections::HashMap,
};
use ic_cdk_macros::{update, query, init, pre_upgrade, post_upgrade};
use ic_cdk::{
    api::{
        trap,
        caller
    },
    export::{
        Principal,
        candid::{
            CandidType,
            Deserialize,
        },
    }
};
//use ic_certified_map::{RbTree, HashTree};

use cts_lib::{
    tools::localkey_refcell::{with, with_mut},
    types::{
        UserId,
        UserCanisterId
    },
    global_allocator_counter::get_allocated_bytes_count
};


type UsersMap = HashMap<UserId, UserCanisterId>;






const MAX_CANISTER_SIZE: usize =  1 * 1024*1024*1024 / 11;// bytes // 1 GiB // each user-map-canister can hold round 10_000_000 users. test
const MAX_USERS: usize = 2_000_000;



thread_local! {
    static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static USERS_MAP: RefCell<UsersMap> = RefCell::new(UsersMap::new());
}



fn cts_id() -> Principal {
    CTS_ID.with(|cts_id| cts_id.get())
}


fn is_full() -> bool {
    get_allocated_bytes_count() > MAX_CANISTER_SIZE || with(&USERS_MAP, |users_map| users_map.len()) >= MAX_USERS
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
    FoundUser(UserCanisterId)
}

#[update]
pub fn put_new_user(user_id: UserId, user_canister_id: UserCanisterId) -> Result<(), PutNewUserError> {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    if is_full() {
        return Err(PutNewUserError::CanisterIsFull);
    }
    with_mut(&USERS_MAP, |users_map| {
        match users_map.get(&user_id) {
            Some(user_canister_id) => Err(PutNewUserError::FoundUser(*user_canister_id)),
            None => {
                users_map.insert(user_id, user_canister_id);
                Ok(())
            }
        }
    })
}




#[export_name = "canister_query find_user"]
pub fn find_user() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    let (user_id,): (UserId,) = arg_data<(UserId,)>();
    with(&USERS_MAP, |users_map| {
        reply<(Option<&UserCanisterId>,)>((users_map.get(&user_id),));
    });
}



#[update]
pub fn void_user(user_id: UserId) -> Option<UserCanisterId> {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with(&USERS_MAP, |users_map| {
        users_map.remove(&user_id)
    })
}




