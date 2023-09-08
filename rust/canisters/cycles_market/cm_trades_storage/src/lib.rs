use std::cell::RefCell;
use std::collections::HashMap;
use cts_lib::{
    tools::{
        localkey::{
            refcell::{with, with_mut}
        },
        stable_read_into_vec,
    },
    ic_cdk::{
        self,
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    },
    stable_memory_tools::{
        self,
        MemoryId,
    },
    types::cycles_market::tc::{
        PositionId, 
        PurchaseId, 
        ViewLatestTradesQuest, 
        ViewLatestTradesSponse, 
        LatestTradesDataItem, 
        MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA,
        trade_log,
    },
};


use cm_storage_lib::{
    self,
    StorageData,
    OldStorageData,
    STORAGE_DATA,
    STORAGE_DATA_MEMORY_ID,
    get_logs_storage_memory,    
    LogStorageInit,
    FlushQuest,
    FlushSuccess,
    FlushError,
};


// ----------------------------------





type PositionsPurchasesKey = PositionId;
type PositionsPurchasesVecValue = PurchaseId;
type PositionsPurchases = HashMap<PositionsPurchasesKey, Vec<PositionsPurchasesVecValue>>;





const POSITIONS_PURCHASES_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local!{
    static POSITIONS_PURCHASES: RefCell<PositionsPurchases> = RefCell::new(PositionsPurchases::new());
}





#[init]
fn init(q: LogStorageInit) {
    stable_memory_tools::init(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID);
    stable_memory_tools::init(&POSITIONS_PURCHASES, POSITIONS_PURCHASES_MEMORY_ID);
        
    with_mut(&STORAGE_DATA, |data| {
        data.set_log_size(q.log_size);
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::post_upgrade(&STORAGE_DATA, STORAGE_DATA_MEMORY_ID, None::<fn(OldStorageData) -> StorageData>);
    stable_memory_tools::post_upgrade(&POSITIONS_PURCHASES, POSITIONS_PURCHASES_MEMORY_ID, None::<fn(PositionsPurchases) -> PositionsPurchases>);
}


#[update]
pub fn flush(q: FlushQuest) -> Result<FlushSuccess, FlushError> {
    with_mut(&POSITIONS_PURCHASES, |positions_purchases| {
        cm_storage_lib::flush(
            q, 
            positions_purchases,
            trade_log::log_id_of_the_log_serialization,
            trade_log::index_keys_of_the_log_serialization,
        )    
    })
}

#[query(manual_reply = true)]
pub fn map_logs_rchunks(k: PositionsPurchasesKey, opt_start_before_id: Option<u128>, chunk_size: u32) {
    with(&POSITIONS_PURCHASES, |positions_purchases| {
        cm_storage_lib::map_logs_rchunks(&k, opt_start_before_id, chunk_size, positions_purchases);
    });
}





#[query]
pub fn view_latest_trades(q: ViewLatestTradesQuest) -> ViewLatestTradesSponse{
    
    let mut trades_data: Vec<LatestTradesDataItem> = vec![];
    let mut is_last_chunk_on_this_canister: bool = true;
    with(&STORAGE_DATA, |storage_data| {
        if storage_data.logs_memory_i() >= trade_log::STABLE_MEMORY_SERIALIZE_SIZE as u64 {
            
            let logs_memory = get_logs_storage_memory();
            
            let logs_storage_till_start_before_id_len_i: u64 = {
                q.opt_start_before_id
                    .map(|start_before_id| {
                        std::cmp::min(
                            {
                                start_before_id
                                    .checked_sub(storage_data.first_log_id())
                                    .unwrap_or(0) as u64
                                * 
                                storage_data.log_size() as u64
                            },
                            storage_data.logs_memory_i()
                        )
                    })
                    .unwrap_or(storage_data.logs_memory_i()) 
            };
            
            for i in 0..std::cmp::min(logs_storage_till_start_before_id_len_i / storage_data.log_size() as u64, MAX_LATEST_TRADE_LOGS_SPONSE_TRADE_DATA as u64) {
                
                let log_finish_i: u64 = logs_storage_till_start_before_id_len_i - i * storage_data.log_size() as u64;
                
                let log: Vec<u8> = stable_read_into_vec(
                    &logs_memory,
                    log_finish_i - storage_data.log_size() as u64,
                    storage_data.log_size() as usize,
                );
                    
                trades_data.push((
                    trade_log::log_id_of_the_log_serialization(&log),
                    trade_log::tokens_quantity_of_the_log_serialization(&log),
                    trade_log::rate_of_the_log_serialization(&log),
                    trade_log::timestamp_nanos_of_the_log_serialization(&log).try_into().unwrap() // good for a couple hundred years
                )); 
                
            }
            
            if trades_data.len() >= 1 && trades_data.first().unwrap().0 != storage_data.first_log_id() { /*unwrap cause if there is at least one in the sponse we know there is at least one on the canistec*/
                is_last_chunk_on_this_canister = false;
            }   
        }
    });
    
    ViewLatestTradesSponse {
        trades_data, 
        is_last_chunk_on_this_canister,
    }   
}














