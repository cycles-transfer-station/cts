use std::cell::Cell;
use cts_lib::{
    ic_cdk::{
        self,
        export::Principal,
        api::{
            trap,
            call::{
                call,
                call_with_payment128,
                msg_cycles_accept128,
                msg_cycles_available128,
                reject,
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
    static CYCLES_TRANSFER_CYCLES: Cell<Cycles> = Cell::new(0);
}



#[update]
pub async fn cycles_transfer(ct: CyclesTransfer) -> () {
    CYCLES_TRANSFER_CYCLES.with(|ctc| { ctc.set(ctc.get() + msg_cycles_accept128(msg_cycles_available128())); });
    
}

#[update]
pub fn see_cycles_transfer_cycles() -> Cycles {
    CYCLES_TRANSFER_CYCLES.with(|ctc| { ctc.get() })
}



#[update]
pub async fn test_cycles_transfer_pogation_two(test_canister_id: Principal) -> () {
    // have testcanister2 pogate the cycles to a testcanister1 method and refund    

    let cycles_available: Cycles = msg_cycles_available128();

    match call::<(), (u64,)>(
        test_canister_id,
        "test_cycles_transfer_pogation_three",
        (),
    ).await {
        Ok((v,)) => {
            if v != 5u64 { trap("wrong three sponse") }
            let cycles_accepted: Cycles = msg_cycles_accept128(msg_cycles_available128());
            if cycles_accepted != cycles_available { trap("check cycles count") }
            CYCLES_TRANSFER_CYCLES.with(|ctc| { ctc.set(ctc.get() + cycles_accepted); });
            return;
        },
        Err(call_error) => {
            reject(&format!("three call-error. call-error: {:?}", call_error));
            return;
        }
    }
    
    

}

