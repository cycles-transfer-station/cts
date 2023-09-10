

use futures::task::Poll;
use crate::*;





pub enum DoCyclesPayoutError {
    CandidError(CandidError),
    CMCallCallPerformError(u32),
    CMCallCallError((u32, String)),
    CMCallError(CMCallError)
}
impl From<CandidError> for DoCyclesPayoutError {
    fn from(ce: CandidError) -> DoCyclesPayoutError {
        DoCyclesPayoutError::CandidError(ce)  
    }
}

// use this enum in the stead of returning the CyclesPayoutData cause we want to make sure the cycles_payout_data is not re-place by this output cause the cycles_transferrer-transfer_cycles_callback can come back before this output is put back on the purchase/vcp. so we use this struct so that only the fields get re-place. 
pub enum DoCyclesPayoutSponse {
    CMCallerCyclesPayoutCallSuccessTimestampNanos(Option<u128>),
    ManagementCanisterPositCyclesCallSuccess(bool),
    NothingToDo
}

pub type DoCyclesPayoutResult = Result<DoCyclesPayoutSponse, DoCyclesPayoutError>;

pub async fn do_cycles_payout<T: CyclesPayoutDataTrait>(q: T) -> DoCyclesPayoutResult {
    
    // try cycles-payouts a couple of times before using the management canister's deposit cycles method.
    // cycles-bank can be in the stop mode for an upgrade.  
    
    if q.cycles_payout_data().cmcaller_cycles_payout_call_success_timestamp_nanos.is_none() {
        let cmcaller_cycles_payout_call_success_timestamp_nanos: Option<u128>;
        
        let mut call_future = call_raw128(
            with(&CM_DATA, |cm_data| { cm_data.cm_caller }),
            "cm_call",
            encode_one(
                CMCallQuest{
                    cm_call_id: q.cm_call_id(),
                    for_the_canister: q.cycles_payout_payee(),
                    method: q.cycles_payout_payee_method().to_string(),
                    put_bytes: q.cycles_payout_payee_method_quest_bytes()?,
                    cycles: q.cycles().saturating_sub(q.cycles_payout_fee()),
                    cm_callback_method: q.cm_call_callback_method().to_string(),
                }
            )?,
            q.cycles().saturating_sub(q.cycles_payout_fee()) + 500_000_000/*for the cm_caller*/
        );
                
        if let Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
            //cmcaller_cycles_payout_call_success_timestamp_nanos = None;
            return Err(DoCyclesPayoutError::CMCallCallPerformError(call_result_with_an_error.unwrap_err().0 as u32));
        } 
        match call_future.await {
            Ok(sponse_bytes) => match decode_one::<CMCallResult>(&sponse_bytes) {
                Ok(cm_call_result) => match cm_call_result {
                    Ok(()) => {
                        cmcaller_cycles_payout_call_success_timestamp_nanos = Some(time_nanos());
                    },
                    Err(cm_call_error) => {
                        //cmcaller_cycles_payout_call_success_timestamp_nanos = None;
                        return Err(DoCyclesPayoutError::CMCallError(cm_call_error))
                    }
                },
                Err(_candid_decode_error) => {
                    if msg_cycles_refunded128() >= q.cycles() {
                        cmcaller_cycles_payout_call_success_timestamp_nanos = None;
                    } else {
                        cmcaller_cycles_payout_call_success_timestamp_nanos = Some(time_nanos());
                    }    
                }
            },
            Err(cm_call_call_error) => {
                //cmcaller_cycles_payout_call_success_timestamp_nanos = None;
                return Err(DoCyclesPayoutError::CMCallCallError((cm_call_call_error.0 as u32, cm_call_call_error.1)));
            }
        }
        return Ok(DoCyclesPayoutSponse::CMCallerCyclesPayoutCallSuccessTimestampNanos(cmcaller_cycles_payout_call_success_timestamp_nanos));
    }
    
    if let Some((cycles_transfer_refund, _)) = q.cycles_payout_data().cmcaller_cycles_payout_callback_complete {
        if cycles_transfer_refund != 0 
        && q.cycles_payout_data().management_canister_posit_cycles_call_success == false {
            let management_canister_posit_cycles_call_success: bool;
            match call_with_payment128::<(management_canister::CanisterIdRecord,),()>(
                MANAGEMENT_CANISTER_ID,
                "deposit_cycles",
                (management_canister::CanisterIdRecord{
                    canister_id: q.cycles_payout_payee()
                },),
                cycles_transfer_refund
            ).await {
                Ok(_) => {
                    management_canister_posit_cycles_call_success = true;
                },
                Err(_) => {
                    management_canister_posit_cycles_call_success = false;
                }
            }
            return Ok(DoCyclesPayoutSponse::ManagementCanisterPositCyclesCallSuccess(management_canister_posit_cycles_call_success));
        }
    }    
    
    return Ok(DoCyclesPayoutSponse::NothingToDo);
}


