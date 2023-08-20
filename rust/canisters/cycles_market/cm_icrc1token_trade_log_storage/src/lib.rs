use std::cell::RefCell;
use std::collections::HashMap;
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
            },
        },
        export::{candid::{CandidType}, Principal},
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
    types::cycles_market::icrc1token_trade_contract::{PositionId, PurchaseId, icrc1token_trade_log_storage::*},
};

use ic_stable_structures::{
    Memory,
    DefaultMemoryImpl, 
    memory_manager::{MemoryId, VirtualMemory},
};

use serde::{Serialize, Deserialize};
use serde_bytes::Bytes;

#[derive(Serialize, Deserialize)]
pub struct OldData {}

#[derive(Serialize, Deserialize)]
pub struct Data {
    log_size: u32,
    first_log_id: u128,
    trade_logs_memory_i: u64,
    positions_purchases: HashMap<PositionId, Vec<PurchaseId>>
}

impl Data {
    fn new() -> Self {
        Self {
            log_size: 0u32,
            first_log_id: 0u128,
            trade_logs_memory_i: 0u64,
            positions_purchases: HashMap::new(),
        }
    }
}


const STABLE_MEMORY_ID_HEAP_DATA_SERIALIZATION: MemoryId = MemoryId::new(0);
const STABLE_MEMORY_ID_TRADE_LOGS_STORAGE: MemoryId = MemoryId::new(1);
const MAX_STABLE_TRADE_LOGS_STORAGE_BYTES: u64 = 20 * GiB as u64;
const MAX_POSITIONS_PURCHASES_HEAP_HASHMAP_SIZE: u64 = 1 * GiB as u64/*max hashmap size on the heap*/;



thread_local!{
    static DATA: RefCell<Data> = RefCell::new(Data::new());
}


// -------------------------


#[init]
fn init(q: LogStorageInit) {
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

pub fn position_id_of_the_serialization(b: &[u8]) -> PositionId {
    u128::from_be_bytes(b[0..16].try_into().unwrap())
}
pub fn purchase_id_of_the_serialization(b: &[u8]) -> PurchaseId {
    u128::from_be_bytes(b[16..32].try_into().unwrap())
}





// ----------------

#[update]
pub fn flush(q: FlushQuest) -> Result<FlushSuccess, FlushError> {
    caller_is_controller_gaurd(&caller());
    
    msg_cycles_accept128(msg_cycles_available128());
    
    let trade_log_storage_memory: VirtualMemory<DefaultMemoryImpl> = get_trade_logs_storage_memory();
        
    with_mut(&DATA, |data| {     
        
        let log_size: usize = data.log_size.try_into().unwrap();
        
        if q.bytes.len() % log_size != 0 {
            trap("flush q.bytes.len() % data.log_size != 0");
        }
        
        if data.trade_logs_memory_i + q.bytes.len() as u64 > MAX_STABLE_TRADE_LOGS_STORAGE_BYTES {
            return Err(FlushError::StorageIsFull);
        }
            
        if data.positions_purchases.len() as u64 * std::mem::size_of::<PositionId>() as u64 + ((data.trade_logs_memory_i + q.bytes.len() as u64) / data.log_size as u64) * std::mem::size_of::<PurchaseId>() as u64 >= MAX_POSITIONS_PURCHASES_HEAP_HASHMAP_SIZE {
            return Err(FlushError::StorageIsFull);        
        }
            
        if let Err(_) = locate_minimum_memory(
            &trade_log_storage_memory,
            data.trade_logs_memory_i + q.bytes.len() as u64
        ) {
            return Err(FlushError::StorageIsFull);
        }
          
        if data.trade_logs_memory_i == 0 {
            data.first_log_id = purchase_id_of_the_serialization(&q.bytes);//u128::from_be_bytes(q.bytes[16..32].try_into().unwrap());   
        }
                            
        trade_log_storage_memory.write(
            data.trade_logs_memory_i,
            &q.bytes
        );
        
        data.trade_logs_memory_i += q.bytes.len() as u64;
        
        for i in 0..(q.bytes.len() / log_size) {
            let log_slice: &[u8] = &q.bytes[(i*log_size)..(i*log_size+log_size)];
            data.positions_purchases.entry(position_id_of_the_serialization(log_slice))
                .or_insert(Vec::new())
                .push(purchase_id_of_the_serialization(log_slice));
        }
        
        Ok(())
        
    })?;
    
    Ok(FlushSuccess{})
}


// -----



#[derive(CandidType, Deserialize)]
pub struct ViewTradeLogsQuest {
    pub start_id: u128,
    pub length: u128,
}

#[derive(CandidType)]
pub struct ViewTradeLogsSponse<'a> {
    pub logs: &'a Bytes
}



// this function and then the move_complete_trade_logs_into_the_stable_memory function on the token_trade_contract
// disable on replicated?
#[query(manual_reply = true)]
pub fn view_trade_logs(q: ViewTradeLogsQuest) { //-> ViewTradeLogsSponse {
    
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
    
    reply::<(ViewTradeLogsSponse,)>((
        ViewTradeLogsSponse{
            logs: &Bytes::new(&logs),
        }
    ,));

}


fn logs_count(data: &Data) -> u64 {
    data.trade_logs_memory_i / data.log_size as u64
}




