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
                call,
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
        CyclesTransferMemo,
        cts::{

        },
        cycles_transferrer::{
            CyclesTransferrerCanisterInit,
            CyclesTransfer,
            TransferCyclesQuest,
            TransferCyclesError,
            TransferCyclesCallbackQuest
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
    }
};
use std::cell::{Cell, RefCell};
use futures::task::Poll;


type CyclesTransferRefund = Cycles;

#[derive(CandidType, Deserialize)]
pub struct TryTransferCyclesCallback { 
    call_error_of_the_last_try: (RejectionCode, String)/*the call-error of the last try*/, 
    transfer_cycles_callback_quest: TransferCyclesCallbackQuest, 
    cycles_transfer_refund: CyclesTransferRefund,
    try_number: u32
}


#[derive(CandidType, Deserialize)]
struct CTCData {
    cts_id: Principal,
    ongoing_cycles_transfers_count: u64,
    try_transfer_cycles_callbacks: Vec<TryTransferCyclesCallback>
}
impl CTCData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            ongoing_cycles_transfers_count: 0,
            try_transfer_cycles_callbacks: Vec::new()
        }
    }
}



pub const TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

pub const MAX_ONGOING_CYCLES_TRANSFERS: u64 = 1000;

const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;



thread_local! {
    
    static CTC_DATA: RefCell<CTCData> = RefCell::new(CTCData::new()); 
    
    // not save in a CTCData
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}



#[init]
fn init(cycles_transferrer_init: CyclesTransferrerCanisterInit) {
    with_mut(&CTC_DATA, |ctc_data| {
        ctc_data.cts_id = cycles_transferrer_init.cts_id;
    });
}


fn create_ctc_data_candid_bytes() -> Vec<u8> {
    let mut ctc_data_candid_bytes: Vec<u8> = with(&CTC_DATA, |ctc_data| { encode_one(ctc_data).unwrap() });
    ctc_data_candid_bytes.shrink_to_fit();
    ctc_data_candid_bytes
}


fn re_store_ctc_data_candid_bytes(ctc_data_candid_bytes: Vec<u8>) {
    let ctc_data: CTCData = match decode_one::<CTCData>(&ctc_data_candid_bytes) {
        Ok(ctc_data) => ctc_data,
        Err(_) => {
            trap("error decode of the ctc_data");
            /*
            let old_ctc_data: OldCTCData = decode_one::<OldCTCData>(&ctc_data_candid_bytes).unwrap();
            let ctc_data: CTCData = CTCData{
                cts_id: old_ctc_data.cts_id,
                ......
            };
            ctc_data
            */
        }
    };
    
    std::mem::drop(ctc_data_candid_bytes);
    
    with_mut(&CTC_DATA, |ctcd| {
        *ctcd = ctc_data;
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    let ctc_upgrade_data_candid_bytes: Vec<u8> = create_ctc_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*u64 len of the ctc_upgrade_data_candid_bytes*/ + ctc_upgrade_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
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
    with(&CTC_DATA, |ctc_data| { ctc_data.cts_id })
}



// ---------------------------------------------------


// (q: TransferCyclesQuest) -> Result<(), TransferCyclesError> 
#[update(manual_reply = true)]
pub async fn transfer_cycles() {
    
    if get(&STOP_CALLS) == true {
        trap("Maintenance. try later.")
    }
    
    if arg_data_raw_size() > 150 {
        trap("arg_data_raw_size too big");
    }
    
    let (q,): (TransferCyclesQuest,) = arg_data::<(TransferCyclesQuest,)>();
    
    if msg_cycles_available128() < q.cycles + TRANSFER_CYCLES_FEE {
        reply::<(Result<(), TransferCyclesError>,)>((Err(TransferCyclesError::MsgCyclesTooLow{ transfer_cycles_fee: TRANSFER_CYCLES_FEE }),));    
        return;
    } 
    
    if with(&CTC_DATA, |ctc_data| { ctc_data.ongoing_cycles_transfers_count }) >= MAX_ONGOING_CYCLES_TRANSFERS {
        reply::<(Result<(), TransferCyclesError>,)>((Err(TransferCyclesError::MaxOngoingCyclesTransfers),));    
        return;
    }
    
    
    let cycles_transfer_candid: Vec<u8> = match candid::utils::encode_one(
        &CyclesTransfer{ 
            memo: q.cycles_transfer_memo,
            original_caller: Some(caller())
        }
    ) {
        Ok(candid_bytes) => candid_bytes,    
        Err(candid_error) => {
            reply::<(Result<(), TransferCyclesError>,)>((Err(TransferCyclesError::CyclesTransferQuestCandidCodeError(format!("{:?}", candid_error))),));    
            return;        
        }
    };
    
    // make sure to cept the cts cycles for the call after any possibility of a reply(Err()) and return; make sure errors after here before the cycles_transfer_call_future.await are trap so that the state rolls back and the caller gets the cycles back 
    // cept the cts cycles before the cycles_transfer call
    msg_cycles_accept128(q.cycles + TRANSFER_CYCLES_FEE); // already checked that there is correct mount of the cycles        // before calling reply or reject.
    
    with_mut(&CTC_DATA, |ctc_data| { ctc_data.ongoing_cycles_transfers_count += 1; });
    
    // call_raw because dont want to rely on the canister returning the correct candid 
    let mut cycles_transfer_call_future = call_raw128(
        q.for_the_canister,
        "cycles_transfer",
        &cycles_transfer_candid,
        q.cycles,
    );
    
    if let Poll::Ready(call_result) = futures::poll!(&mut cycles_transfer_call_future) {
        // a trap will refund the already accepted cycles
        trap(&format!("cycles_transfer call_perform error: {:?}", call_result.unwrap_err()));
    }
    
    reply::<(Result<(), TransferCyclesError>,)>((Ok(()),)); // at the next commit point, the cts is replied-to without waiting for the cycles_transfer call to come back 
    
    let cycles_transfer_call_result: CallResult<Vec<u8>> = cycles_transfer_call_future.await;
    
    let cycles_transfer_refund: Cycles = msg_cycles_refunded128(); // now that we are for the sure in a callback

    let opt_cycles_transfer_call_error: Option<(u32, String)>;
    
    match cycles_transfer_call_result {
        Ok(_) => {
            opt_cycles_transfer_call_error = None;
        },
        Err(call_error) => {
            opt_cycles_transfer_call_error = Some((call_error.0 as u32, call_error.1));
        }
    }
    
    
    // we make a new call here because we already replied to the transfer_cycles-caller before the cycles_transfer call.
    do_transfer_cycles_callback(
        TransferCyclesCallbackQuest{
            user_cycles_transfer_id: q.user_cycles_transfer_id,
            opt_cycles_transfer_call_error
        },
        cycles_transfer_refund,
        1
    ).await;
    
}


async fn do_transfer_cycles_callback(transfer_cycles_callback_quest: TransferCyclesCallbackQuest, cycles_transfer_refund: CyclesTransferRefund, try_number: u32) {
    
    let transfer_cycles_callback_quest_cb_result = candid::utils::encode_one(&transfer_cycles_callback_quest); // before the move into the closure.
    
    let g = || {
        with_mut(&CTC_DATA, |ctc_data| { ctc_data.ongoing_cycles_transfers_count = ctc_data.ongoing_cycles_transfers_count.checked_sub(1).unwrap_or(0); });
    };
    
    let log_try = |transfer_cycles_callback_call_error: (RejectionCode, String), try_n: u32| {
        with_mut(&CTC_DATA, |ctc_data| {
            ctc_data.try_transfer_cycles_callbacks.push(
                TryTransferCyclesCallback { 
                    call_error_of_the_last_try: transfer_cycles_callback_call_error, 
                    transfer_cycles_callback_quest, 
                    cycles_transfer_refund,
                    try_number: try_n
                }
            );             
        });
    };
    
    let transfer_cycles_callback_quest_cb: Vec<u8> = match transfer_cycles_callback_quest_cb_result {
        Ok(b) => b,
        Err(candid_error) => {
            log_try((RejectionCode::Unknown, format!("candid code TransferCyclesCallbackQuest error: {:?}", candid_error)), try_number);
            return;
        }
    };

    let mut transfer_cycles_callback_call_future = call_raw128(
        cts_id(),
        "cycles_transferrer_transfer_cycles_callback",
        &transfer_cycles_callback_quest_cb,
        cycles_transfer_refund
    );
    
    if let Poll::Ready(call_result) = futures::poll!(&mut transfer_cycles_callback_call_future) {
        log_try(call_result.unwrap_err(), try_number);
        return;
    }
    
    match transfer_cycles_callback_call_future.await {
        Ok(_) => {
            g();
        },
        Err(transfer_cycles_callback_call_error) => {
            if msg_cycles_refunded128() != cycles_transfer_refund {
                g();
                return;
            }
            if try_number < 7 {
                log_try(transfer_cycles_callback_call_error, try_number + 1);
            } else {
                g();
            }
        }
    }
    
}


#[update(manual_reply = true)]
pub async fn cts_do_try_transfer_cycles_callbacks() {
    
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
   
    futures::future::join_all(
        with_mut(&CTC_DATA, |ctc_data| {
            ctc_data.try_transfer_cycles_callbacks.drain(..).map(
                |try_transfer_cycles_callback: TryTransferCyclesCallback| {
                    do_transfer_cycles_callback(
                        try_transfer_cycles_callback.transfer_cycles_callback_quest, 
                        try_transfer_cycles_callback.cycles_transfer_refund,
                        try_transfer_cycles_callback.try_number
                    )
                }
            ).collect::<Vec<_/*anonymous-future*/>>()
        })
    ).await;
    
    with(&CTC_DATA, |ctc_data| {
        reply::<(&Vec<TryTransferCyclesCallback>,)>((&(ctc_data.try_transfer_cycles_callbacks),));
    });
    
}



#[export_name = "canister_query cts_see_try_transfer_cycles_callbacks"]
pub fn cts_see_try_transfer_cycles_callbacks() {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    
    with(&CTC_DATA, |ctc_data| {
        reply::<(&Vec<TryTransferCyclesCallback>,)>((&(ctc_data.try_transfer_cycles_callbacks),));
    });
}


#[export_name = "canister_update cts_drain_try_transfer_cycles_callbacks"]
pub fn cts_drain_try_transfer_cycles_callbacks() {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    with_mut(&CTC_DATA, |ctc_data| {
        reply::<(Vec<TryTransferCyclesCallback>,)>((ctc_data.try_transfer_cycles_callbacks.drain(..).collect::<Vec<TryTransferCyclesCallback>>(),));
    });    
}


#[update]
pub fn cts_put_try_transfer_cycles_callback(try_transfer_cycles_callback: TryTransferCyclesCallback) {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    with_mut(&CTC_DATA, |ctc_data| {
        ctc_data.try_transfer_cycles_callbacks.push(try_transfer_cycles_callback);
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
    with_mut(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        *state_snapshot_ctc_data_candid_bytes = create_ctc_data_candid_bytes();
        state_snapshot_ctc_data_candid_bytes.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    let chunk_size: usize = 1024*1024;
    with(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_ctc_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    })

}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        *state_snapshot_ctc_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CTC_DATA_CANDID_BYTES, |state_snapshot_ctc_data_candid_bytes| {
        state_snapshot_ctc_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_ctc_data_out_of_the_state_snapshot() {
    if caller() != cts_id() {
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



