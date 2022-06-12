use cts_lib::{
    ic_cdk::{
        self,
        api::{
            call::{
                call
            },
        },
        export::{
            candid,
            Principal
        },
    },
    types::{
        cts::{},
        user_canister::{
            CyclesTransferPurchaseLogId
        },
        cycles_transferrer::{
            CyclesTransferrerInit,
            CTSUserTransferCyclesQuest,
            CTSUserTransferCyclesError
        }
    },
    
};



struct OngoingUserTransferCycles {
    
}





pub const MAX_ONGOING_CYCLES_TRANSFERS: usize = 1000;





thread_local! {
    static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static ONGOING_CYCLES_TRANSFERS: Cell<usize> = Cell::new(0);
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
    
    if ONGOING_CYCLES_TRANSFERS.with(|octs| octs.get()) >= MAX_ONGOING_CYCLES_TRANSFERS {
        reply::<(Result<(), CTSUserTransferCyclesError>,)>((Err(CTSUserTransferCyclesError::MaxOngoingCyclesTransfers(MAX_ONGOING_CYCLES_TRANSFERS)),));
        return;
    }
    
    let cycles_transfer_candid: Vec<u8> = match candid::utils::encode_one(
        CyclesTransfer{ memo: cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles_transfer_memo }
    ) {
        Ok(candid_bytes) => candid_bytes,    
        Err(candid_error) => {
            reply::<(Result<(), CTSUserTransferCyclesError>,)>((Err(CTSUserTransferCyclesError::CyclesTransferQuestCandidCodeError(format!("{:?}", candid_error))),));    
            return;
            
        },
    };
    
    let cycles: Cycles = msg_cycles_accept128(msg_cycles_available128()); // make sure to cept the cts cycles for the call after any possibility of a reply() and return; also make sure errors after here before the cycles_transfer_call_future.await are trap so that the state rolls back and the cts gets the cycles back 
    if cycles != cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles {
        trap("check the cts call of this cycles_transferrer.")
    }
    
    ONGOING_CYCLES_TRANSFERS.with(|octs| { octs.set(octs.get() + 1); }); // checked add?
    
    reply::<(Result<(), CTSUserTransferCyclesError>,)>((Ok(),)); /// test that at the next commit point, the cts is replied-to without waiting for the cycles_transfer call to come back 
    
    // call_raw because dont want to rely on the canister returning the correct candid 
    let cycles_transfer_call_future: CallFuture<Vec<u8>> = call_raw128(
        cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.canister_id,
        "cycles_transfer",
        cycles_transfer_candid,
        cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles,
    );
    
    if cycles_transfer_call_future.call_perform_status_code as u32 != 0 {
        // test that a trap will refund the already accepted cycles(from the cts-main) and discard the reply(to the cts-main) 
        trap(&format!("cycles_transfer call_perform error: {:?}", cycles_transfer_call_future.call_perform_status_code));
        //cycles_transfer_refund = cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles;
        //cycles_transfer_call_error = Some(cycles_transfer_call_future.call_perform_status_code, "call_perform error");
    }
    
    let cycles_transfer_call_result: CallResult<Vec<u8>> = cycles_transfer_call_future.await;
    
    let cycles_transfer_refund: Cycles = msg_cycles_refunded128(); // now that we are for sure in a callback

    let mut cycles_transfer_call_error: Option<(u32, String)>;
    
    match cycles_transfer_call_result {
        Ok(_) => {
            cycles_transfer_call_error = None;
        },
        Err(call_error) => {
            cycles_transfer_call_error = Some(call_error);
        }
    }
   
    
    
    // we make a new call here because we already replied to the cts before the cycles_transfer call.
    match call_with_payment128::<(CyclesTransferrerUserTransferCyclesCallback,), (Result<(), CyclesTransferrerUserTransferCyclesCallbackError>,)>(
        cts_id(),
        "cycles_transferrer_user_transfer_cycles_callback",
        (CyclesTransferrerUserTransferCyclesCallback{
            cts_user_transfer_cycles_quest: cts_q
        },),
        cycles_transfer_refund
    ).await {
        Ok((cycles_transferrer_user_transfer_cycles_callback_sponse,)) => match cycles_transferrer_user_transfer_cycles_callback_sponse {
            Ok(()) => { 
                ONGOING_CYCLES_TRANSFERS.with(|octs| { octs.set(octs.get() - 1); }); 
            }, // checked sub?
            Err(cycles_transferrer_user_transfer_cycles_callback_error) => {
            
            }
        },
        Err(cycles_transferrer_user_transfer_cycles_callback_call_error) => {
            // log and re-try in a heartbeat or similar
        }
    }
    
    
    
    
}





