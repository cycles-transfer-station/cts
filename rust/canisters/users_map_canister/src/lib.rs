use std::{
    cell::{Cell,RefCell},
    collections::HashMap,
};
use cts_lib::{
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                reply,
                arg_data,
                call,
            }
        },
        export::{
            Principal,
            candid::{
                CandidType,
                Deserialize,
            },
        }
    },
    ic_cdk_macros::{update, init, pre_upgrade, post_upgrade},
    tools::localkey_refcell::{with, with_mut},
    types::{
        UserId,
        UserCanisterId,
        cts::{
            UMCUserTransferCyclesQuest,
            UMCUserTransferCyclesError
        },
        users_map_canister::{
            UsersMapCanisterInit,
            UCUserTransferCyclesQuest,
            UCUserTransferCyclesError,
        }
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


// ------------------------------------------------------------------------------------



#[init]
fn init(users_map_canister_init: UsersMapCanisterInit) {
    CTS_ID.with(|cts_id| { cts_id.set(users_map_canister_init.cts_id); });
}

#[pre_upgrade]
fn pre_upgrade() {

}

#[post_upgrade]
fn post_upgrade() {
    
}

#[no_mangle]
pub fn canister_inspect_message() {
    trap("This canister is talked-to by the cts-canisters")
}




// ------------------------------------------------------------------------------------




fn cts_id() -> Principal {
    CTS_ID.with(|cts_id| cts_id.get())
}


fn is_full() -> bool {
    get_allocated_bytes_count() > MAX_CANISTER_SIZE || with(&USERS_MAP, |users_map| users_map.len()) >= MAX_USERS
}



// ------------------------------------------------------------------------------------




#[derive(CandidType,Deserialize)]
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
    let (user_id,): (UserId,) = arg_data::<(UserId,)>();
    with(&USERS_MAP, |users_map| {
        reply::<(Option<&UserCanisterId>,)>((users_map.get(&user_id),));
    });
}



#[update]
pub fn void_user(user_id: UserId) -> Option<UserCanisterId> {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with_mut(&USERS_MAP, |users_map| {
        users_map.remove(&user_id)
    })
}




// ------------------------------------------------------------------------------------






// Ok(()) means the cycles_transfer is in the call-queue

#[update]
pub async fn uc_user_transfer_cycles(uc_q: UCUserTransferCyclesQuest) -> Result<(), UCUserTransferCyclesError> {
    // caller-check
    with(&USERS_MAP, |users_map| { 
        match users_map.get(&uc_q.user_id) { 
            Some(user_canister_id) => { 
                if *user_canister_id != caller() { 
                    trap("caller of this method must be the user-canister") 
                }
            }, 
            None => trap("user_id not found in this canister") 
        } 
    });
    
    match call::<(UMCUserTransferCyclesQuest,), (Result<(), UMCUserTransferCyclesError>,)>(
        cts_id(),
        "umc_user_transfer_cycles",
        (UMCUserTransferCyclesQuest{
            user_canister_id: caller(),
            uc_user_transfer_cycles_quest: uc_q
        },)
    ).await {
        Ok((umc_user_transfer_cycles_sponse,)) => match umc_user_transfer_cycles_sponse {
            Ok(()) => return Ok(()),
            Err(umc_user_transfer_cycles_error) => return Err(UCUserTransferCyclesError::UMCUserTransferCyclesError(umc_user_transfer_cycles_error)) 
        },
        Err(umc_user_transfer_cycles_call_error) => return Err(UCUserTransferCyclesError::UMCUserTransferCyclesCallError(format!("{:?}", umc_user_transfer_cycles_call_error)))
    }
}













