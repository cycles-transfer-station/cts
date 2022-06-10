use cts_lib::{
    ic_cdk::{
        self,
        api::{
            call::{
                call
            },
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



thread_local! {
    static CTS_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static ONGOING_CYCLES_TRANSFERS: RefCell<HashMap<(UserId, CyclesTransferPurchaseLogId), OngoingUserTransferCycles>> = RefCell::new();
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



#[update]
pub async fn cts_user_transfer_cycles(cts_q: CTSUserTransferCyclesQuest) -> Result<(), CTSUserTransferCyclesError> {
    if caller() != cts_id() {
        trap("Caller must be the CTS.")
    }
    
    
    
    
}





