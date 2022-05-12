use global_allocator_counter::get_allocated_bytes_count;
use cts_lib::{
    types::{
        UserData
    },
    tools::{
        localkey_refcell::{with, with_mut},
    }
};
use std::{
    cell::RefCell,
    collections::HashMap,
};
use ic_cdk_macros::{update, query, init, pre_upgrade, post_upgrade};
use ic_cdk::{
    api::{
        trap,
        caller,

    },
    export::{
        Principal,
        candid::{
            CandidType,
            Deserialize,
        },
    }
};






thread_local! {
    static USER_DATA: RefCell<UserData> = RefCell::new(UserData::new());    
}










