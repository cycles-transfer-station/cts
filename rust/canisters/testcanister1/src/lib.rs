//Test the reply::<>() before a call.await 


use std::cell::Cell;
use cts_lib::{
    ic_cdk::{
        self,
        export::Principal,
        api::{
            trap,
            id,
            canister_balance128,
            call::{
                call_with_payment128,
                msg_cycles_accept128,
                msg_cycles_available128,
                msg_cycles_refunded128,
                reject,
                CallFuture,
                RejectionCode,
                call_raw128
            }
            
        }
    },
    ic_cdk_macros::{
        update,
        query
    },
    types::{
        Cycles,
        CyclesTransfer,
        CyclesTransferMemo,
        
    }
};



thread_local! {
    /*static CYCLES_TRANSFER_CYCLES: Cell<Cycles> = Cell::new(0);*/
    

}


/*
#[update]
pub async fn cycles_transfer(ct: CyclesTransfer) -> () {
    CYCLES_TRANSFER_CYCLES.with(|ctc| { ctc.set(ctc.get() + msg_cycles_accept128(msg_cycles_available128())); });
    
}
*/

/*
#[update]
pub fn see_cycles_transfer_cycles() -> Cycles {
    CYCLES_TRANSFER_CYCLES.with(|ctc| { ctc.get() })
}
*/


#[update]
pub async fn test_cycles_transfer_pogation_ingress_start(test_canister_id: Principal, cycles: Cycles) -> (u32, String, Cycles) {
    // send cycles to testcanister2
    
    match call_raw128(
        test_canister_id,
        "test_cycles_transfer_pogation_two",
        &candid::utils::encode_one(id()).unwrap(),
        cycles
    ).await {
        Ok(_) => trap(""),
        Err(call_error) => return (call_error.0 as u32, call_error.1, msg_cycles_refunded128())
    }

}






#[update(manual_reply = true)]
pub fn test_manual_reply() {
    reject("reject-message-here");
}






