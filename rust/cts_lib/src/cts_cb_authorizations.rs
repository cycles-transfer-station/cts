use crate::types::cts::UserAndCB;
use candid::Principal;


pub const CTS_CB_AUTHORIZATIONS_SEED: &'static [u8; 21] = b"CTS-CB-AUTHORIZATIONS";
pub const IC_ROOT_KEY: [u8; 96] = [129, 76, 14, 110, 199, 31, 171, 88, 59, 8, 189, 129, 55, 60, 37, 92, 60, 55, 27, 46, 132, 134, 60, 152, 164, 241, 224, 139, 116, 35, 93, 20, 251, 93, 156, 12, 213, 70, 217, 104, 95, 145, 58, 12, 11, 44, 197, 52, 21, 131, 191, 75, 67, 146, 228, 103, 219, 150, 214, 91, 155, 180, 203, 113, 113, 18, 248, 71, 46, 13, 90, 77, 20, 80, 95, 253, 116, 132, 176, 18, 145, 9, 28, 95, 135, 185, 136, 131, 70, 63, 152, 9, 26, 11, 170, 174];


pub fn is_cts_cb_authorization_valid(cts_id: Principal, user_and_cb: UserAndCB, authorization: Vec<u8>) -> bool {
    canister_authorizations::verify(
        &user_and_cb.create_cts_cb_authorization_msg(),
        authorization,
        cts_id,
        CTS_CB_AUTHORIZATIONS_SEED,        
        {
            #[cfg(not(feature = "test"))]
            {IC_ROOT_KEY}
            #[cfg(feature = "test")]
            {localkey::cell::get(&LOCAL_IC_ROOT_KEY)}
        }
    )
}


#[cfg(feature = "test")]
mod local_put_ic_root_key {
    use super::*;
    use ic_cdk::api::call::{arg_data, reply}; 
    use std::cell::Cell;
    thread_local!{
        pub static LOCAL_IC_ROOT_KEY: Cell<[u8; 96]> = Cell::new([0; 96]);
    }
    #[export_name = "canister_update local_put_ic_root_key"] 
    pub fn local_put_ic_root_key() {
        localkey::cell::set(&LOCAL_IC_ROOT_KEY, arg_data::<(Vec<u8>,)>().0.try_into().unwrap());
        reply(());
    }
}
#[cfg(feature = "test")] use local_put_ic_root_key::LOCAL_IC_ROOT_KEY;
#[cfg(feature = "test")] use crate::tools::localkey;

