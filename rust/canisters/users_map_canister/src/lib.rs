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
                call_raw128
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
        sha256,
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
        Cycles,
        UserId,
        UserCanisterId,
        canister_code::CanisterCode,
        cts::{
            UMCUserTransferCyclesQuest,
            UMCUserTransferCyclesError
        },
        users_map_canister::{
            UsersMapCanisterInit,
            UCUserTransferCyclesQuest,
            UCUserTransferCyclesError,
            UMCUpgradeUCError,
            UMCUpgradeUCCallErrorType
        },
        management_canister::{
            CanisterIdRecord,
            ManagementCanisterInstallCodeQuest,
            ManagementCanisterInstallCodeMode
        }
    },
    consts::{
        WASM_PAGE_SIZE_BYTES,
        MANAGEMENT_CANISTER_ID,
        
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
    static USER_CANISTER_CODE: RefCell<CanisterCode> = RefCell::new(CanisterCode::new(Vec::new()));
    static USER_CANISTER_UPGRADE_FAILS: RefCell<Vec<UMCUpgradeUCError>> = RefCell::new(Vec::new());

    // not save in a UMCData
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
    users_map: Vec<(UserId, UserCanisterId)>,
    user_canister_code: CanisterCode,
    user_canister_upgrade_fails: Vec<UMCUpgradeUCError>,
}


fn create_umc_data_candid_bytes() -> Vec<u8> {
    let mut umc_data_candid_bytes: Vec<u8> = encode_one(
        &UMCData {
            cts_id: cts_id(),
            users_map: with(&USERS_MAP, |users_map| { (*users_map).clone().into_iter().collect::<Vec<(UserId, UserCanisterId)>>() }),
            user_canister_code: with(&USER_CANISTER_CODE, |user_canister_code| { (*user_canister_code).clone() }),
            user_canister_upgrade_fails: with(&USER_CANISTER_UPGRADE_FAILS, |user_canister_upgrade_fails| { (*user_canister_upgrade_fails).clone() })
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
    with_mut(&USER_CANISTER_CODE, |user_canister_code| { *user_canister_code = umc_data.user_canister_code; });
    with_mut(&USER_CANISTER_UPGRADE_FAILS, |user_canister_upgrade_fails| { *user_canister_upgrade_fails = umc_data.user_canister_upgrade_fails; });
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
// ---------- uc_-methods ---------------------------------------------



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
 
 
 
 
 // ----- STOP_CALLS-METHODS --------------------------
 

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





// ----- STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES-METHODS --------------------------

#[update]
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot_candid_bytes*/ {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        *state_snapshot_umc_data_candid_bytes = create_umc_data_candid_bytes();
        state_snapshot_umc_data_candid_bytes.len() as u64
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
        let (chunk_i,): (u32,) = arg_data::<(u32,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_umc_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
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

// ------ Upgrade User_Canisters methods -----------------------------


// see user_canister_code_module_hash . maybe in a metrics public-method



#[update]
pub fn cts_put_user_canister_code(canister_code: CanisterCode) -> () {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    
    if sha256(canister_code.module()) != *canister_code.module_hash() {
        trap("Given canister_code.module_hash is different than the manual compute module hash");
    }
    
    with_mut(&USER_CANISTER_CODE, |user_canister_code| {
        *user_canister_code = canister_code;
    });
}

#[query(manual_reply = true)]
pub fn cts_see_uc_code_module_hash() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    with(&USER_CANISTER_CODE, |uc_code| {
        reply::<(&[u8; 32],)>((uc_code.module_hash(),));
    })
}





const SEE_USER_CANISTER_UPGRADE_FAILS_CHUNK_SIZE: usize = 500;


// do 30_000 user_canisters per call just in a case that the principals need to come back if the upgrade doesn't go through ? or do everything in one but save the UCs that upgrade fail in a global?
#[update(manual_reply = true)]
pub async fn cts_upgrade_ucs() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    
    // make sure the user_canister_upgrade_fails vec is empty ?
    
    let (opt_upgrade_ucs, post_upgrade_arg): (Option<Vec<UserCanisterId>>, Vec<u8>) = arg_data::<(Option<Vec<UserCanisterId>>, Vec<u8>)>();
    
    let upgrade_ucs: Vec<UserCanisterId> = {
        if let Some(upgrade_ucs) = opt_upgrade_ucs {
            let mut upgrade_ucs_good_check_map: HashMap<UserCanisterId, bool> = upgrade_ucs.into_iter().map(|upgrade_uc| { (upgrade_uc, false) }).collect::<HashMap<UserCanisterId, bool>>();
            with(&USERS_MAP, |users_map| { 
                for users_map_user_canister in users_map.values() {
                    if upgrade_ucs_good_check_map.contains_key(users_map_user_canister) {
                        upgrade_ucs_good_check_map.insert(*users_map_user_canister, true);
                    }
                } 
            });
            for (upgrade_uc, is_in_the_users_map_values) in upgrade_ucs_good_check_map.iter() {
                if *is_in_the_users_map_values == false {
                    trap(&format!("umc users_map does not contain the user_canister: {:?}", upgrade_uc));
                }
            }
            upgrade_ucs_good_check_map.into_iter().map(|(upgrade_uc, _is_in_the_users_map_values): (UserCanisterId, bool)| { upgrade_uc }).collect::<Vec<UserCanisterId>>()
        } else {
            with(&USERS_MAP, |users_map| { users_map.values().map(|user_canister| { *user_canister/*copy*/ }).collect::<Vec<UserCanisterId>>() })
        }
    };    
    
    for upgrade_ucs_chunk in upgrade_ucs.chunks(500usize) {
        let sponses: Vec<Result<(), UMCUpgradeUCError>> = futures::future::join_all(
            upgrade_ucs_chunk.into_iter().map(|upgrade_uc| {
                async {
                
                    match call::<(CanisterIdRecord,), ()>(
                        MANAGEMENT_CANISTER_ID,
                        "stop_canister",
                        (CanisterIdRecord{ canister_id: *upgrade_uc },)
                    ).await {
                        Ok(_) => {},
                        Err(stop_canister_call_error) => {
                            return Err((*upgrade_uc, UMCUpgradeUCCallErrorType::StopCanisterCallError, (stop_canister_call_error.0 as u32, stop_canister_call_error.1))); 
                        }
                    }
                
                    match call_raw128(
                        MANAGEMENT_CANISTER_ID,
                        "install_code",
                        &encode_one(&ManagementCanisterInstallCodeQuest{
                            mode : ManagementCanisterInstallCodeMode::upgrade,
                            canister_id : *upgrade_uc,
                            wasm_module : unsafe { &*with(&USER_CANISTER_CODE, |uc_code| { uc_code.module() as *const Vec<u8> }) },
                            arg : &post_upgrade_arg,
                        }).unwrap(),
                        0
                    ).await {
                        Ok(_) => {},
                        Err(upgrade_code_call_error) => {
                            return Err((*upgrade_uc, UMCUpgradeUCCallErrorType::UpgradeCodeCallError, (upgrade_code_call_error.0 as u32, upgrade_code_call_error.1)));
                        }
                    }

                    match call::<(CanisterIdRecord,), ()>(
                        MANAGEMENT_CANISTER_ID,
                        "start_canister",
                        (CanisterIdRecord{ canister_id: *upgrade_uc },)
                    ).await {
                        Ok(_) => {},
                        Err(start_canister_call_error) => {
                            return Err((*upgrade_uc, UMCUpgradeUCCallErrorType::StartCanisterCallError, (start_canister_call_error.0 as u32, start_canister_call_error.1))); 
                        }
                    }
                    
                    Ok(())
                }
            }).collect::<Vec<_/*anonymous-future*/>>() 
        ).await;
 
        let mut/*mut for the append*/ upgrade_fails: Vec<UMCUpgradeUCError> = sponses.into_iter().filter_map(
            |sponse: Result<(), UMCUpgradeUCError>| {
                match sponse {
                    Ok(()) => None,
                    Err(umc_upgrade_uc_error) => Some(umc_upgrade_uc_error)
                }
            }
        ).collect::<Vec<UMCUpgradeUCError>>();
        
        with_mut(&USER_CANISTER_UPGRADE_FAILS, |user_canister_upgrade_fails| {
            user_canister_upgrade_fails.append(&mut upgrade_fails);
            std::mem::drop(upgrade_fails); // cause its empty by the append
        });
    }
    
    with(&USER_CANISTER_UPGRADE_FAILS, |user_canister_upgrade_fails| {
        if let Some(user_canister_upgrade_fails_chunk_0) = user_canister_upgrade_fails.chunks(SEE_USER_CANISTER_UPGRADE_FAILS_CHUNK_SIZE).nth(0) {
            reply::<(&[UMCUpgradeUCError],)>((user_canister_upgrade_fails_chunk_0,));
        } else {
            reply::<(&[UMCUpgradeUCError],)>((&[],));
        }
    });
}


#[query(manual_reply = true)]
pub fn cts_see_user_canister_upgrade_fails() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    
    let (chunk_i,): (u32,) = arg_data::<(u32,)>();
    
    with(&USER_CANISTER_UPGRADE_FAILS, |user_canister_upgrade_fails| {
        reply::<(Option<&[UMCUpgradeUCError]>,)>((user_canister_upgrade_fails.chunks(SEE_USER_CANISTER_UPGRADE_FAILS_CHUNK_SIZE).nth(chunk_i.try_into().unwrap()),));
    });
}

#[update]
pub fn cts_clear_user_canister_upgrade_fails() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
        
    with_mut(&USER_CANISTER_UPGRADE_FAILS, |user_canister_upgrade_fails| {
        user_canister_upgrade_fails.clear();
        // shrink to fit?
    });
}


// ---------------------------------------------------------------------------------


#[derive(CandidType, Deserialize)]
pub struct CTSCallCanisterQuest {
    callee: Principal,
    method_name: String,
    arg_raw: Vec<u8>,
    cycles: Cycles
}

#[update]
pub async fn cts_call_canister() {
    if caller() != cts_id() {
        trap("caller must be the CTS")
    }
    
    let (q,): (CTSCallCanisterQuest,) = arg_data::<(CTSCallCanisterQuest,)>(); 
    
    match call_raw128(
        q.callee,
        &q.method_name,
        &q.arg_raw,
        q.cycles   
    ).await {
        Ok(raw_sponse) => {
            reply::<(Result<Vec<u8>, (u32, String)>,)>((Ok(raw_sponse),));
        }, 
        Err(call_error) => {
            reply::<(Result<Vec<u8>, (u32, String)>,)>((Err((call_error.0 as u32, call_error.1)),));
        }
    }
}



// ------------ Metrics ------------------------------------

#[derive(CandidType, Deserialize)]
pub struct UMCMetrics {

}

#[query]
pub fn cts_metrics() -> UMCMetrics {
    trap("")

}













