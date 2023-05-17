use std::cell::RefCell;
use cts_lib::{
    tools::{
        localkey::{
            refcell::{with, with_mut}
        },
        caller_is_controller_gaurd,
    },
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                reply,
                msg_cycles_available128,
                msg_cycles_accept128,
            },
            canister_balance128,
        },
        export::{
            candid::{
                CandidType,
                Deserialize,
            }
        },
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    },
    stable_memory_tools::{
        locate_minimum_memory,
        get_stable_memory,
        self,
    }

};

use ic_stable_structures::{
    Memory,
    DefaultMemoryImpl, 
    memory_manager::{MemoryId, VirtualMemory},
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
    stable_memory_tools::set_data(&DATA, |_old_data: OldData| { None });
    
    with_mut(&DATA, |data| {
        data.log_size = q.log_size;
        data.first_log_id = q.first_log_id;
    });
}

 
#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::set_data(&DATA, |_old_data: OldData| { None });
    stable_memory_tools::post_upgrade();
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
    
    let trade_log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_stable_memory(STABLE_MEMORY_ID_TRADE_LOGS_STORAGE);
        
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






