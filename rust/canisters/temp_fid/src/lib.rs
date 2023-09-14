use cts_lib::{
    ic_cdk::{
        self,
        update,
        query,
        api::{
            is_controller,
            caller,
        },
        trap,
    },
    management_canister::{
        create_canister,
        ManagementCanisterCreateCanisterQuest
    },
    types::{
        CallError,
        Cycles
    }
};

use candid::Principal;


#[update]
pub async fn controller_create_canister(q:ManagementCanisterCreateCanisterQuest, with_cycles: Cycles) -> Result<Principal, CallError> {
    if is_controller(&caller()) == false {
        trap("Caller must be the controller for this method.");
    }
    create_canister(q, with_cycles).await
}