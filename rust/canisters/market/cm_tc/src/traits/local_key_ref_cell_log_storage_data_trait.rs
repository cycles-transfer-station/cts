use std::{
    thread::LocalKey,
    cell::RefCell,
};
use cts_lib::types::cm::tc::{
    LogStorageData,
    storage_logs::{
        position_log::PositionLog,
        trade_log::TradeLog,
    }
};
use crate::{POSITIONS_STORAGE_DATA, TRADES_STORAGE_DATA};



pub trait LocalKeyRefCellLogStorageDataTrait {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>>;    
}

impl LocalKeyRefCellLogStorageDataTrait for PositionLog {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>> = &POSITIONS_STORAGE_DATA;
}

impl LocalKeyRefCellLogStorageDataTrait for TradeLog {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>> = &TRADES_STORAGE_DATA;
}
