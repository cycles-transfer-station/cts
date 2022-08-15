use std::cell::{Cell, RefCell};
use cts_lib::{
    types::{
        user_canister_cache::UserCanisterCache,
    },
    tools::{
        localkey::{
            self,
            refcell::{with, with_mut}
        }
    },
    ic_cdk::{
        api::{
            call,
            
        },
        export::{
            Principal,
            candid::{
                self, 
                utils::{encode_one, decode_one}
            }
        }
    },
    ic_cdk_macros::{
        update,
        query,
    }
};


enum Commodity {
    Icp(IcpTokens),
    Cycles(Cycles)
}



struct Position {
    positor: Principal,
    positor_contract: Principal,
    commodity: Commodity,
    cost: Commodity,
    timestamp_nanos: u64,
    id: u128,
    purchase: Option<PositionPurchase>   
}

struct PositionPurchase {
    purchaser: Principal,
    purchaser_contract: Principal,
    timestamp_nanos: u64,
    icp_transfer_complete: bool // the purchase is [only] log if the cycles-transfer is success.    
}


struct CMData {
    cts_id: Principal,
    positions: Vec<Position>,
    
}

impl CMData {
    fn new() -> Self {
        Self {
            positions: Vec::new()
        }
    }
}



pub const CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;


thread_local! {

    static CM_DATA: RefCell<CMData> = RefCell::new(CMData::new()); 
    
    // not save through the upgrades
    static USER_CANISTER_CACHE: RefCell<UserCanisterCache> = RefCell::new(UserCanisterCache::new(1400));
    static STOP_CALLS: Cell<bool> = Cell::new(false);
    static STATE_SNAPSHOT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    
}


// -------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CMInit {
    cts_id: Principal,
} 

#[init]
fn init(cm_init: CMInit) {
    with_mut(&CM_DATA, |cm_data| { cm_data.cts_id = cm_init.cts_id; });
} 


// -------------------------------------------------------------


fn create_state_snapshot() {
    let mut cm_data_candid_bytes: Vec<u8> = with(&CM_DATA, |cm_data| { encode_one(cm_data).unwrap() });
    cm_data_candid_bytes.shrink_to_fit();
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = cm_data_candid_bytes; 
    });
}

fn load_state_snapshot_data() {
    
    let cm_data_of_the_state_snapshot: CMData = with(&STATE_SNAPSHOT, |state_snapshot| {
        match decode_one::<CMData>(state_snapshot) {
            Ok(cm_data) => cm_data,
            Err(_) => {
                trap("error decode of the state-snapshot CMData");
                /*
                let old_cm_data: OldCMData = decode_one::<OldCMData>(state_snapshot).unwrap();
                let cm_data: CMData = CMData{
                    cts_id: old_cm_data.cts_id
                    ........
                };
                cm_data
                */
            }
        }
    });

    with_mut(&CM_DATA, |cm_data| {
        *cm_data = cm_data_of_the_state_snapshot;    
    });
    
}

// -------------------------------------------------------------


#[pre_upgrade]
fn pre_upgrade() {
    
    create_state_snapshot();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;

    with(&STATE_SNAPSHOT, |state_snapshot| {
        let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*len of the state_snapshot*/ + state_snapshot.len() as u64; 
        if current_stable_size_bytes < want_stable_memory_size_bytes {
            stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
        }
        stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((state_snapshot.len() as u64).to_be_bytes()));
        stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, state_snapshot);
    });
}

#[post_upgrade]
fn post_upgrade() {
    let mut state_snapshot_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut state_snapshot_len_u64_be_bytes);
    let state_snapshot_len_u64: u64 = u64::from_be_bytes(state_snapshot_len_u64_be_bytes); 
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = vec![0; state_snapshot_len_u64 as usize]; 
        stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, state_snapshot);
    });
    
    load_state_snapshot_data();
} 


// -------------------------------------------------------------


fn cts_id() -> Principal {
    with(&CM_DATA, |cm_data| { cm_data.cts_id })
}






// -------------------------------------------------------------















// -------------------------------------------------------------





#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::get(&STOP_CALLS)
}





#[update]
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot*/ {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    create_state_snapshot();
    
    with(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT, |state_snapshot| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot.chunks(chunk_size).nth(chunk_i as usize),));
    });
}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_load_state_snapshot_data() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    load_state_snapshot_data();
}



// -------------------------------------------------------------



