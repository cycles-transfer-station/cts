use std::cell::Cell;
use cts_lib::{
    ic_cdk::{
        self,
        export::Principal,
        api::{
            trap,
            canister_balance128,
            call::{
                call,
                call_with_payment128,
                msg_cycles_accept128,
                msg_cycles_available128,
                reject,
                reply,
                
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





#[update]
pub async fn test_cycles_transfer_pogation_two() -> () {
    // have testcanister2 pogate the cycles to a testcanister1 method and refund    

    let cycles_available: Cycles = msg_cycles_available128();
     
    let mut s: String = String::new();
    
    s.push_str(&format!("first balance: {:?}", canister_balance128()));
     
    let cycles_cept: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    s.push_str(&format!("\ncycles_cept: {:?}\nsecond_balance: {:?}", cycles_cept, canister_balance128()));
    
    reply::<(Result<(), ()>,)>((Ok(()),));
    
    trap(&s)
    

}

