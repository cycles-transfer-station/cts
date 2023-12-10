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
                call_raw128
            },
        },
        update, 
        query, 
        init, 
        pre_upgrade, 
        post_upgrade
    },
    tools::{
        sha256,
        localkey::{
            refcell::{
                with, 
                with_mut,
            },
        },
        upgrade_canisters::*,
        caller_is_controller_gaurd,
    },
    types::{
        Cycles,
        canister_code::CanisterCode,
        cbs_map::{
            CBSMInit,
            CBSMUserData,
            PutNewUserError,
            UpdateUserError,
            UpdateUserResult,
            CBSMUserDataUpdateFields,
        },
    },
    global_allocator_counter::get_allocated_bytes_count
};

use candid::{
    Principal,
    CandidType,
    Deserialize,
};
use serde::{Serialize};
      
use canister_tools::MemoryId;


type UsersMap = HashMap<Principal, CBSMUserData>;

#[derive(Serialize, Deserialize)]
struct CBSMData {
    cts_id: Principal,
    users_map: UsersMap,
    cycles_bank_canister_code: CanisterCode,
}
impl CBSMData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            users_map: UsersMap::new(),
            cycles_bank_canister_code: CanisterCode::empty(),
        }
    }
}



const CBSM_DATA_UPGRADE_MEMORY_ID: MemoryId = MemoryId::new(0);

const MAX_USERS: usize = 30_000; 



thread_local! {
    static CBSM_DATA: RefCell<CBSMData> = RefCell::new(CBSMData::new());
    // not save in a CBSMData
    static STOP_CALLS: Cell<bool> = Cell::new(false);
}


// ------------------------------------------------------------------------------------



#[init]
fn init(users_map_canister_init: CBSMInit) {
    canister_tools::init(&CBSM_DATA, CBSM_DATA_UPGRADE_MEMORY_ID);
    
    with_mut(&CBSM_DATA, |cbsm_data| {
        cbsm_data.cts_id = users_map_canister_init.cts_id; 
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    canister_tools::post_upgrade(&CBSM_DATA, CBSM_DATA_UPGRADE_MEMORY_ID, None::<fn(CBSMData) -> CBSMData>)
}

#[no_mangle]
pub fn canister_inspect_message() {
    trap("This canister is talked-to by the cts-canisters");
}




// ------------------------------------------------------------------------------------




fn cts_id() -> Principal {
    with(&CBSM_DATA, |cbsm_data| { cbsm_data.cts_id })
}


fn is_full() -> bool {
    with(&CBSM_DATA, |cbsm_data| { cbsm_data.users_map.len() }) >= MAX_USERS
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
    let (user_id, umc_user_data): (Principal, CBSMUserData) = arg_data::<(Principal, CBSMUserData)>();
    with_mut(&CBSM_DATA, |cbsm_data| {
        match cbsm_data.users_map.get(&user_id) {
            Some(found_umc_user_data) => {
                reply::<(Result<(), PutNewUserError>,)>((Err(PutNewUserError::FoundUser((*found_umc_user_data).clone())),));
            },
            None => {
                cbsm_data.users_map.insert(user_id, umc_user_data);
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
    let (user_id,): (Principal,) = arg_data::<(Principal,)>();
    with(&CBSM_DATA, |cbsm_data| {
        reply::<(Option<&CBSMUserData>,)>((cbsm_data.users_map.get(&user_id),));
    });
}



#[export_name = "canister_update void_user"]
pub fn void_user() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    let (user_id,): (Principal,) = arg_data::<(Principal,)>();
    with_mut(&CBSM_DATA, |cbsm_data| {
        reply::<(Option<CBSMUserData>,)>((cbsm_data.users_map.remove(&user_id),));
    });
}





#[update]
pub fn update_user(user_id: Principal, update_fields: CBSMUserDataUpdateFields) -> UpdateUserResult {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    with_mut(&CBSM_DATA, |cbsm_data| {
        match cbsm_data.users_map.get_mut(&user_id) {
            Some(found_user_data) => {
                if let Some(change_cycles_bank_canister_id) = update_fields.cycles_bank_canister_id {
                    found_user_data.cycles_bank_canister_id = change_cycles_bank_canister_id; 
                }
                if let Some(change_first_membership_creation_timestamp_nanos) = update_fields.first_membership_creation_timestamp_nanos {
                    found_user_data.first_membership_creation_timestamp_nanos = change_first_membership_creation_timestamp_nanos; 
                }
                if let Some(change_cb_latest_known_module_hash) = update_fields.cycles_bank_latest_known_module_hash {
                    found_user_data.cycles_bank_latest_known_module_hash = change_cb_latest_known_module_hash; 
                }
                if let Some(change_cb_lifetime_termination_timestamp_seconds) = update_fields.cycles_bank_lifetime_termination_timestamp_seconds {
                    found_user_data.cycles_bank_lifetime_termination_timestamp_seconds = change_cb_lifetime_termination_timestamp_seconds; 
                }
                if let Some(change_membership_termination_cb_uninstall_data) = update_fields.membership_termination_cb_uninstall_data {
                    found_user_data.membership_termination_cb_uninstall_data = change_membership_termination_cb_uninstall_data; 
                }
                Ok(())  
            },
            None => {
                Err(UpdateUserError::UserNotFound)
            }
        }
    })
}



  
  
 
// ----- STOP_CALLS-METHODS --------------------------
 

#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.set(stop_calls_flag); });
}

#[query]
pub fn cts_view_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    STOP_CALLS.with(|stop_calls| { stop_calls.get() })
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
    
    with_mut(&CBSM_DATA, |cbsm_data| {
        cbsm_data.cycles_bank_canister_code = canister_code;
    });
}

#[query(manual_reply = true)]
pub fn cts_view_uc_code_module_hash() {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }
    with(&CBSM_DATA, |cbsm_data| {
        reply::<(&[u8; 32],)>((cbsm_data.cycles_bank_canister_code.module_hash(),));
    });
}



#[update]
pub async fn controller_upgrade_cbs_chunk(q: ControllerUpgradeCSQuest) -> Vec<(Principal, UpgradeOutcome)> {
    caller_is_controller_gaurd(&caller());
    
    let cc: CanisterCode = with_mut(&CBSM_DATA, |cbsm_data| {
        if let Some(new_canister_code) = q.new_canister_code {
            new_canister_code.verify_module_hash().unwrap();
            cbsm_data.cycles_bank_canister_code = new_canister_code; 
        }
        cbsm_data.cycles_bank_canister_code.clone()
    });
    
    let users_cbs: Vec<(Principal, Principal)> = match q.specific_cs {
        Some(specific_cs) => {
            let ones_in_the_users_map = with(&CBSM_DATA, |cbsm_data| {
                cbsm_data.users_map.iter()
                .filter_map(|(user_id, d)| {
                    if specific_cs.contains(&d.cycles_bank_canister_id) {
                        Some((user_id.clone(), d.cycles_bank_canister_id.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()                
            });
            if ones_in_the_users_map.len() != specific_cs.len() {
                trap(&format!("cbsm users_map does not contain some of the cycles-banks set in the specific_cs parameter."));
            }
            ones_in_the_users_map
        }
        None => {
            with(&CBSM_DATA, |cbsm_data| {
                cbsm_data.users_map.iter()
                .filter_map(|(user_id, d)| {
                    if &d.cycles_bank_latest_known_module_hash != cc.module_hash() 
                    && d.cycles_bank_latest_known_module_hash != [0u8; 32] {            // [0u8; 32] means that the canister is still in the middle of the purchase_cycles_bank method. 
                        Some((user_id.clone(), d.cycles_bank_canister_id.clone()))
                    } else {
                        None
                    }
                })
                .take(200)
                .collect()
            })
        }
    };
    
    let (users, cbs): (Vec<Principal>, Vec<Principal>) = users_cbs.into_iter().unzip();
    
    let rs: Vec<(Principal, UpgradeOutcome)> = upgrade_canisters(cbs, &cc, &q.post_upgrade_quest).await;
    
    with_mut(&CBSM_DATA, |cbsm_data| {
        for (user_id, (_cb, uo)) in users.into_iter().zip(rs.iter()) {
            if let Some(ref r) = uo.install_code_result {
                if r.is_ok() {
                    if let Some(d) = cbsm_data.users_map.get_mut(&user_id) {
                        d.cycles_bank_latest_known_module_hash = cc.module_hash().clone();
                    } else {
                        ic_cdk::print("check this");
                    } 
                }
            }
        } 
    });
    
    return rs;
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
}

#[query]
pub fn cts_view_metrics() -> UMCMetrics {
    if caller() != cts_id() {
        trap("caller must be the CTS");
    }

    with(&CBSM_DATA, |cbsm_data| {
        UMCMetrics {
            global_allocator_counter: get_allocated_bytes_count() as u64,    
            stable_size: ic_cdk::api::stable::stable64_size(),
            cycles_balance: ic_cdk::api::canister_balance128(),
            user_canister_code_hash: if cbsm_data.cycles_bank_canister_code.module().len() != 0 { Some(cbsm_data.cycles_bank_canister_code.module_hash().clone()) } else { None },
            users_map_len: cbsm_data.users_map.len() as u64,
        }
    })
}













