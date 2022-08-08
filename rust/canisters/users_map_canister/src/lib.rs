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
            canister_balance128,
            performance_counter,
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
            
        },
        users_map_canister::{
            UsersMapCanisterInit,
            UMCUserData,
            PutNewUserError,
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
        MiB,
        
    },
    global_allocator_counter::get_allocated_bytes_count
};




type UsersMap = HashMap<UserId, UMCUserData>;


#[derive(CandidType, Deserialize)]
struct UMCData {
    cts_id: Principal,
    users_map: UsersMap,
    user_canister_code: CanisterCode,
    user_canister_upgrade_fails: Vec<UMCUpgradeUCError>,
}
impl UMCData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            users_map: UsersMap::new(),
            user_canister_code: CanisterCode::new(Vec::new()),
            user_canister_upgrade_fails: Vec::new(),
        }
    }
}



const MAX_USERS: usize = 10_000; 

const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;

const USER_CANISTER_UPGRADES_CHUNK_SIZE: usize = 500;
const SEE_USER_CANISTER_UPGRADE_FAILS_CHUNK_SIZE: usize = 500;



thread_local! {
    static UMC_DATA: RefCell<UMCData> = RefCell::new(UMCData::new());
    
    // not save in a UMCData
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}


// ------------------------------------------------------------------------------------



#[init]
fn init(users_map_canister_init: UsersMapCanisterInit) {
    with_mut(&UMC_DATA, |umc_data| {
        umc_data.cts_id = users_map_canister_init.cts_id; 
    });
}





fn create_umc_data_candid_bytes() -> Vec<u8> {
    let mut umc_data_candid_bytes: Vec<u8> = with(&UMC_DATA, |umc_data| { encode_one(umc_data).unwrap() });
    umc_data_candid_bytes.shrink_to_fit();
    umc_data_candid_bytes
}

fn re_store_umc_data_candid_bytes(umc_data_candid_bytes: Vec<u8>) {
    let umc_data: UMCData = match decode_one::<UMCData>(&umc_data_candid_bytes) {
        Ok(umc_data) => umc_data,
        Err(_) => {
            trap("error decode of the UMCData");
            /*
            let old_umc_data: OldUMCData = decode_one::<OldUMCData>(&umc_data_candid_bytes).unwrap();
            let umc_data: UMCData = UMCData{
                cts_id: old_umc_data.cts_id,
                .......
            };
            umc_data
            */
        }
    };

    std::mem::drop(umc_data_candid_bytes);    

    with_mut(&UMC_DATA, |umcd| {
        *umcd = umc_data;
    });
    
}


#[pre_upgrade]
fn pre_upgrade() {
    let umc_data_candid_bytes: Vec<u8> = create_umc_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*u64 len of the umc_data_candid_bytes*/ + umc_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
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
    trap("This canister is talked-to by the cts-canisters");
}




// ------------------------------------------------------------------------------------




fn cts_id() -> Principal {
    with(&UMC_DATA, |umc_data| { umc_data.cts_id })
}


fn is_full() -> bool {
    with(&UMC_DATA, |umc_data| { umc_data.users_map.len() }) >= MAX_USERS
}



// ------------------------------------------------------------------------------------




#[export_name = "canister_update put_new_user"]
pub fn put_new_user() {             // Result<(), PutNewUserError> 
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    if is_full() {
        reply::<(Result<(), PutNewUserError>,)>((Err(PutNewUserError::CanisterIsFull),));
        return;
    }
    let (user_id, umc_user_data): (UserId, UMCUserData) = arg_data::<(UserId, UMCUserData)>();
    with_mut(&UMC_DATA, |umc_data| {
        match umc_data.users_map.get(&user_id) {
            Some(found_umc_user_data) => {
                reply::<(Result<(), PutNewUserError>,)>((Err(PutNewUserError::FoundUser((*found_umc_user_data).clone())),));
            },
            None => {
                umc_data.users_map.insert(user_id, umc_user_data);
                reply::<(Result<(), PutNewUserError>,)>((Ok(()),));
            }
        }
    });
}




#[export_name = "canister_query find_user"]
pub fn find_user() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    let (user_id,): (UserId,) = arg_data::<(UserId,)>();
    with(&UMC_DATA, |umc_data| {
        reply::<(Option<&UMCUserData>,)>((umc_data.users_map.get(&user_id),));
    });
}



#[export_name = "canister_update void_user"]
pub fn void_user() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    let (user_id,): (UserId,) = arg_data::<(UserId,)>();
    with_mut(&UMC_DATA, |umc_data| {
        reply::<(Option<UMCUserData>,)>((umc_data.users_map.remove(&user_id),));
    });
}


#[derive(CandidType, Deserialize)]
pub enum UpdateUserError {
    UserNotFound
}


#[export_name = "canister_update update_user"]
pub fn update_user() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    let (user_id, umc_user_data): (UserId, UMCUserData) = arg_data::<(UserId, UMCUserData)>();
    with_mut(&UMC_DATA, |umc_data| {
        match umc_data.users_map.get(&user_id) {
            Some(_found_umc_user_data) => {
                umc_data.users_map.insert(user_id, umc_user_data);
                reply::<(Result<(), UpdateUserError>,)>((Ok(()),));  
            },
            None => {
                reply::<(Result<(), UpdateUserError>,)>((Err(UpdateUserError::UserNotFound),));
            }
        }
    });
}




// ------------------------------------------------------------------------------------

  
 
// ----- STOP_CALLS-METHODS --------------------------
 

#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.set(stop_calls_flag); });
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.get() })
}





// ----- STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES-METHODS --------------------------

#[update]
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot_candid_bytes*/ {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        *state_snapshot_umc_data_candid_bytes = create_umc_data_candid_bytes();
        state_snapshot_umc_data_candid_bytes.len() as u64
    })
}



#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_umc_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    });
}


#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        *state_snapshot_umc_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    with_mut(&STATE_SNAPSHOT_UMC_DATA_CANDID_BYTES, |state_snapshot_umc_data_candid_bytes| {
        state_snapshot_umc_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_umc_data_out_of_the_state_snapshot() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
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

// ------ Upgrade user_canisters methods -----------------------------



#[update]
pub fn cts_put_user_canister_code(canister_code: CanisterCode) -> () {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    
    if sha256(canister_code.module()) != *canister_code.module_hash() {
        trap("Given canister_code.module_hash is different than the manual compute module hash");
    }
    
    with_mut(&UMC_DATA, |umc_data| {
        umc_data.user_canister_code = canister_code;
    });
}

#[query(manual_reply = true)]
pub fn cts_see_uc_code_module_hash() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    with(&UMC_DATA, |umc_data| {
        reply::<(&[u8; 32],)>((umc_data.user_canister_code.module_hash(),));
    });
}





#[update(manual_reply = true)]
pub async fn cts_upgrade_ucs_chunk() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    
    // make sure the user_canister_upgrade_fails vec is empty ?
    
    if with(&UMC_DATA, |umc_data| { umc_data.user_canister_code.module().len() == 0 }) {
        trap("No user-canister-code found on this umc");
    }

    let (opt_upgrade_ucs, post_upgrade_arg): (Option<Vec<UserCanisterId>>, Vec<u8>) = arg_data::<(Option<Vec<UserCanisterId>>, Vec<u8>)>();
    
    let upgrade_ucs: Vec<(UserId, UserCanisterId)> = {
        if let Some(q_upgrade_ucs) = opt_upgrade_ucs {
            if q_upgrade_ucs.len() > USER_CANISTER_UPGRADES_CHUNK_SIZE {
                trap(&format!("Max upgrade_ucs: {:?}", USER_CANISTER_UPGRADES_CHUNK_SIZE));
            }
            let mut q_upgrade_ucs_good_check_map: HashMap<UserCanisterId, Option<UserId>> = q_upgrade_ucs.into_iter().map(|q_upgrade_uc| { (q_upgrade_uc, None) }).collect::<HashMap<UserCanisterId, Option<UserId>>>();
            with(&UMC_DATA, |umc_data| {
                for (user_id, umc_user_data) in umc_data.users_map.iter() {
                    if q_upgrade_ucs_good_check_map.contains_key(&(umc_user_data.user_canister_id)) {
                        q_upgrade_ucs_good_check_map.insert(umc_user_data.user_canister_id, Some(/*copy*/*user_id));
                    }
                }
            });
            for (q_upgrade_uc, is_with_a_user_id) in q_upgrade_ucs_good_check_map.iter() {
                if is_with_a_user_id.is_none() {
                    trap(&format!("umc users_map does not contain the user_canister: {:?}", q_upgrade_uc));
                }
            }
            q_upgrade_ucs_good_check_map.into_iter().map(|(q_upgrade_uc, with_the_user_id): (UserCanisterId, Option<UserId>)| { (with_the_user_id.unwrap(), q_upgrade_uc) }).collect::<Vec<(UserId, UserCanisterId)>>()
        } else {
            let mut upgrade_ucs: Vec<(UserId, UserCanisterId)> = Vec::new();
            with(&UMC_DATA, |umc_data| { 
                let current_uc_code_module_hash: [u8; 32] = umc_data.user_canister_code.module_hash().clone();
                for (user_id, umc_user_data) in umc_data.users_map.iter() {
                    if upgrade_ucs.len() >= USER_CANISTER_UPGRADES_CHUNK_SIZE {
                        break;
                    }
                    if umc_user_data.user_canister_latest_known_module_hash != current_uc_code_module_hash {
                        upgrade_ucs.push((user_id.clone(), umc_user_data.user_canister_id.clone()));
                    }
                }
            });
            upgrade_ucs
        }
    };    
    
    if upgrade_ucs.len() == 0 {
        reply::<(Option<&Vec<UMCUpgradeUCError>>,)>((None,));
        return;
    }
    
    // PORTANT!! trying to do a loop here of any sort, for-loop or loop{}-block over chunks of the upgrade_ucs will cause memory corruption in a localkey.with closure within the async block  
    // Now one chunk per call.
   
    let sponses: Vec<Result<[u8; 32], UMCUpgradeUCError>> = futures::future::join_all(
        upgrade_ucs.iter().map(|upgrade_uc| {
            async {
                let (_user_id,upgrade_uc): (UserId, UserCanisterId) = upgrade_uc.clone();
                
                match call::<(CanisterIdRecord,), ()>(
                    MANAGEMENT_CANISTER_ID,
                    "stop_canister",
                    (CanisterIdRecord{ canister_id: upgrade_uc },)
                ).await {
                    Ok(_) => {},
                    Err(stop_canister_call_error) => {
                        return Err((upgrade_uc, UMCUpgradeUCCallErrorType::StopCanisterCallError, (stop_canister_call_error.0 as u32, stop_canister_call_error.1))); 
                    }
                }
                
                let user_canister_code_module_hash: [u8; 32] = with(&UMC_DATA, |umc_data| { umc_data.user_canister_code.module_hash().clone() });
                match call_raw128(
                    MANAGEMENT_CANISTER_ID,
                    "install_code",
                    &encode_one(&ManagementCanisterInstallCodeQuest{
                        mode : ManagementCanisterInstallCodeMode::upgrade,
                        canister_id : upgrade_uc,
                        wasm_module : unsafe{&*with(&UMC_DATA, |umc_data| { umc_data.user_canister_code.module() as *const Vec<u8> })},
                        arg : &post_upgrade_arg,
                    }).unwrap(),
                    0
                ).await {
                    Ok(_) => {},
                    Err(upgrade_code_call_error) => {
                        return Err((upgrade_uc, UMCUpgradeUCCallErrorType::UpgradeCodeCallError{wasm_module_hash: user_canister_code_module_hash}, (upgrade_code_call_error.0 as u32, upgrade_code_call_error.1)));
                    }
                }
                
                match call::<(CanisterIdRecord,), ()>(
                    MANAGEMENT_CANISTER_ID,
                    "start_canister",
                    (CanisterIdRecord{ canister_id: upgrade_uc },)
                ).await {
                    Ok(_) => {},
                    Err(start_canister_call_error) => {
                        return Err((upgrade_uc, UMCUpgradeUCCallErrorType::StartCanisterCallError, (start_canister_call_error.0 as u32, start_canister_call_error.1))); 
                    }
                }
                
                Ok(user_canister_code_module_hash)
            }
        }).collect::<Vec<_/*anonymous-future*/>>() 
    ).await;
        
    let mut/*mut for the append*/ current_upgrade_fails: Vec<UMCUpgradeUCError> = Vec::new();
    
    // doing this outside of the async blocks. i seen memory corruption in a localkey refcell with(&) in the async blocks. i rather keep the with_mut(&) out of it
    with_mut(&UMC_DATA, |umc_data| {
        for ((user_id, user_canister_id), sponse)/*: ((UserId,UserCanisterId),Result<[u8; 32], UMCUpgradeUCError>)*/ in upgrade_ucs.into_iter().zip(sponses.into_iter()) {    
            match sponse {
                Ok(user_canister_code_module_hash) => {
                    match umc_data.users_map.get_mut(&user_id) {
                        Some(umc_user_data) => {
                            (*umc_user_data).user_canister_latest_known_module_hash = user_canister_code_module_hash; 
                        },
                        None => {}
                    }
                },
                Err(umc_upgrade_uc_error) => {
                    current_upgrade_fails.push(umc_upgrade_uc_error);
                }
            }
        }
    });
    
    reply::<(Option<&Vec<UMCUpgradeUCError>>,)>((Some(&current_upgrade_fails),));
    
    with_mut(&UMC_DATA, |umc_data| {
        umc_data.user_canister_upgrade_fails.append(&mut current_upgrade_fails);
        std::mem::drop(current_upgrade_fails); // cause its empty by the append
    });
    
}







#[query(manual_reply = true)]
pub fn cts_see_user_canister_upgrade_fails() {
    if caller() != cts_id() {
        trap("caller must be the CTS");            
    }
    
    let (chunk_i,): (u64,) = arg_data::<(u64,)>();
    
    with(&UMC_DATA, |umc_data| {
        reply::<(Option<&[UMCUpgradeUCError]>,)>((umc_data.user_canister_upgrade_fails.chunks(SEE_USER_CANISTER_UPGRADE_FAILS_CHUNK_SIZE).nth(chunk_i.try_into().unwrap()),));
    });
}

#[update]
pub fn cts_clear_user_canister_upgrade_fails() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
        
    with_mut(&UMC_DATA, |umc_data| {
        umc_data.user_canister_upgrade_fails.clear();
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

#[update(manual_reply = true)]
pub async fn cts_call_canister() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
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
    global_allocator_counter: u64,
    stable_size: u64,
    cycles_balance: u128,
    user_canister_code_hash: Option<[u8; 32]>,
    users_map_len: u64,
    user_canister_upgrade_fails_len: u64,
}

#[query]
pub fn cts_see_metrics() -> UMCMetrics {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }

    with(&UMC_DATA, |umc_data| {
        UMCMetrics {
            global_allocator_counter: get_allocated_bytes_count() as u64,    
            stable_size: ic_cdk::api::stable::stable64_size(),
            cycles_balance: ic_cdk::api::canister_balance128(),
            user_canister_code_hash: if umc_data.user_canister_code.module().len() != 0 { Some(umc_data.user_canister_code.module_hash().clone()) } else { None },
            users_map_len: umc_data.users_map.len() as u64,
            user_canister_upgrade_fails_len: umc_data.user_canister_upgrade_fails.len() as u64,
        }
    })
}













