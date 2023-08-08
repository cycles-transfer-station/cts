use crate::*;




#[derive(Serialize, Deserialize)]
pub enum FlushTradeLogStorageError {
    CreateTradeLogStorageCanisterError(CreateTradeLogStorageCanisterError),
    TradeLogStorageCanisterCallError(CallError),
    NewTradeLogStorageCanisterIsFull, // when a *new* trade-log-storage-canister returns StorageIsFull on the first flush call. 
}



pub async fn flush_trade_logs() {
    
    with_mut(&CM_DATA, |cm_data| {
        while cm_data.trade_logs.len() > 0 {
            if cm_data.trade_logs[0].can_move_into_the_stable_memory_for_the_long_term_storage() == true {
                cm_data.trade_log_storage_buffer.extend(cm_data.trade_logs.pop_front().unwrap().into_stable_memory_serialize());
            } else {
                break; // bc want to save into the stable-memory in the correct sequence.
            }
        }
        
        if cm_data.trade_log_storage_buffer.len() >= FLUSH_TRADE_LOGS_STORAGE_BUFFER_AT_SIZE 
        && cm_data.trade_log_storage_flush_lock == false {
            cm_data.trade_log_storage_flush_lock = true;
        }
    });
    
    if with(&CM_DATA, |cm_data| { cm_data.trade_log_storage_flush_lock == true }) {
        
        let trade_log_storage_canister_id: Principal = {
            match with(&CM_DATA, |data| { 
                data.trade_log_storage_canisters
                    .last()
                    .and_then(|storage_canister| { 
                        if storage_canister.is_full { None } else { Some(storage_canister.canister_id) }
                    })
            }) {
                Some(c_id) => c_id,
                None => {
                    match create_trade_log_storage_canister().await {
                        Ok(p) => p,
                        Err(e) => {
                            with_mut(&CM_DATA, |data| {
                                data.trade_log_storage_flush_lock = false;
                                data.flush_trade_log_storage_errors.push((FlushTradeLogStorageError::CreateTradeLogStorageCanisterError(e), time_nanos_u64()));
                            });
                            return;
                        }
                    }
                }
            }
        };
        
        let chunk_sizes: Vec<usize>/*vec len is the num_of_chunks*/ = with(&CM_DATA, |cm_data| {
            cm_data.trade_log_storage_buffer.chunks(FLUSH_TRADE_LOGS_STORAGE_BUFFER_CHUNK_SIZE).map(|c| c.len()).collect::<Vec<usize>>()
        });
        
        for chunk_size in chunk_sizes.into_iter() {

            let chunk_future = with(&CM_DATA, |cm_data| {
                call_raw128( // <(FlushQuestForward,), (FlushResult,)>
                    trade_log_storage_canister_id,
                    "flush",
                    &encode_one(&
                        FlushQuestForward{
                            bytes: Bytes::new(&cm_data.trade_log_storage_buffer[..chunk_size]),
                        }
                    ).unwrap(),
                    10_000_000_000 // put some cycles for the trade-log-storage-canister
                )
            });
            
            match chunk_future.await {
                Ok(sb) => match decode_one::<FlushResult>(&sb).unwrap() {
                    Ok(_flush_success) => {
                        with_mut(&CM_DATA, |cm_data| {
                            cm_data.trade_log_storage_canisters.last_mut().unwrap().length += (chunk_size / TradeLog::STABLE_MEMORY_SERIALIZE_SIZE) as u64;
                            cm_data.trade_log_storage_buffer.drain(..chunk_size);
                        });
                    },
                    Err(flush_error) => match flush_error {
                        FlushError::StorageIsFull => {
                            with_mut(&CM_DATA, |cm_data| {
                                cm_data.trade_log_storage_canisters.last_mut().unwrap().is_full = true;
                            });
                            break;
                        }
                    }
                }
                Err(flush_call_error) => {
                    with_mut(&CM_DATA, |data| {
                        data.flush_trade_log_storage_errors.push((FlushTradeLogStorageError::TradeLogStorageCanisterCallError(call_error_as_u32_and_string(flush_call_error)), time_nanos_u64()));
                    });
                    break;
                }
            }
        }

        with_mut(&CM_DATA, |data| {
            data.trade_log_storage_flush_lock = false;
        });
    }
}





#[derive(Serialize, Deserialize)]
pub enum CreateTradeLogStorageCanisterError {
    CreateCanisterCallError(CallError),
    InstallCodeCandidError(String),
    InstallCodeCallError(CallError),
}

async fn create_trade_log_storage_canister() -> Result<Principal/*saves the trade-log-storage-canister-data in the CM_DATA*/, CreateTradeLogStorageCanisterError> {
    use management_canister::*;
    
    
    let canister_id: Principal = match with_mut(&CM_DATA, |data| { data.create_trade_log_storage_canister_temp_holder.take() }) {
        Some(canister_id) => canister_id,
        None => {
            match call_with_payment128::<(ManagementCanisterCreateCanisterQuest,), (CanisterIdRecord,)>(
                Principal::management_canister(),
                "create_canister",
                (ManagementCanisterCreateCanisterQuest{
                    settings: None,
                },),
                CREATE_TRADE_LOG_STORAGE_CANISTER_CYCLES, // cycles for the canister
            ).await {
                Ok(r) => r.0.canister_id,
                Err(call_error) => {
                    return Err(CreateTradeLogStorageCanisterError::CreateCanisterCallError(call_error_as_u32_and_string(call_error)));
                }
            }
        }
    };
    
    let mut module_hash: [u8; 32] = [0; 32]; // can't initalize an immutable variable from within a closure because the closure mutable-borrows it.
    
    match with(&CM_DATA, |data| {
        module_hash = data.trade_log_storage_canister_code.module_hash().clone();
        
        Ok(call_raw128(
            Principal::management_canister(),
            "install_code",
            &encode_one(
                ManagementCanisterInstallCodeQuest{
                    mode : ManagementCanisterInstallCodeMode::install,
                    canister_id : canister_id,
                    wasm_module : data.trade_log_storage_canister_code.module(),
                    arg : &encode_one(
                        Icrc1TokenTradeLogStorageInit{
                            log_size: TradeLog::STABLE_MEMORY_SERIALIZE_SIZE as u32,
                        }
                    ).map_err(|e| { CreateTradeLogStorageCanisterError::InstallCodeCandidError(format!("{:?}", e)) })?,
                }
            ).map_err(|e| { CreateTradeLogStorageCanisterError::InstallCodeCandidError(format!("{:?}", e)) })?,    
            0
        ))
        
    })?.await {
        Ok(_) => {
            with_mut(&CM_DATA, |data| {
                data.trade_log_storage_canisters.push(
                    TradeLogStorageCanisterData {
                        log_size: TradeLog::STABLE_MEMORY_SERIALIZE_SIZE as u32,
                        first_log_id: data.trade_log_storage_canisters.last().map(|c| c.first_log_id + c.length as u128).unwrap_or(0),
                        length: 0,
                        is_full: false,
                        canister_id: canister_id,
                        creation_timestamp: time_nanos(),
                        module_hash,
                    }
                );
            });
            Ok(canister_id)
        }
        Err(install_code_call_error) => {
            with_mut(&CM_DATA, |data| { data.create_trade_log_storage_canister_temp_holder = Some(canister_id); });
            return Err(CreateTradeLogStorageCanisterError::InstallCodeCallError(call_error_as_u32_and_string(install_code_call_error)));
        }
    }
    
}





