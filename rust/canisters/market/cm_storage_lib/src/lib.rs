use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use cts_lib::{
    tools::{
        localkey::{
            refcell::{with, with_mut}
        },
        caller_is_controller_gaurd,
    },
    consts::GiB,
};
use ic_cdk::{
    self,
    api::{
        trap,
        caller,
        call::{
            msg_cycles_available128,
            msg_cycles_accept128,
        },
        stable::{
            WASM_PAGE_SIZE_IN_BYTES
        }
    },
    update,
};
use candid::CandidType;

use ic_stable_structures::{
    Memory,
    DefaultMemoryImpl, 
    memory_manager::{MemoryId, VirtualMemory},
};

use serde::{Serialize, Deserialize};
use serde_bytes::Bytes;




#[derive(CandidType, Serialize, Deserialize)]
pub struct OldStorageData {}

#[derive(CandidType, Serialize, Deserialize)]
pub struct StorageData {
    log_size: u32, // set in the canister_init
    first_log_id: u128,
    logs_memory_i: u64,
    controller_mark_full: bool,
}

impl StorageData {
    fn new() -> Self {
        Self {
            log_size: 0u32,
            first_log_id: 0u128,
            logs_memory_i: 0u64,
            controller_mark_full: false
        }
    }
    pub fn set_log_size(&mut self, log_size: u32) {
        self.log_size = log_size;
    }
    pub fn logs_memory_i(&self) -> u64 {
        self.logs_memory_i
    }
    pub fn first_log_id(&self) -> u128 {
        self.first_log_id
    }
    pub fn log_size(&self) -> u32 {
        self.log_size
    }
}


pub const STORAGE_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);
const LOGS_STABLE_MEMORY_ID: MemoryId = MemoryId::new(1);
const MAX_STABLE_LOGS_STORAGE_BYTES: u64 = 20 * GiB as u64;
const MAX_HEAP_HASHMAP_SIZE: u64 = 1 * GiB as u64/*max hashmap size on the heap*/;



thread_local!{
    pub static STORAGE_DATA: RefCell<StorageData> = RefCell::new(StorageData::new());
}


pub fn get_logs_storage_memory() -> VirtualMemory<DefaultMemoryImpl> {
    canister_tools::get_virtual_memory(LOGS_STABLE_MEMORY_ID)
}



#[derive(CandidType)]
pub struct FlushQuestForward<'a> {
    pub bytes: &'a Bytes
}

#[derive(CandidType, Deserialize)]
pub struct FlushQuest {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>
}

#[derive(CandidType, Deserialize)]
pub struct FlushSuccess {}

#[derive(CandidType, Deserialize)]
pub enum FlushError {
    StorageIsFull,
}

pub type FlushResult = Result<FlushSuccess, FlushError>;


pub fn flush<K, F, Q>(q: FlushQuest, map: &mut HashMap<K, Vec<u128>>, log_id_of_the_log_serialization: F, index_keys_of_the_log_serialization: Q) -> Result<FlushSuccess, FlushError> 
where
    K: PartialEq + Eq + Hash,
    F: Fn(&[u8]) -> u128,
    Q: Fn(&[u8]) -> Vec<K>
{
    caller_is_controller_gaurd(&caller());
    
    msg_cycles_accept128(msg_cycles_available128());
    
    let log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_logs_storage_memory();
        
    with_mut(&STORAGE_DATA, |data| {     
        
        if data.controller_mark_full == true {
            return Err(FlushError::StorageIsFull);
        }
        
        let log_size: usize = data.log_size.try_into().unwrap();
        
        if q.bytes.len() % log_size != 0 {
            trap("flush q.bytes.len() % data.log_size != 0");
        }
        
        if data.logs_memory_i + q.bytes.len() as u64 > MAX_STABLE_LOGS_STORAGE_BYTES {
            return Err(FlushError::StorageIsFull);
        }
            
        if map.len() as u64 * std::mem::size_of::<K>() as u64 + ((data.logs_memory_i + q.bytes.len() as u64) / data.log_size as u64) * std::mem::size_of::<u128>() as u64 >= MAX_HEAP_HASHMAP_SIZE {
            return Err(FlushError::StorageIsFull);        
        }
            
        if let Err(_) = locate_minimum_memory(
            &log_storage_memory,
            data.logs_memory_i + q.bytes.len() as u64
        ) {
            return Err(FlushError::StorageIsFull);
        }
          
        if data.logs_memory_i == 0 {
            data.first_log_id = log_id_of_the_log_serialization(&q.bytes);   
        }
                            
        log_storage_memory.write(
            data.logs_memory_i,
            &q.bytes
        );
        
        data.logs_memory_i += q.bytes.len() as u64;
        
        for i in 0..(q.bytes.len() / log_size) {
            let log_slice: &[u8] = &q.bytes[(i*log_size)..(i*log_size+log_size)];
            for index_key in index_keys_of_the_log_serialization(log_slice).into_iter() {
                map.entry(index_key)
                .or_insert(Vec::new())
                .push(log_id_of_the_log_serialization(log_slice));    
            }; 
        }
        
        Ok(())
        
    })?;
    
    Ok(FlushSuccess{})
}




pub fn map_logs_rchunks<K>(k: &K, opt_start_before_id: Option<u128>, chunk_size: u32, map: &HashMap<K, Vec<u128>>) /*manual replies with the serialized logs in rchunks*/ 
where
    K: PartialEq + Eq + Hash,
{
    match map.get(k) {
        None => {},
        Some(vec) => {
            let vec_till_start_before_id = &vec[
                0
                ..
                ({
                    match opt_start_before_id {
                        None => vec.len(),
                        Some(start_before_id) => vec.binary_search(&start_before_id).unwrap_or_else(|e| e),
                    }
                })
            ];
            let rchunk: &[u128] = vec_till_start_before_id.rchunks(chunk_size.try_into().unwrap()).next().unwrap_or(&[]);
            
            let log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_logs_storage_memory();    
            
            with(&STORAGE_DATA, |data| {
                for log_id in rchunk.into_iter() {
                    let mut log_b: Vec<u8> = vec![0; data.log_size as usize];
                    log_storage_memory.read((log_id - data.first_log_id) as u64 * data.log_size as u64, &mut log_b);
                    if log_b.len() != 0 {
                        unsafe { ic0::msg_reply_data_append(log_b.as_ptr() as i32, log_b.len() as i32); }
                    }
                }
            });
        }
    }
    unsafe { ic0::msg_reply(); }

}






#[update]
fn controller_mark_full(mark: bool) {
    caller_is_controller_gaurd(&caller());
    with_mut(&STORAGE_DATA, |data| {
        data.controller_mark_full = mark;
    })
}



#[update]
pub fn cm_update_log(log_id: u128, log_b: Vec<u8>) {
    caller_is_controller_gaurd(&caller());
    
    let logs_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_logs_storage_memory();
    
    with(&STORAGE_DATA, |data| {
        if log_b.len() != data.log_size as usize {
            trap("log_size is the wrong_length");
        }
        
        let log_start_memory_i: u64 = ((log_id.checked_sub(data.first_log_id).unwrap()) as u64).checked_mul(data.log_size as u64).unwrap();  
        
        if log_start_memory_i > data.logs_memory_i.checked_sub(data.log_size as u64).unwrap() {
            trap("log-id not found");
        }
        
        logs_storage_memory.write(log_start_memory_i, &log_b[..data.log_size as usize]);
    });

}









fn locate_minimum_memory(memory: &VirtualMemory<DefaultMemoryImpl>, want_memory_size_bytes: u64) -> Result<(),()> {
    let memory_size_wasm_pages: u64 = memory.size();
    let memory_size_bytes: u64 = memory_size_wasm_pages * WASM_PAGE_SIZE_IN_BYTES as u64;
    
    if memory_size_bytes < want_memory_size_bytes {
        let grow_result: i64 = memory.grow(((want_memory_size_bytes - memory_size_bytes) / WASM_PAGE_SIZE_IN_BYTES as u64) + 1);
        if grow_result == -1 {
            return Err(());
        }
    }
    
    Ok(())
}









