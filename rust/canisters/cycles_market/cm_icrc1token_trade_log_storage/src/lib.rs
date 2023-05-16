use std::{
    cell::{Cell, RefCell},
};
use cts_lib::{
    tools::{
        localkey::{
            self,
            refcell::{with, with_mut}
        },
        caller_is_controller_gaurd,
    },
    consts::{
        MiB,
        WASM_PAGE_SIZE_BYTES,
    },
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            is_controller,
            call::{
                reply,
                msg_cycles_available128,
                msg_cycles_accept128,
                arg_data,
            },
            canister_balance128,
        },
        export::{
            Principal,
            candid::{
                self, 
                CandidType,
                Deserialize,
                utils::{encode_one, decode_one},
                error::Error as CandidError,
            }
        },
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    },
    stable_memory_tools::{
        create_state_snapshot,
        load_state_snapshot,
        write_state_snapshot_with_length_onto_the_stable_memory,
        read_stable_memory_bytes_with_length_onto_the_state_snapshot,
        locate_minimum_memory,
        get_memory,
    }

};

use ic_stable_structures::{
    Memory,
    DefaultMemoryImpl, 
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    
};

use serde_bytes::ByteBuf;



#[derive(CandidType, Deserialize)]
pub struct OldData {}

#[derive(CandidType, Deserialize)]
pub struct Data {
    log_size: u32,
    first_log_id: u128,
    trade_logs_memory_i: u64
}

impl Data {
    fn new() -> Self {
        Self {
            log_size: 0u32,
            first_log_id: 0u128,
            trade_logs_memory_i: 0u64
        }
    }
}








const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;

const STABLE_MEMORY_ID_HEAP_SERIALIZATION: MemoryId = MemoryId::new(0);
const STABLE_MEMORY_ID_TRADE_LOGS_STORAGE: MemoryId = MemoryId::new(1);




thread_local!{
    
    static DATA: RefCell<Data> = RefCell::new(Data::new());
    
}


// -------------------------

#[derive(CandidType, Deserialize)]
struct Icrc1TokenTradeLogStorageInit {
    log_size: u32,
    first_log_id: u128,
}

#[init]
fn init(q: Icrc1TokenTradeLogStorageInit) {
    with_mut(&DATA, |data| {
        data.log_size = q.log_size;
        data.first_log_id = q.first_log_id;
    });
}

 
#[pre_upgrade]
fn pre_upgrade() {
    with(&DATA, |data| {
        create_state_snapshot(data);
    });
    write_state_snapshot_with_length_onto_the_stable_memory(
        &get_memory(STABLE_MEMORY_ID_HEAP_SERIALIZATION),
        STABLE_MEMORY_HEADER_SIZE_BYTES,
    );
}

#[post_upgrade]
fn post_upgrade() {
    read_stable_memory_bytes_with_length_onto_the_state_snapshot(
        &get_memory(STABLE_MEMORY_ID_HEAP_SERIALIZATION),
        STABLE_MEMORY_HEADER_SIZE_BYTES
    );
    with_mut(&DATA, |data| {
        *data = load_state_snapshot(None::<fn(OldData) -> Data>).unwrap();
        //*data = load_state_snapshot(Some(|old_data: OldData| { Data{ d: 5 } })).unwrap();
    });
}


// --------------------------------------



#[derive(CandidType, Deserialize)]
pub struct FlushQuest {
    bytes: ByteBuf
}

#[derive(CandidType, Deserialize)]
pub struct FlushSuccess {}


#[derive(CandidType, Deserialize)]
pub enum FlushError {
    StorageIsFull,
}

#[update]
pub fn flush(q: FlushQuest) -> Result<FlushSuccess, FlushError> {
    caller_is_controller_gaurd(&caller());
    
    let trade_log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_memory(STABLE_MEMORY_ID_TRADE_LOGS_STORAGE);
        
    with(&DATA, |data| {     
    
        if let Err(_) = locate_minimum_memory(
            &trade_log_storage_memory,
            data.trade_logs_memory_i + q.bytes.len() as u64
        ) {
            return Err(FlushError::StorageIsFull);
        }

        trade_log_storage_memory.write(
            data.trade_logs_memory_i,
            &q.bytes
        );
        
        Ok(())
        
    })?;
    
    with_mut(&DATA, |data| {
        data.trade_logs_memory_i += q.bytes.len() as u64
    });
    
    Ok(FlushSuccess{})
}


// -----

#[query]
pub fn read_data() {}








// -------- STATE-SNAPSHOT ---------



#[update]
pub fn controller_create_state_snapshot() -> u64/*len of the state_snapshot*/ {
    caller_is_controller_gaurd(&caller());
        
    with(&DATA, |data| {
        create_state_snapshot(data).unwrap()
    })   
}

#[update]
pub fn controller_load_state_snapshot_data() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&DATA, |data| {
        *data = load_state_snapshot(None::<fn(OldData) -> Data>).unwrap();
    });
}


// ---------------------------------

