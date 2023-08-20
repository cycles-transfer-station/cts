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
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                msg_cycles_available128,
                msg_cycles_accept128,
                reply,
                reply_raw,
            },
        },
        export::{candid::{CandidType}},
        query,
    },
    stable_memory_tools::{
        self,
        locate_minimum_memory
    },
    consts::GiB,
    types::cycles_market::icrc1token_trade_contract::{icrc1token_trade_log_storage::*},
};

use ic_stable_structures::{
    Memory,
    DefaultMemoryImpl, 
    memory_manager::{MemoryId, VirtualMemory},
};

use serde::{Serialize, Deserialize};
use serde_bytes::Bytes;




#[derive(Serialize, Deserialize)]
pub struct OldStorageData {}

#[derive(Serialize, Deserialize)]
pub struct StorageData {
    log_size: u32, // set in the canister_init
    first_log_id: u128,
    logs_memory_i: u64,
}

impl StorageData {
    fn new() -> Self {
        Self {
            log_size: 0u32,
            first_log_id: 0u128,
            logs_memory_i: 0u64,
        }
    }
    pub fn set_log_size(&mut self, log_size: u32) {
        self.log_size = log_size;
    }
}


pub const STORAGE_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);
const LOGS_STABLE_MEMORY_ID: MemoryId = MemoryId::new(1);
const MAX_STABLE_LOGS_STORAGE_BYTES: u64 = 20 * GiB as u64;
const MAX_HEAP_HASHMAP_SIZE: u64 = 1 * GiB as u64/*max hashmap size on the heap*/;



thread_local!{
    pub static STORAGE_DATA: RefCell<StorageData> = RefCell::new(StorageData::new());
}


fn get_logs_storage_memory() -> VirtualMemory<DefaultMemoryImpl> {
    stable_memory_tools::get_stable_memory(LOGS_STABLE_MEMORY_ID)
}




pub fn flush<K, F, Q>(q: FlushQuest, map: &mut HashMap<K, Vec<u128>>, log_id_of_the_log_serialization: F, index_key_of_the_log_serialization: Q) -> Result<FlushSuccess, FlushError> 
where
    K: PartialEq + Eq + Hash,
    F: Fn(&[u8]) -> u128,
    Q: Fn(&[u8]) -> K
{
    caller_is_controller_gaurd(&caller());
    
    msg_cycles_accept128(msg_cycles_available128());
    
    let log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_logs_storage_memory();
        
    with_mut(&STORAGE_DATA, |data| {     
        
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
            data.first_log_id = log_id_of_the_log_serialization(&q.bytes);//u128::from_be_bytes(q.bytes[16..32].try_into().unwrap());   
        }
                            
        log_storage_memory.write(
            data.logs_memory_i,
            &q.bytes
        );
        
        data.logs_memory_i += q.bytes.len() as u64;
        
        for i in 0..(q.bytes.len() / log_size) {
            let log_slice: &[u8] = &q.bytes[(i*log_size)..(i*log_size+log_size)];
            map.entry(index_key_of_the_log_serialization(log_slice))
                .or_insert(Vec::new())
                .push(log_id_of_the_log_serialization(log_slice));
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
        None => return,
        Some(vec) => {
            let vec_till_start_before_id = &vec[
                0
                ..
                ({
                    match opt_start_before_id {
                        None => vec.len(),
                        Some(start_before_id) => vec.binary_search(&start_before_id).unwrap(),
                    }
                })
            ];
            let rchunk: &[u128] = vec_till_start_before_id.rchunks(chunk_size.try_into().unwrap()).next().unwrap_or(&[]);
            
            let log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_logs_storage_memory();    
            
            with(&STORAGE_DATA, |data| {
                for log_id in rchunk.into_iter() {
                    let mut log_b: Vec<u8> = vec![0; data.log_size as usize];
                    log_storage_memory.read((log_id - data.first_log_id) as u64 * data.log_size as u64, &mut log_b);
                    reply_raw(&log_b);
                }
            });
        }
    }

}







#[derive(CandidType, Deserialize)]
pub struct ViewTradeLogsQuest {
    pub start_id: u128,
    pub length: u128,
}

#[derive(CandidType)]
pub struct ViewTradeLogsSponse<'a> {
    pub logs: &'a Bytes
}



// disable on replicated?
#[query(manual_reply = true)]
pub fn view_trade_logs(q: ViewTradeLogsQuest) { //-> ViewTradeLogsSponse {
    
    let mut logs: Vec<u8> = Vec::new();
    
    with(&STORAGE_DATA, |data| {
        if q.start_id < data.first_log_id {
            trap("start_id is less than the first_log_id in this storage canister")
        }         
        if q.start_id + q.length > data.first_log_id + logs_count(data) as u128 {
            trap("out of range, the last log requested is out of the range of this storage canister")
        }
        
        let start_i: u64 = (q.start_id - data.first_log_id) as u64 * data.log_size as u64;
        let finish_i: u64 = start_i + (q.length as u64 * data.log_size as u64);
        
        let memory = get_logs_storage_memory();
        
        logs = vec![0; (finish_i - start_i) as usize]; 
        
        memory.read(start_i, &mut logs);
       
    });
    
    reply::<(ViewTradeLogsSponse,)>((
        ViewTradeLogsSponse{
            logs: &Bytes::new(&logs),
        }
    ,));

}


fn logs_count(data: &StorageData) -> u64 {
    data.logs_memory_i / data.log_size as u64
}





