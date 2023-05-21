use cts_lib::{
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                call_raw128,
                CallResult,
                arg_data,
                reply,
                RejectionCode,
                msg_cycles_available128,
                msg_cycles_accept128,
                msg_cycles_refunded128    
            },
        },
        export::{
            Principal,
            candid::{
                self, 
                CandidType, 
                Deserialize,
            },
        },
        init,
        pre_upgrade,
        post_upgrade,
        update,
        query
    },
    types::{
        Cycles,
        cm_caller::*
    },
    tools::{
        localkey::{
            refcell::{
                with, 
                with_mut,
            },
            cell::{
                get,
                set
            }
        },
        caller_is_controller_gaurd,
    },
    stable_memory_tools::{self, MemoryId},
};
use std::cell::{Cell, RefCell};
use futures::task::Poll;



type CyclesTransferRefund = Cycles;




#[derive(CandidType, Deserialize)]
pub struct TryCallback { 
    cm_callback_method: String,
    cm_callback_quest: CMCallbackQuest, 
    cycles_transfer_refund: CyclesTransferRefund,
    try_number: u32,
    call_error_of_the_last_try: (RejectionCode, String)
}




#[derive(CandidType, Deserialize)]
struct OldCMCallerData {}


#[derive(CandidType, Deserialize)]
struct CMCallerData {
    cycles_market_token_trade_contract: Principal,
    ongoing_calls_count: u64,
    try_callbacks: Vec<TryCallback>
}
impl CMCallerData {
    fn new() -> Self {
        Self {
            cycles_market_token_trade_contract: Principal::from_slice(&[]),
            ongoing_calls_count: 0,
            try_callbacks: Vec::new()
        }
    }
}


pub const MAX_ONGOING_CALLS: u64 = 5000;

const HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID: MemoryId = MemoryId::new(0);


thread_local! {

    static CMCALLER_DATA: RefCell<CMCallerData> = RefCell::new(CMCallerData::new());     
    
    // not save in a CMCALLER_DATA
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
}



#[init]
fn init(cmcaller_init: CMCallerInit) {
    stable_memory_tools::init(&CMCALLER_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID);

    with_mut(&CMCALLER_DATA, |cmcaller_data| {
        cmcaller_data.cycles_market_token_trade_contract = cmcaller_init.cycles_market_token_trade_contract;
    });
}



#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::post_upgrade(&CMCALLER_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID, None::<fn(OldCMCallerData) -> CMCallerData>);
}




// --------------------------------------------------


// ---------------------------------------------------


// (q: TransferCyclesQuest) -> Result<(), TransferCyclesError> 
#[update(manual_reply = true)]
pub async fn cm_call() {
    let cycles_market_token_trade_contract: Principal = with(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.cycles_market_token_trade_contract });
    if caller() != cycles_market_token_trade_contract {
        trap(&format!("this method must be call by the CYCLES-MARKET token trade contract: {}", cycles_market_token_trade_contract));
    }
    
    if get(&STOP_CALLS) == true {
        trap("Maintenance. try later.")
    }
    
    
    if with(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.ongoing_calls_count }) >= MAX_ONGOING_CALLS {
        reply::<(CMCallResult,)>((Err(CMCallError::MaxCalls),));    
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
    
    reply::<(CMCallResult,)>((Ok(()),)); 
    
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
        with(&CMCALLER_DATA, |cmcaller_data| { cmcaller_data.cycles_market_token_trade_contract }),
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
pub async fn controller_do_try_callbacks() {    
    caller_is_controller_gaurd(&caller());  
    
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
pub fn controller_see_try_callbacks() {
    caller_is_controller_gaurd(&caller());
    
    with(&CMCALLER_DATA, |cmcaller_data| {
        reply::<(&Vec<TryCallback>,)>((&(cmcaller_data.try_callbacks),));
    });
}


#[export_name = "canister_update cts_drain_try_callbacks"]
pub fn controller_drain_try_callbacks() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CMCALLER_DATA, |cmcaller_data| {
        reply::<(Vec<TryCallback>,)>((cmcaller_data.try_callbacks.drain(..).collect::<Vec<TryCallback>>(),));
    });    
}


#[update]
pub fn controller_put_try_callback(try_callback: TryCallback) {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&CMCALLER_DATA, |cmcaller_data| {
        cmcaller_data.try_callbacks.push(try_callback);
    });    
}




// -----------------------------------------------------------------------------------



#[update]
pub fn controller_set_stop_calls_flag(stop_calls_flag: bool) {
    caller_is_controller_gaurd(&caller());
    set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn controller_see_stop_calls_flag() -> bool {
    caller_is_controller_gaurd(&caller());
    get(&STOP_CALLS)
}


// -------------------------------------------------------------------------


#[derive(CandidType, Deserialize)]
pub struct ControllerCallCanisterQuest {
    callee: Principal,
    method_name: String,
    arg_raw: Vec<u8>,
    cycles: Cycles
}

#[update(manual_reply = true)]
pub async fn controller_call_canister() {
    caller_is_controller_gaurd(&caller());
    
    let (q,): (ControllerCallCanisterQuest,) = arg_data::<(ControllerCallCanisterQuest,)>(); 
    
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



