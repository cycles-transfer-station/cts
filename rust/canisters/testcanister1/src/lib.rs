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


#[update]
pub fn see_cycles_transfer_cycles() -> Cycles {
    CYCLES_TRANSFER_CYCLES.with(|ctc| { ctc.get() })
}
*/


#[update]
pub async fn test_cycles_transfer_pogation_ingress_start(test_canister_id: Principal, cycles: Cycles) -> String {
    // send cycles to testcanister2
    
    let mut s: String = String::new();
    
    s.push_str(&format!("first_canister_balance: {:?}", canister_balance128()));
    
    let cycles_transfer_call_future: CallFuture<Vec<u8>> = call_raw128(
        test_canister_id,
        "test_cycles_transfer_pogation_two",
        &candid::utils::encode_one(id()).unwrap(),
        cycles
    );
    
    s.push_str(&format!("\nsecond_canister_balance: {:?}\ncall_perform_status_code: {:?}", canister_balance128(), cycles_transfer_call_future.call_perform_status_code));
    
    /*
    match cycles_transfer_call_future.await {
        Ok(()) => return,
        Err(call_error) => {
            reject(&format!("two call error: {:?}", call_error));
            return;
        }
    }
    */
    
    
    // have testcanister2 pogate the cycles to a testcanister1 method and refund    
    
    s
}



#[update]
pub async fn test_cycles_transfer_pogation_three() -> u64 {
    5u64
}


