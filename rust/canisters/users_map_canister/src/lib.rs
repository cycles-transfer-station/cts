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
            },
            stable::{
                stable64_grow,
                stable64_read,
                stable64_size,
                stable64_write,
            }
        },
        export::{
            Principal,
            candid::{
                CandidType,
                Deserialize,
                utils::{
                    encode_one,
                    decode_one
                }
            },
        }
    },
    ic_cdk_macros::{update, query, init, pre_upgrade, post_upgrade},
    tools::{
        localkey::{
            self,
            refcell::{
                with, 
                with_mut,
            },
            cell::{
                get,
                set
            }
        }
    },
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
    consts::{
        WASM_PAGE_SIZE_BYTES
    },
    global_allocator_counter::get_allocated_bytes_count
};


type UsersMap = HashMap<UserId, UserCanisterId>;






const MAX_CANISTER_SIZE: usize =  1024*1024*100;
const MAX_USERS: usize = 30_000; 


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;



thread_local! {
    static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static USERS_MAP: RefCell<UsersMap> = RefCell::new(UsersMap::new());

    // not save in a UsersMapCanisterData
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}


// ------------------------------------------------------------------------------------



#[init]
fn init(users_map_canister_init: UsersMapCanisterInit) {
    CTS_ID.with(|cts_id| { cts_id.set(users_map_canister_init.cts_id); });
}


#[derive(CandidType, Deserialize)]
struct UMCData {
    cts_id: Principal,
    users_map: Vec<(UserId, UserCanisterId)>
}


fn create_umc_data_candid_bytes() -> Vec<u8> {
    let mut umc_data_candid_bytes: Vec<u8> = encode_one(
        &UMCData {
            cts_id: cts_id(),
            users_map: with(&USERS_MAP, |users_map| { (*users_map).clone().into_iter().collect::<Vec<(UserId, UserCanisterId)>>() })
        }
    ).unwrap();
    umc_data_candid_bytes.shrink_to_fit();
    umc_data_candid_bytes
}

fn re_store_umc_data_candid_bytes(umc_data_candid_bytes: Vec<u8>) {
    let umc_data: UMCData = decode_one::<UMCData>(&umc_data_candid_bytes).unwrap();
    // std::mem::drop(umc_data_candid_bytes);
    CTS_ID.with(|cts_id| { cts_id.set(umc_data.cts_id); });
    with_mut(&USERS_MAP, |users_map| { *users_map = umc_data.users_map.into_iter().collect::<HashMap<UserId, UserCanisterId>>(); });
}


#[pre_upgrade]
fn pre_upgrade() {
    let umc_data_candid_bytes: Vec<u8> = create_umc_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*u64 len of the umc_data_candid_bytes*/ + umc_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES) + 1).unwrap();
    }
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((umc_data_candid_bytes.len() as u64).to_be_bytes()));
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &umc_data_candid_bytes);

}

#[post_upgrade]
fn post_upgrade() {
    let mut umc_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut umc_data_candid_bytes_len_u64_be_bytes);
    let umc_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(umc_data_candid_bytes_len_u64_be_bytes); 
    
    let mut umc_data_candid_bytes: Vec<u8> = vec![0; umc_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut umc_data_candid_bytes);
    
    re_store_umc_data_candid_bytes(umc_data_candid_bytes);
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
    get_allocated_bytes_count() >= MAX_CANISTER_SIZE || with(&USERS_MAP, |users_map| users_map.len()) >= MAX_USERS
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
    if STOP_CALLS.with(|stop_calls| { stop_calls.get() }) { trap("Maintenance. try again soon.") }
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



 // ------------------------------------------------------------------------------
 
 
 
 

#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.set(stop_calls_flag); });
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.get() })
}





#[update]
pub fn cts_create_state_snapshot() -> usize/*len of the state_snapshot_candid_bytes*/ {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        *state_snapshot_umc_data_candid_bytes = create_umc_data_candid_bytes();
        state_snapshot_umc_data_candid_bytes.len()
    })
}



// chunk_size = 1mib

#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    let chunk_size: usize = 1024*1024;
    with(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        let (chunk_i,): (usize,) = arg_data::<(usize,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_umc_data_candid_bytes.chunks(chunk_size).nth(chunk_i),));
    });
}


#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        *state_snapshot_umc_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        state_snapshot_umc_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_cts_data_out_of_the_state_snapshot() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    re_store_umc_data_candid_bytes(
        with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
            let mut v: Vec<u8> = Vec::new();
            v.append(state_snapshot_umc_data_candid_bytes);
            v
        })
    );

}




// ---------------------------------------------------------------


/*
#[update]
pub fn cts_upgrade_users_canisters(cts_q: CTSUpgradeUsersCanistersQuest) -> Vec<Principal> {

}
*/







