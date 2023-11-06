// do not borrow or borrow_mut the CM_DATA.

use futures::task::Poll;
use crate::*;

use cts_lib::{
    ic_cdk::api::call::CallResult
};




pub enum DoCyclesPayoutError {
    CandidError(CandidError),
    CyclesPayoutCallPerformError(u32),
    CyclesPayoutCallError(CallError),
    //ManagementCanisterPositCyclesCallError(CallError),
}
impl From<CandidError> for DoCyclesPayoutError {
    fn from(ce: CandidError) -> DoCyclesPayoutError {
        DoCyclesPayoutError::CandidError(ce)  
    }
}

pub enum DoCyclesPayoutSponse {
    CyclesPayoutSuccess,
    NothingToDo
}

pub type DoCyclesPayoutResult = Result<DoCyclesPayoutSponse, DoCyclesPayoutError>;

pub async fn do_cycles_payout<T: CyclesPayoutTrait>(q: T) -> DoCyclesPayoutResult {
    
    // try cycles-payouts a couple of times before using the management canister's deposit cycles method.
    // cycles-bank can be in the stop mode for an upgrade.  
    
    if q.cycles_payout_data().cycles_payout == false {
        
        let mut call_future = call_raw128(
            q.cycles_payout_payee(),
            q.cycles_payout_payee_method(),
            q.cycles_payout_payee_method_quest_bytes()?,
            q.cycles().saturating_sub(q.cycles_payout_fee())
        );
                
        if let Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
            return Err(DoCyclesPayoutError::CyclesPayoutCallPerformError(call_result_with_an_error.unwrap_err().0 as u32));
        } 
        let call_result: CallResult<Vec<u8>> = call_future.await;
        
        let _cycles_refund: Cycles = msg_cycles_refunded128();
        
        match call_result {
            Ok(_sponse_bytes) => {
                return Ok(DoCyclesPayoutSponse::CyclesPayoutSuccess);
            },
            Err(call_error) => {
                // what error does it give when the canister is stopped, and what error does it give when the canister is empty?
                // if the canister is empty, call the management canister posit cycles
                /*
                if canister-module-is-empty  {
                    match call_with_payment128::<(management_canister::CanisterIdRecord,),()>(
                        MANAGEMENT_CANISTER_ID,
                        "deposit_cycles",
                        (management_canister::CanisterIdRecord{
                            canister_id: q.cycles_payout_payee()
                        },),
                        q.cycles().saturating_sub(q.cycles_payout_fee())
                    ).await {
                        Ok(_) => {
                            return Ok(DoCyclesPayoutSponse::CyclesPayoutSuccess);
                        },
                        Err(call_error) => {
                            return Err(ManagementCanisterPositCyclesCallError(call_error_as_u32_and_string(call_error)));
                        }
                    }
                }
                */
                return Err(DoCyclesPayoutError::CyclesPayoutCallError(call_error_as_u32_and_string(call_error)));
            }
        }
    }
    
    return Ok(DoCyclesPayoutSponse::NothingToDo);
}


