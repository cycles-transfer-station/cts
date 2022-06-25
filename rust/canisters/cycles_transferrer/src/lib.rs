use cts_lib::{
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                call,
                call_raw128,
                CallRawFuture,
                CallResult,
                arg_data,
                reply,
                RejectionCode,
                msg_cycles_accept128,
                msg_cycles_available128,
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
        CyclesTransfer,
        CyclesTransferMemo,
        cts::{
            CyclesTransferrerUserTransferCyclesCallbackQuest,
        },
        user_canister::{
            CyclesTransferPurchaseLogId
        },
        cycles_transferrer::{
            CyclesTransferrerInit,
            CTSUserTransferCyclesQuest,
            CTSUserTransferCyclesError
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
        WASM_PAGE_SIZE_BYTES
    }
};
use std::cell::{Cell, RefCell};


pub type CyclesTransferRefund = Cycles;








pub const MAX_ONGOING_CYCLES_TRANSFERS: usize = 1000;


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;



thread_local! {
    static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static ONGOING_CYCLES_TRANSFERS_COUNT: Cell<usize> = Cell::new(0);
    static RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS: RefCell<Vec<((u32, String), CyclesTransferrerUserTransferCyclesCallbackQuest, CyclesTransferRefund)>> = RefCell::new(Vec::new()); // (cycles_transferrer_user_transfer_cycles_call_error, CyclesTransferrerUserTransferCyclesCallbackQuest, CyclesTransferRefund)

    // not save in a CTCData
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}



#[init]
fn init(cycles_transferrer_init: CyclesTransferrerInit) {
    CTS_ID.with(|cts_id| { cts_id.set(cycles_transferrer_init.cts_id); });
}

#[derive(CandidType, Deserialize)]
struct CTCData {
    cts_id: Principal,
    ongoing_cycles_transfers_count: usize,
    re_try_cycles_transferrer_user_transfer_cycles_callbacks: Vec<((u32, String), CyclesTransferrerUserTransferCyclesCallbackQuest, CyclesTransferRefund)>
}

fn create_ctc_data_candid_bytes() -> Vec<u8> {
    let mut ctc_data_candid_bytes: Vec<u8> = encode_one(
        &CTCData{
            cts_id: get(&CTS_ID),
            ongoing_cycles_transfers_count: get(&ONGOING_CYCLES_TRANSFERS_COUNT),
            re_try_cycles_transferrer_user_transfer_cycles_callbacks: with(&RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS, |re_try_cycles_transferrer_user_transfer_cycles_callbacks| {
                (*re_try_cycles_transferrer_user_transfer_cycles_callbacks).clone()
            })
        }
    ).unwrap();
    ctc_data_candid_bytes.shrink_to_fit();
    ctc_data_candid_bytes
}


fn re_store_ctc_data_candid_bytes(ctc_data_candid_bytes: Vec<u8>) {
    let ctc_data: CTCData = decode_one::<CTCData>(&ctc_data_candid_bytes).unwrap();
    set(&CTS_ID, ctc_data.cts_id);
    set(&ONGOING_CYCLES_TRANSFERS_COUNT, ctc_data.ongoing_cycles_transfers_count);
    with_mut(&RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS, |re_try_cycles_transferrer_user_transfer_cycles_callbacks| {
        *re_try_cycles_transferrer_user_transfer_cycles_callbacks = ctc_data.re_try_cycles_transferrer_user_transfer_cycles_callbacks;
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    let ctc_upgrade_data_candid_bytes: Vec<u8> = create_ctc_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*u64 len of the ctc_upgrade_data_candid_bytes*/ + ctc_upgrade_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES) + 1).unwrap();
    }
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((ctc_upgrade_data_candid_bytes.len() as u64).to_be_bytes()));
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &ctc_upgrade_data_candid_bytes);
}

#[post_upgrade]
fn post_upgrade() {
    let mut ctc_upgrade_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut ctc_upgrade_data_candid_bytes_len_u64_be_bytes);
    let ctc_upgrade_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(ctc_upgrade_data_candid_bytes_len_u64_be_bytes); 
    
    let mut ctc_upgrade_data_candid_bytes: Vec<u8> = vec![0; ctc_upgrade_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut ctc_upgrade_data_candid_bytes);
    
    re_store_ctc_data_candid_bytes(ctc_upgrade_data_candid_bytes);
}




// --------------------------------------------------

fn cts_id() -> Principal {
    CTS_ID.with(|cts_id| { cts_id.get() })
}



// ---------------------------------------------------


// (cts_q: CTSUserTransferCyclesQuest) -> Result<(), CTSUserTransferCyclesError>
#[update(manual_reply = true)]
pub async fn cts_user_transfer_cycles() {
    
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    
    if ONGOING_CYCLES_TRANSFERS_COUNT.with(|octs| octs.get()) >= MAX_ONGOING_CYCLES_TRANSFERS {
        reply::<(Result<(), CTSUserTransferCyclesError>,)>((Err(CTSUserTransferCyclesError::MaxOngoingCyclesTransfers(MAX_ONGOING_CYCLES_TRANSFERS)),));
        return;
    }
    
    let (cts_q,): (CTSUserTransferCyclesQuest,) = arg_data::<(CTSUserTransferCyclesQuest,)>();
    
    let cycles_transfer_candid: Vec<u8> = match candid::utils::encode_one(
        &CyclesTransfer{ memo: cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles_transfer_memo.clone() }
    ) {
        Ok(candid_bytes) => candid_bytes,    
        Err(candid_error) => {
            reply::<(Result<(), CTSUserTransferCyclesError>,)>((Err(CTSUserTransferCyclesError::CyclesTransferQuestCandidCodeError(format!("{:?}", candid_error))),));    
            return;
        
        },
    };
    
    // make sure to cept the cts cycles for the call after any possibility of a reply() and return; make sure errors after here before the cycles_transfer_call_future.await are trap so that the state rolls back and the cts gets the cycles back 
    // cept the cts cycles before the cycles_transfer call
    let cycles: Cycles = msg_cycles_accept128(msg_cycles_available128());
    if cycles != cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles {
        trap("check the cts call of this cycles_transferrer.")
    }
    
    ONGOING_CYCLES_TRANSFERS_COUNT.with(|octs| { octs.set(octs.get() + 1); }); // checked add?
    
    reply::<(Result<(), CTSUserTransferCyclesError>,)>((Ok(()),)); /// test that at the next commit point, the cts is replied-to without waiting for the cycles_transfer call to come back 
    
    // call_raw because dont want to rely on the canister returning the correct candid 
    let cycles_transfer_call_future: CallRawFuture = call_raw128(
        cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.canister_id,
        "cycles_transfer",
        &cycles_transfer_candid,
        cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles,
    );
    
    if cycles_transfer_call_future.call_perform_status_code != 0 {
        // a trap will refund the already accepted cycles(from the cts-main) and discard the reply(to the cts-main) 
        trap(&format!("cycles_transfer call_perform error: {:?}", RejectionCode::from(cycles_transfer_call_future.call_perform_status_code)));
    }
    
    let cycles_transfer_call_result: CallResult<Vec<u8>> = cycles_transfer_call_future.await;
    
    let cycles_transfer_refund: Cycles = msg_cycles_refunded128(); // now that we are for sure in a callback

    let cycles_transfer_call_error: Option<(u32, String)>;
    
    match cycles_transfer_call_result {
        Ok(_) => {
            cycles_transfer_call_error = None;
        },
        Err(call_error) => {
            cycles_transfer_call_error = Some((call_error.0 as u32, call_error.1));
        }
    }
    
    
    // we make a new call here because we already replied to the cts before the cycles_transfer call.
    do_cycles_transferrer_user_transfer_cycles_callback(
        CyclesTransferrerUserTransferCyclesCallbackQuest{
            cycles_transfer_call_error,
            cts_user_transfer_cycles_quest: cts_q
        },
        cycles_transfer_refund
    ).await;
    
    
}


async fn do_cycles_transferrer_user_transfer_cycles_callback(cycles_transferrer_user_transfer_cycles_callback_quest: CyclesTransferrerUserTransferCyclesCallbackQuest, cycles_transfer_refund: Cycles) {

    let cycles_transferrer_user_transfer_cycles_callback_call_future: CallRawFuture = call_raw128(
        cts_id(),
        "cycles_transferrer_user_transfer_cycles_callback",
        &candid::utils::encode_one(&cycles_transferrer_user_transfer_cycles_callback_quest).unwrap(), // .unwrap ? test it before the ployment
        cycles_transfer_refund
    );
    
    if cycles_transferrer_user_transfer_cycles_callback_call_future.call_perform_status_code != 0 {
        // log and re-try in a heartbeat or similar
        // in the re-try, make sure to give the cts back the user_transfer_cycles-refund 
        with_mut(&RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS, |rcs| {
            rcs.push(((cycles_transferrer_user_transfer_cycles_callback_call_future.call_perform_status_code, "call_perform error".to_string()), cycles_transferrer_user_transfer_cycles_callback_quest, cycles_transfer_refund));
        });
        return;
    }

    let cycles_transferrer_user_transfer_cycles_callback_call_result: CallResult<Vec<u8>> = cycles_transferrer_user_transfer_cycles_callback_call_future.await;  
    
    match cycles_transferrer_user_transfer_cycles_callback_call_result {
        Ok(_) => {
            // for the cts of the cycles_transferrer_user_transfer_cycles_callback_call-cept is with the cept of the cycles of this call for the user for the re-fund   
            ONGOING_CYCLES_TRANSFERS_COUNT.with(|octs| { octs.set(octs.get() - 1); }); // checked_sub? 
        },
        Err(cycles_transferrer_user_transfer_cycles_callback_call_error) => {
            // cts no cept the cycles in a case of a call-error here
            // log and re-try in a heartbeat or similar
            with_mut(&RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS, |rcs| {
                rcs.push(((cycles_transferrer_user_transfer_cycles_callback_call_error.0 as u32, cycles_transferrer_user_transfer_cycles_callback_call_error.1), cycles_transferrer_user_transfer_cycles_callback_quest, cycles_transfer_refund)); 
            });
        }
    }
    
}


#[update]
pub async fn re_try_cycles_transferrer_user_transfer_cycles_callbacks() {
    
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
   
    for i in 0..with(&RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS, |rcs| rcs.len()) {
        
        let possible_re_try_callback: Option<((u32, String), CyclesTransferrerUserTransferCyclesCallbackQuest, CyclesTransferRefund)> = with_mut(&RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS, |rcs| { rcs.pop() });
        
        if let Some(re_try_callback) = possible_re_try_callback {
            do_cycles_transferrer_user_transfer_cycles_callback(re_try_callback.1, re_try_callback.2).await;
        }
        
    }
    
}



#[export_name = "canister_query cts_see_re_try_cycles_transferrer_user_transfer_cycles_callbacks"]
pub fn cts_see_re_try_cycles_transferrer_user_transfer_cycles_callbacks() {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    
    with(&RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS, |rcs| {
        reply::<(&Vec<((u32, String), CyclesTransferrerUserTransferCyclesCallbackQuest, Cycles)>,)>((rcs,));
    })
}














// -----------------------------------------------------------------------------------








#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the cts for this method.")
    }
    set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the cts for this method.")
    }
    get(&STOP_CALLS)
}





#[update]
pub fn cts_create_state_snapshot() -> usize/*len of the state_snapshot_candid_bytes*/ {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        *state_snapshot_ctc_data_candid_bytes = create_ctc_data_candid_bytes();
        state_snapshot_ctc_data_candid_bytes.len()
    })
}



// chunk_size = 1mib


#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the cts for this method.")
    }
    let chunk_size: usize = 1024*1024;
    with(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        let (chunk_i,): (usize,) = arg_data::<(usize,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_ctc_data_candid_bytes.chunks(chunk_size).nth(chunk_i),));
    })

}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        *state_snapshot_ctc_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        state_snapshot_ctc_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_ctc_data_out_of_the_state_snapshot() {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the cts for this method.")
    }
    re_store_ctc_data_candid_bytes(
        with_mut(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
            let mut v: Vec<u8> = Vec::new();
            v.append(state_snapshot_ctc_data_candid_bytes);  // moves the bytes out of the state_snapshot vec
            v
        })
    );

}




// -------------------------------------------------------------------------








