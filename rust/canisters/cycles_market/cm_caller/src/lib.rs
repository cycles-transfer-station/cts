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
        CyclesTransferMemo,
        cts::{

        },
        cycles_transferrer::{
            CyclesTransferrerCanisterInit,
            CyclesTransfer,
            TransferCyclesQuest,
            TransferCyclesError,
            TransferCyclesCallbackQuest
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


type CyclesTransferRefund = Cycles;


#[derive(CandidType, Deserialize)]
pub struct CMCallerInit {
    pub cycles_market_id: Principal,
    pub cts_id: Principal
}

#[derive(CandidType, Deserialize)]    
pub struct CMCallQuest{
    pub cm_call_id: u128,
    pub for_the_canister: Principal,
    pub method: String,
    #[serde(with = "serde_bytes")]
    pub put_bytes: Vec<u8>,
    pub cycles: Cycles,
    pub cm_callback_method: String,
}


#[derive(CandidType, Deserialize)]
pub enum CMCallError {
    MaxCalls,
}



#[derive(CandidType, Deserialize, Clone)]
pub struct CMCallbackQuest {
    pub cm_call_id: u128,
    pub opt_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)> // None means callstatus == 'replied'
    // sponse_bytes? do i care? 
}




#[derive(CandidType, Deserialize)]
pub struct TryCallback { 
    cm_callback_method: String,
    cm_callback_quest: CMCallbackQuest, 
    cycles_transfer_refund: CyclesTransferRefund,
    try_number: u32,
    call_error_of_the_last_try: (RejectionCode, String)/*the call-error of the last try*/
}


#[derive(CandidType, Deserialize)]
struct CMCallerData {
    cycles_market_id: Principal,
    cts_id: Principal,
    ongoing_calls_count: u64,
    try_callbacks: Vec<TryCallback>
}
impl CMCallerData {
    fn new() -> Self {
        Self {
            cycles_market_id: Principal::from_slice(&[]),
            cts_id: Principal::from_slice(&[]),
            ongoing_calls_count: 0,
            try_callbacks: Vec::new()
        }
    }
}



pub const MAX_ONGOING_CALLS: u64 = 5000;

const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;



thread_local! {

    static CMCALLER_DATA: RefCell<CMCallerData> = RefCell::new(CMCallerData::new());     
    
    // not save in a CMCALLER_DATA
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_CMCALLER_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}



#[init]
fn init(cmcaller_init: CMCallerInit) {
    with_mut(&CMCALLER_DATA, |cmcaller_data| {
        cmcaller_data.cycles_market_id = cmcaller_init.cycles_market_id;
        cmcaller_data.cts_id = cmcaller_init.cts_id;
    });
}


fn create_cmcaller_data_candid_bytes() -> Vec<u8> {
    let mut cmcaller_data_candid_bytes: Vec<u8> = with(&CMCALLER_DATA, |cmcaller_data| { encode_one(cmcaller_data).unwrap() });
    cmcaller_data_candid_bytes.shrink_to_fit();
    cmcaller_data_candid_bytes
}


fn re_store_cmcaller_data_candid_bytes(cmcaller_data_candid_bytes: Vec<u8>) {
    let cmcaller_data: CMCallerData = match decode_one::<CMCallerData>(&cmcaller_data_candid_bytes) {
        Ok(cmcaller_data) => cmcaller_data,
        Err(_) => {
            trap("error decode of the cmcaller_data");
            /*
            let old_cmcaller_data: OldCMCallerData = decode_one::<OldCMCallerData>(&cmcaller_data_candid_bytes).unwrap();
            let cmcaller_data: CMCallerData = CMCallerData{
                cts_id: old_cmcaller_data.cts_id,
                ......
            };
            cmcaller_data
            */
        }
    };
    
    std::mem::drop(cmcaller_data_candid_bytes);
    
    with_mut(&CMCALLER_DATA, |cmcallerd| {
        *cmcallerd = cmcaller_data;
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    let cmcaller_upgrade_data_candid_bytes: Vec<u8> = create_cmcaller_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*u64 len of the upgrade_data_candid_bytes*/ + cmcaller_upgrade_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
    }
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((cmcaller_upgrade_data_candid_bytes.len() as u64).to_be_bytes()));
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &cmcaller_upgrade_data_candid_bytes);
}

#[post_upgrade]
fn post_upgrade() {
    let mut cmcaller_upgrade_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut cmcaller_upgrade_data_candid_bytes_len_u64_be_bytes);
    let cmcaller_upgrade_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(cmcaller_upgrade_data_candid_bytes_len_u64_be_bytes); 
    
    let mut cmcaller_upgrade_data_candid_bytes: Vec<u8> = vec![0; cmcaller_upgrade_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut cmcaller_upgrade_data_candid_bytes);
    
    re_store_cmcaller_data_candid_bytes(cmcaller_upgrade_data_candid_bytes);
}




// --------------------------------------------------

fn cts_id() -> Principal {
    with(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.cts_id })
}

// ---------------------------------------------------


// (q: TransferCyclesQuest) -> Result<(), TransferCyclesError> 
#[update(manual_reply = true)]
pub async fn cm_call() {
    if caller() != with(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.cycles_market_id }) {
        trap("this method must be call by the CYCLES-MARKET");
    }
    
    if get(&STOP_CALLS) == true {
        trap("Maintenance. try later.")
    }
    
    
    if with(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.ongoing_calls_count }) >= MAX_ONGOING_CALLS {
        reply::<(Result<(), CMCallError>,)>((Err(CMCallError::MaxCalls),));    
        return;
    }
    
    let (q,): (CMCallQuest,) = arg_data::<(CMCallQuest,)>();
    
    if msg_cycles_available128() < q.cycles {
        trap("call cycles must be >= the q.cycles");
    } 
    
    msg_cycles_accept128(msg_cycles_available128()); // already checked that there is correct mount of the cycles        // before calling reply or reject.
    
    with_mut(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.ongoing_calls_count += 1; });
    
    let mut call_future = call_raw128(
        q.for_the_canister,
        &q.method,
        &q.put_bytes,
        q.cycles,
    );
    
    if let Poll::Ready(call_result) = futures::poll!(&mut call_future) {
        trap(&format!("call_perform error: {:?}", call_result.unwrap_err()));
    }
    
    reply::<(Result<(), CMCallError>,)>((Ok(()),)); 
    
    let call_result: CallResult<Vec<u8>> = call_future.await;
    
    let cycles_transfer_refund: Cycles = msg_cycles_refunded128(); // now that we are for the sure in a callback

    let opt_call_error: Option<(u32, String)>;
    
    match call_result {
        Ok(_) => {
            opt_call_error = None;
        },
        Err(call_error) => {
            opt_call_error = Some((call_error.0 as u32, call_error.1));
        }
    }
    
    
    do_callback(
        q.cm_callback_method,
        CMCallbackQuest{
            cm_call_id: q.cm_call_id,
            opt_call_error
        },
        cycles_transfer_refund,
        1
    ).await;
    
}


async fn do_callback(cm_callback_method: String, cm_callback_quest: CMCallbackQuest, cycles_transfer_refund: CyclesTransferRefund, try_number: u32) {
    
    let cm_callback_quest_cb_result = candid::utils::encode_one(&cm_callback_quest); // before the move into the closure.
    
    let g = || {
        with_mut(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.ongoing_calls_count = cmcaller_data.ongoing_calls_count.saturating_sub(1); });
    };
    
    let log_try = |cm_callback_call_error: (RejectionCode, String), try_n: u32| {
        with_mut(&CMCALLER_DATA, |cmcaller_data| {
            cmcaller_data.try_callbacks.push(
                TryCallback { 
                    cm_callback_method: cm_callback_method.clone(),
                    cm_callback_quest, 
                    cycles_transfer_refund,
                    try_number: try_n,
                    call_error_of_the_last_try: cm_callback_call_error,
                }
            );             
        });
    };
    
    let cm_callback_quest_cb: Vec<u8> = match cm_callback_quest_cb_result {
        Ok(b) => b,
        Err(candid_error) => {
            log_try((RejectionCode::Unknown, format!("candid code CMCallbackQuest error: {:?}", candid_error)), try_number);
            return;
        }
    };

    let mut cm_callback_call_future = call_raw128(
        with(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.cycles_market_id }),
        &cm_callback_method,
        &cm_callback_quest_cb,
        cycles_transfer_refund
    );

    if let Poll::Ready(call_result) = futures::poll!(&mut cm_callback_call_future) {
        log_try(call_result.unwrap_err(), try_number + 1);
        return;
    }
    
    match cm_callback_call_future.await {
        Ok(_) => {
            g();
        },
        Err(cm_callback_call_error) => {
            if msg_cycles_refunded128() != cycles_transfer_refund {
                g();
                return;
            }
            log_try(cm_callback_call_error, try_number + 1);
        }
    }
       
}


// ---------------------------------------------------


#[update(manual_reply = true)]
pub async fn cts_do_try_callbacks() {
    
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
   
    futures::future::join_all(
        with_mut(&CMCALLER_DATA, |cmcaller_data| {
            cmcaller_data.try_callbacks.drain(..).map(
                |try_callback: TryCallback| {
                    do_callback(
                        try_callback.cm_callback_method,
                        try_callback.cm_callback_quest, 
                        try_callback.cycles_transfer_refund,
                        try_callback.try_number
                    )
                }
            ).collect::<Vec<_/*anonymous-future*/>>()
        })
    ).await;
    
    with(&CMCALLER_DATA, |cmcaller_data| {
        reply::<(&Vec<TryCallback>,)>((&(cmcaller_data.try_callbacks),));
    });
    
}




#[export_name = "canister_query cts_see_try_callbacks"]
pub fn cts_see_try_callbacks() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    
    with(&CMCALLER_DATA, |cmcaller_data| {
        reply::<(&Vec<TryCallback>,)>((&(cmcaller_data.try_callbacks),));
    });
}


#[export_name = "canister_update cts_drain_try_callbacks"]
pub fn cts_drain_try_callbacks() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&CMCALLER_DATA, |cmcaller_data| {
        reply::<(Vec<TryCallback>,)>((cmcaller_data.try_callbacks.drain(..).collect::<Vec<TryCallback>>(),));
    });    
}


#[update]
pub fn cts_put_try_callback(try_callback: TryCallback) {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&CMCALLER_DATA, |cmcaller_data| {
        cmcaller_data.try_callbacks.push(try_callback);
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
    with_mut(&STATE_SNAPSHOT_CMCALLER_DATA_CANDID_BYTES, |state_snapshot_cmcaller_data_candid_bytes| {
        *state_snapshot_cmcaller_data_candid_bytes = create_cmcaller_data_candid_bytes();
        state_snapshot_cmcaller_data_candid_bytes.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    let chunk_size: usize = 1024*1024;
    with(&STATE_SNAPSHOT_CMCALLER_DATA_CANDID_BYTES, |state_snapshot_cmcaller_data_candid_bytes| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_cmcaller_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    })

}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CMCALLER_DATA_CANDID_BYTES, |state_snapshot_cmcaller_data_candid_bytes| {
        *state_snapshot_cmcaller_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    with_mut(&STATE_SNAPSHOT_CMCALLER_DATA_CANDID_BYTES, |state_snapshot_cmcaller_data_candid_bytes| {
        state_snapshot_cmcaller_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_ctc_data_out_of_the_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the cts for this method.")
    }
    re_store_cmcaller_data_candid_bytes(
        with_mut(&STATE_SNAPSHOT_CMCALLER_DATA_CANDID_BYTES, |state_snapshot_cmcaller_data_candid_bytes| {
            let mut v: Vec<u8> = Vec::new();
            v.append(state_snapshot_cmcaller_data_candid_bytes);  // moves the bytes out of the state_snapshot vec
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



