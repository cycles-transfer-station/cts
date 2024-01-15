use cts_lib::{
    types::{CallCanisterQuest, CallError},
    tools::call_error_as_u32_and_string
};
use ic_cdk::{
    update,
    api::call::call_raw128,
};

#[update]
pub async fn call_canister(q: CallCanisterQuest) -> Result<Vec<u8>, CallError> {
    call_raw128(
        q.callee,
        &q.method_name,
        q.arg_raw,
        q.cycles
    )
    .await
    .map_err(call_error_as_u32_and_string)
}