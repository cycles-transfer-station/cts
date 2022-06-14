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
                msg_cycles_refunded128,
                
            },
        },
        export::{
            Principal,
            candid::{self, CandidType, Deserialize},
        },
    },
    ic_cdk_macros::{
        init,
        update
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
        localkey_refcell::{with, with_mut}
    }
};
use std::cell::{Cell, RefCell};


pub type CyclesTransferRefund = Cycles;








pub const MAX_ONGOING_CYCLES_TRANSFERS: usize = 1000;



thread_local! {
    static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static ONGOING_CYCLES_TRANSFERS_COUNT: Cell<usize> = Cell::new(0);
    static RE_TRY_CYCLES_TRANSFERRER_USER_TRANSFER_CYCLES_CALLBACKS: RefCell<Vec<((u32, String), CyclesTransferrerUserTransferCyclesCallbackQuest, CyclesTransferRefund)>> = RefCell::new(Vec::new()); // (cycles_transferrer_user_transfer_cycles_call_error, CyclesTransferrerUserTransferCyclesCallback, CyclesTransferRefund)
}



#[init]
fn init(cycles_transferrer_init: CyclesTransferrerInit) {
    CTS_ID.with(|cts_id| { cts_id.set(cycles_transferrer_init.cts_id); });
}



// --------------------------------------------------

fn cts_id() -> Principal {
    CTS_ID.with(|cts_id| { cts_id.get() })
}



// ---------------------------------------------------


// (cts_q: CTSUserTransferCyclesQuest) -> Result<(), CTSUserTransferCyclesError>
#[export_name = "canister_update cts_user_transfer_cycles"]
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
        // test that a trap will refund the already accepted cycles(from the cts-main) and discard the reply(to the cts-main) 
        trap(&format!("cycles_transfer call_perform error: {:?}", RejectionCode::from(cycles_transfer_call_future.call_perform_status_code)));
        //cycles_transfer_refund = cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles;
        //cycles_transfer_call_error = Some(cycles_transfer_call_future.call_perform_status_code, "call_perform error");
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






