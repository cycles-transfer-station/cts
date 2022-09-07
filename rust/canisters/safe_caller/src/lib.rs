// either take the cycles and make the call or don't take any cycles and don't make the call. don't take cycles and not make a call.

use cts_lib::{
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            canister_balance128,
            performance_counter,
            call::{
                call_with_payment128,
                call_raw128,
                CallResult,
                arg_data,
                arg_data_raw_size,
                reply,
                RejectionCode,
                msg_cycles_available128,
                msg_cycles_accept128,
                msg_cycles_refunded128    
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
                self, 
                CandidType, 
                Deserialize,
                utils::{encode_one, decode_one}
            },
        },
    },
    ic_cdk_macros::{
        init,
        pre_upgrade,
        post_upgrade,
        update,
        query
    },
    types::{
        Cycles,
        safe_caller::{
            SafeCallerInit,
            SafeCallQuest,
            SafeCallError,
            SafeCallCallbackQuest
        },
        management_canister::{
            CanisterIdRecord
        }
    },
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
    consts::{
        WASM_PAGE_SIZE_BYTES,
        MANAGEMENT_CANISTER_ID,
    }
};
use std::cell::{Cell, RefCell};
use futures::task::Poll;


type CyclesRefund = Cycles;

#[derive(CandidType, Deserialize)]
pub struct TryCallback { 
    original_caller_canister_id: Principal,
    callback_method: String,
    callback_quest: SafeCallCallbackQuest, 
    cycles_refund: CyclesRefund,
    try_number: u32,
    call_error_of_the_last_try: (RejectionCode, String)/*the call-error of the last try*/
}


#[derive(CandidType, Deserialize)]
struct SCData {
    cts_id: Principal,
    ongoing_safe_calls_count: u64,
    try_callbacks: Vec<TryCallback>
}
impl SCData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            ongoing_safe_calls_count: 0,
            try_callbacks: Vec::new()
        }
    }
}



pub const SAFE_CALL_FEE: Cycles = 20_000_000_000;

pub const MAX_ONGOING_SAFE_CALLS: u64 = 2000;

pub const MAX_CALLBACK_TRIES: u32 = 3;

const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;



thread_local! {
    
    static SC_DATA: RefCell<SCData> = RefCell::new(SCData::new()); 
    
    // not save in a SCData
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_SC_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}



#[init]
fn init(safe_caller_init: SafeCallerInit) {
    with_mut(&SC_DATA, |sc_data| {
        sc_data.cts_id = safe_caller_init.cts_id;
    });
}


fn create_sc_data_candid_bytes() -> Vec<u8> {
    let mut sc_data_candid_bytes: Vec<u8> = with(&SC_DATA, |sc_data| { encode_one(sc_data).unwrap() });
    sc_data_candid_bytes.shrink_to_fit();
    sc_data_candid_bytes
}


fn re_store_sc_data_candid_bytes(sc_data_candid_bytes: Vec<u8>) {
    let sc_data: SCData = match decode_one::<SCData>(&sc_data_candid_bytes) {
        Ok(sc_data) => sc_data,
        Err(_) => {
            trap("error decode of the sc_data");
            /*
            let old_sc_data: OldCTCData = decode_one::<OldSCData>(&sc_data_candid_bytes).unwrap();
            let sc_data: SCData = SCData{
                cts_id: old_sc_data.cts_id,
                ......
            };
            sc_data
            */
        }
    };
    
    std::mem::drop(sc_data_candid_bytes);
    
    with_mut(&SC_DATA, |scd| {
        *scd = sc_data;
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    let sc_upgrade_data_candid_bytes: Vec<u8> = create_sc_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*u64 len of the sc_upgrade_data_candid_bytes*/ + sc_upgrade_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
    }
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((sc_upgrade_data_candid_bytes.len() as u64).to_be_bytes()));
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &sc_upgrade_data_candid_bytes);
}

#[post_upgrade]
fn post_upgrade() {
    let mut sc_upgrade_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut sc_upgrade_data_candid_bytes_len_u64_be_bytes);
    let sc_upgrade_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(sc_upgrade_data_candid_bytes_len_u64_be_bytes); 
    
    let mut sc_upgrade_data_candid_bytes: Vec<u8> = vec![0; sc_upgrade_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut sc_upgrade_data_candid_bytes);
    
    re_store_sc_data_candid_bytes(sc_upgrade_data_candid_bytes);
}




// --------------------------------------------------

fn cts_id() -> Principal {
    with(&SC_DATA, |sc_data| { sc_data.cts_id })
}



// ---------------------------------------------------


// (q: SafeCallQuest) -> Result<(), SafeCallError> 
#[update(manual_reply = true)]
pub async fn safe_call() {
    
    let original_caller_canister_id: Principal = caller();

    if with(&SC_DATA, |sc_data| { sc_data.ongoing_safe_calls_count }) >= MAX_ONGOING_SAFE_CALLS 
    || localkey::cell::get(&STOP_CALLS) == true {
        reply::<(Result<(), SafeCallError>,)>((Err(SafeCallError::SafeCallerIsBusy),));    
        return;
    }
    
    let (q,): (SafeCallQuest,) = arg_data::<(SafeCallQuest,)>();
    
    let msg_cycles_quirement: Cycles = match q.cycles.checked_add(SAFE_CALL_FEE) {
        Some(msg_cycles_quirement) => msg_cycles_quirement,
        None => {
            reply::<(Result<(), SafeCallError>,)>((Err(SafeCallError::MsgCyclesTooLow{ safe_call_fee: SAFE_CALL_FEE }),));    
            return;
        }
    };
                
    if msg_cycles_available128() < msg_cycles_quirement {
        reply::<(Result<(), SafeCallError>,)>((Err(SafeCallError::MsgCyclesTooLow{ safe_call_fee: SAFE_CALL_FEE }),));    
        return;    
    }

    /*
    if msg_cycles_available128() < q.cycles.checked_add(SAFE_CALL_FEE).unwrap_or_else(|| { trap("cycles overflow. cycles + safe_call_fee u128 overflow. ") }) {
        reply::<(Result<(), SafeCallError>,)>((Err(SafeCallError::MsgCyclesTooLow{ safe_call_fee: SAFE_CALL_FEE }),));    
        return;
    } 
    */
    
    
    // make sure to cept the cts cycles for the call after any possibility of a reply(Err()) and return; make sure errors after here before the call_future.await are trap so that the state rolls back and the caller gets the cycles back 
    // cept the cts cycles before the cycles_transfer call //  and before calling reply or reject if want the cycles.
    msg_cycles_accept128(msg_cycles_quirement); // already checked that there is correct mount of the cycles       
    
    with_mut(&SC_DATA, |sc_data| { sc_data.ongoing_safe_calls_count += 1; });
    
    // call_raw because dont want to rely on the canister returning the correct candid 
    let mut call_future = call_raw128(
        q.callee,
        &q.method,
        &q.data,
        q.cycles,
    );
    
    if let Poll::Ready(call_result) = futures::poll!(&mut call_future) {
        // a trap will refund the already accepted cycles
        trap(&format!("call_perform error: {:?}", call_result.unwrap_err()));
    }
    
    reply::<(Result<(), SafeCallError>,)>((Ok(()),)); // at the next commit point, the cts is replied-to without waiting for the cycles_transfer call to come back 
    
    let call_result: CallResult<Vec<u8>> = call_future.await;
    
    let cycles_refund: Cycles = msg_cycles_refunded128(); // now that we are for the sure in a callback
    
    // we make a new call here because we already replied to the original-caller before the cycles_transfer call.
    do_callback(
        original_caller_canister_id,
        q.callback_method,
        SafeCallCallbackQuest{
            call_id: q.call_id,
            call_result,
        },
        cycles_refund,
        1
    ).await;
    
}


async fn do_callback(original_caller_canister_id: Principal, callback_method: String, callback_quest: SafeCallCallbackQuest, cycles_refund: CyclesRefund, try_number: u32) {
    
    let callback_quest_cb_result = candid::utils::encode_one(&callback_quest); // before the move into the closure.
    
    let g = || {
        with_mut(&SC_DATA, |sc_data| { sc_data.ongoing_safe_calls_count = sc_data.ongoing_safe_calls_count.saturating_sub(1); });
    };
    
    let log_try = |callback_call_error: (RejectionCode, String), try_n: u32| {
        with_mut(&SC_DATA, |sc_data| {
            sc_data.try_callbacks.push(
                TryCallback { 
                    original_caller_canister_id,
                    callback_method: callback_method.clone(),
                    callback_quest, 
                    cycles_refund,
                    try_number: try_n,
                    call_error_of_the_last_try: callback_call_error,
                }
            );             
        });
    };
    
    if try_number <= MAX_CALLBACK_TRIES {
    
        let callback_quest_data: Vec<u8> = match callback_quest_cb_result {
            Ok(b) => b,
            Err(candid_error) => {
                log_try((RejectionCode::Unknown, format!("candid code SafeCallCallbackQuest error: {:?}", candid_error)), try_number);
                return;
            }
        };

        let mut callback_call_future = call_raw128(
            original_caller_canister_id,
            &callback_method,
            &callback_quest_data,
            cycles_refund
        );
        
        if let Poll::Ready(call_result) = futures::poll!(&mut callback_call_future) {
            log_try(call_result.unwrap_err(), try_number);
            return;
        }
        
        match callback_call_future.await {
            Ok(_) => {
                g();
            },
            Err(callback_call_error) => {
                if msg_cycles_refunded128() != cycles_refund {
                    g();
                    return;
                }
                log_try(callback_call_error, try_number + 1);
            }
        }
       
    } else {
    
        match call_with_payment128::<(CanisterIdRecord,),()>(
            MANAGEMENT_CANISTER_ID,
            "deposit_cycles",
            (CanisterIdRecord{
                canister_id: original_caller_canister_id
            },),
            cycles_refund
        ).await {
            Ok(()) => {
                g();
            },
            Err(call_error) => {
                log_try(call_error, try_number + 1);
            }
        }
        
    }
    
}


#[update(manual_reply = true)]
pub async fn cts_do_try_callbacks() {
    
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
   
    futures::future::join_all(
        with_mut(&SC_DATA, |sc_data| {
            sc_data.try_callbacks.drain(..).map(
                |try_callback: TryCallback| {
                    do_callback(
                        try_callback.original_caller_canister_id,
                        try_callback.callback_method,
                        try_callback.callback_quest, 
                        try_callback.cycles_refund,
                        try_callback.try_number
                    )
                }
            ).collect::<Vec<_/*anonymous-future*/>>()
        })
    ).await;
    
    with(&SC_DATA, |sc_data| {
        reply::<(&Vec<TryCallback>,)>((&(sc_data.try_callbacks),));
    });
    
}



#[export_name = "canister_query cts_see_try_callbacks"]
pub fn cts_see_try_callbacks() {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    
    with(&SC_DATA, |sc_data| {
        reply::<(&Vec<TryCallback>,)>((&(sc_data.try_callbacks),));
    });
}


#[export_name = "canister_update cts_drain_try_callbacks"]
pub fn cts_drain_try_callbacks() {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    with_mut(&SC_DATA, |sc_data| {
        reply::<(Vec<TryCallback>,)>((sc_data.try_callbacks.drain(..).collect::<Vec<TryCallback>>(),));
    });    
}


#[update]
pub fn cts_put_try_callback(try_callback: TryCallback) {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    with_mut(&SC_DATA, |sc_data| {
        sc_data.try_callbacks.push(try_callback);
    });    
}









// -----------------------------------------------------------------------------------








#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    get(&STOP_CALLS)
}





#[update]
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot_candid_bytes*/ {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_SC_DATA_CANDID_BYTES, |state_snapshot_sc_data_candid_bytes| {
        *state_snapshot_sc_data_candid_bytes = create_sc_data_candid_bytes();
        state_snapshot_sc_data_candid_bytes.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    let chunk_size: usize = 1024*1024;
    with(&STATE_SNAPSHOT_SC_DATA_CANDID_BYTES, |state_snapshot_sc_data_candid_bytes| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_sc_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    })

}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_SC_DATA_CANDID_BYTES, |state_snapshot_sc_data_candid_bytes| {
        *state_snapshot_sc_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_SC_DATA_CANDID_BYTES, |state_snapshot_sc_data_candid_bytes| {
        state_snapshot_sc_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_sc_data_out_of_the_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    re_store_sc_data_candid_bytes(
        with_mut(&STATE_SNAPSHOT_SC_DATA_CANDID_BYTES, |state_snapshot_sc_data_candid_bytes| {
            let mut v: Vec<u8> = Vec::new();
            v.append(state_snapshot_sc_data_candid_bytes);  // moves the bytes out of the state_snapshot vec
            v
        })
    );

}




// -------------------------------------------------------------------------




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



// ---------------------------------------------------------------------------------



