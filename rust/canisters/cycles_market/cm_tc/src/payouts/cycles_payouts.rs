// do not borrow or borrow_mut the CM_DATA.

use futures::task::Poll;
use crate::*;

use cts_lib::{
    ic_cdk::api::call::CallResult,
    types::cts::CMTCUserPayoutCyclesQuest,
};


pub async fn do_cycles_payout<T: CyclesPayoutTrait>(q: T) -> CyclesPayoutData {
    
    // try cycles-payouts a couple of times before using the management canister's deposit cycles method.
    // cycles-bank can be in the stop mode for an upgrade.  
    
    let mut cycles_payout_data: CyclesPayoutData = q.cycles_payout_data(); 
    
    if cycles_payout_data.cycles_payout.is_none() {
        
        if q.cycles() >= CYCLES_DUST_COLLECTION_THRESHOLD {
            
            let (canister, method, quest_bytes) = {
                if q.cycles_payout_payee().as_slice().len() == 29 { // call the cts, this is a user without a cb.
                    (
                        localkey::cell::get(&CTS_ID), 
                        "cm_tc_user_payout_cycles", 
                        match candid::encode_one(CMTCUserPayoutCyclesQuest{user_id: q.cycles_payout_payee()}) {
                            Ok(b) => b,
                            Err(_) => return cycles_payout_data,
                        }
                    )                    
                } else { // this is a cb
                    (
                        q.cycles_payout_payee(),
                        q.cycles_payout_payee_method(),
                        match q.cycles_payout_payee_method_quest_bytes() {
                            Ok(b) => b,
                            Err(_e) => return cycles_payout_data,
                        }
                    )    
                }
            };
            
            let mut call_future = call_raw128(
                canister,
                method,
                quest_bytes,
                q.cycles().saturating_sub(q.cycles_payout_fee())
            );
                    
            if let Poll::Ready(_call_result_with_an_error) = futures::poll!(&mut call_future) {
                //return Err(DoCyclesPayoutError::CyclesPayoutCallPerformError(call_result_with_an_error.unwrap_err().0 as u32));
                return cycles_payout_data;
            } 
            
            let call_result: CallResult<Vec<u8>> = call_future.await;
            
            match call_result {
                Ok(_sponse_bytes) => {
                    cycles_payout_data.cycles_payout = Some(true);
                },
                Err(_call_error) => {
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
                    //return Err(DoCyclesPayoutError::CyclesPayoutCallError(call_error_as_u32_and_string(call_error)));
                }
            } 
        } else {
            cycles_payout_data.cycles_payout = Some(false);
        }
    }
    
    return cycles_payout_data;
}


