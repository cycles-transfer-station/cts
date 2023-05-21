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
                msg_cycles_available128,
                msg_cycles_accept128,
            },
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
        self,
        locate_minimum_memory
    },
    consts::GiB,
    types::cycles_market::icrc1token_trade_log_storage::*,
};

use ic_stable_structures::{
    Memory,
    DefaultMemoryImpl, 
    memory_manager::{MemoryId, VirtualMemory},
};



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


const STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION: MemoryId = MemoryId::new(0);
const STABLE_MEMORY_ID_TRADE_LOGS_STORAGE: MemoryId = MemoryId::new(1);
const MAX_STORAGE_BYTES: u64 = 50 * GiB as u64;



thread_local!{
    static DATA: RefCell<Data> = RefCell::new(Data::new());
}


// -------------------------


#[init]
fn init(q: Icrc1TokenTradeLogStorageInit) {
    stable_memory_tools::init(&DATA, STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION);
    
    with_mut(&DATA, |data| {
        data.log_size = q.log_size;
    });
}

 
#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::post_upgrade(&DATA, STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION, None::<fn(OldData) -> Data>);
}


// --------------------------------------


fn get_trade_logs_storage_memory() -> VirtualMemory<DefaultMemoryImpl> {
    stable_memory_tools::get_stable_memory(STABLE_MEMORY_ID_TRADE_LOGS_STORAGE)
}

// ----------------

#[update]
pub fn flush(q: FlushQuest) -> Result<FlushSuccess, FlushError> {
    caller_is_controller_gaurd(&caller());
    
    msg_cycles_accept128(msg_cycles_available128());
    
    let trade_log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_trade_logs_storage_memory();
        
    with(&DATA, |data| {     
        
        if data.trade_logs_memory_i + q.bytes.len() as u64 > MAX_STORAGE_BYTES {
            return Err(FlushError::StorageIsFull);
        }
            
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
        if data.trade_logs_memory_i == 0 {
            data.first_log_id = u128::from_be_bytes(q.bytes[16..32].try_into().unwrap());   
        }
        data.trade_logs_memory_i += q.bytes.len() as u64
    });
    
    Ok(FlushSuccess{})
}


// -----



// this function and then the move_complete_trade_logs_into_the_stable_memory function on the token_trade_contract
// disable on replicated?
#[query]
pub fn see_trade_logs(q: SeeTradeLogsQuest) -> StorageLogs {
    
    let mut logs: Vec<u8> = Vec::new();
    
    with(&DATA, |data| {
        if q.start_id < data.first_log_id {
            trap("start_id is less than the first_log_id in this storage canister")
        }         
        if q.start_id + q.length > data.first_log_id + logs_count(data) as u128 {
            trap("out of range, the last log requested is out of the range of this storage canister")
        }
        
        let start_i: u64 = (q.start_id - data.first_log_id) as u64 * data.log_size as u64;
        let finish_i: u64 = start_i + (q.length as u64 * data.log_size as u64);
        
        let memory = get_trade_logs_storage_memory();
        
        logs = vec![0; (finish_i - start_i) as usize]; 
        
        memory.read(start_i, &mut logs);
       
    });
    
    StorageLogs{
        logs,
    }

}


fn logs_count(data: &Data) -> u64 {
    data.trade_logs_memory_i / data.log_size as u64
}




