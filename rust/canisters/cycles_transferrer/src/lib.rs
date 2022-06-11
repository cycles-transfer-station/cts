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
    
    let cycles: Cycles = msg_cycles_accept128(msg_cycles_available128());
    if cycles != cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles {
        trap("check the cts call of this cycles_transferrer.")
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
    
    ONGOING_CYCLES_TRANSFERS.with(|octs| { octs.set(octs.get() + 1); });
    
    reply::<(Result<(), CTSUserTransferCyclesError>,)>((Ok(),)); /// test that at the next commit point, the cts is replied-to without waiting for the cycles_transfer call to come back 
    
    // call_raw because dont want to rely on the canister returning the correct candid 
    let cycles_transfer_call: CallResult<Vec<u8>> = call_raw128::<(CyclesTransfer,), ()>(
        cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.canister_id,
        "cycles_transfer",
        cycles_transfer_candid,
        cts_q.umc_user_transfer_cycles_quest.uc_user_transfer_cycles_quest.user_transfer_cycles_quest.cycles,
    ).await;
    
    let cycles_transfer_refund: Cycles = msg_cycles_refunded128();
    
    let mut cycles_transfer_call_error: Option<(u32, String)> = None;
    
    match cycles_transfer_call {
        Ok(_) => {},
        Err(cycles_transfer_call_error) => {
            cycles_transfer_call_error = Some(cycles_transfer_call_error);
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
            Ok(()) => (),
            Err(cycles_transferrer_user_transfer_cycles_callback_error) => {
            
            }
        },
        Err(cycles_transferrer_user_transfer_cycles_callback_call_error) => {
            // log and re-try in a heartbeat or similar
        }
    }
    
    
    
    
}





