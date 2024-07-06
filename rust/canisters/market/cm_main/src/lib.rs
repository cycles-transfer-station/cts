use ic_cdk::{
    api::{
        trap,
        caller,
        call::{
            call,
            call_raw128,
        },
        canister_balance128,
    },
    init,
    pre_upgrade,
    post_upgrade,
    update,
    query
};
use cts_lib::{
    management_canister::*,
    types::{
        Cycles,
        CallError,
        canister_code::CanisterCode,
        cm::{*, cm_main::*, tc::CMIcrc1TokenTradeContractInit}
    },
    tools::{
        localkey::{
            refcell::{
                with, 
                with_mut,
            }
        },
        upgrade_canisters::*,
        caller_is_controller_gaurd,
        sha256,
        time_nanos_u64,
        call_error_as_u32_and_string,
    },
    consts::{MiB, TRILLION},
};
use canister_tools::{self, MemoryId};

use std::cell::{RefCell};
use serde::Serialize;
use candid::{
    Principal,
    CandidType, 
    Deserialize,
    utils::{encode_one}
};


// ----------------------



#[derive(CandidType, Serialize, Deserialize, Clone)]        
pub struct TradeContractData {
    tc_module_hash: [u8; 32],
    latest_upgrade_timestamp_nanos: u64,
}


#[derive(CandidType, Serialize, Deserialize)]
struct CMMainData {
    cts_id: Principal,
    // portant that the sequence of these stay the same and only get added on. the shareholder_payouts canister needs it. 
    trade_contracts: Vec<(TradeContractIdAndLedgerId, TradeContractData)>,
    tc_canister_code: CanisterCode,
    trades_storage_canister_code: CanisterCode,
    positions_storage_canister_code: CanisterCode,
    controller_create_icrc1token_trade_contract_mid_call_data: Option<ControllerCreateIcrc1TokenTradeContractMidCallData>,
    cycles_bank_id: Principal,
}

impl CMMainData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            trade_contracts: Vec::new(),
            tc_canister_code: CanisterCode::empty(),
            trades_storage_canister_code: CanisterCode::empty(),
            positions_storage_canister_code: CanisterCode::empty(),
            controller_create_icrc1token_trade_contract_mid_call_data: None,
            cycles_bank_id: Principal::from_slice(&[]),
        }
    }
}




const NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES: Cycles = {
    #[cfg(not(debug_assertions))]
    { 7 * TRILLION }
    #[cfg(debug_assertions)]
    { 100 * TRILLION }
};
    

const HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID: MemoryId = MemoryId::new(0);


thread_local! {
    static CM_MAIN_DATA: RefCell<CMMainData> = RefCell::new(CMMainData::new());
}



#[init]
fn init(cm_main_init: CMMainInit) {
    canister_tools::init(&CM_MAIN_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID);

    with_mut(&CM_MAIN_DATA, |cm_main_data| {
        cm_main_data.cts_id = cm_main_init.cts_id;
        cm_main_data.cycles_bank_id = cm_main_init.cycles_bank_id;
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    canister_tools::post_upgrade(&CM_MAIN_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID, None::<fn(CMMainData) -> CMMainData>);
}



// ----------------- UPLOAD-CANISTER-CODE --------------------


#[update]
pub fn controller_upload_canister_code(canister_code: CanisterCode, market_canister_type: MarketCanisterType) {
    caller_is_controller_gaurd(&caller());
    if *(canister_code.module_hash()) != sha256(canister_code.module()) {
        trap("module hash is not as given");
    } 
    with_mut(&CM_MAIN_DATA, |data| {
        let cc: &mut CanisterCode = match market_canister_type {
            MarketCanisterType::TradeContract =>    &mut data.tc_canister_code,
            MarketCanisterType::PositionsStorage => &mut data.positions_storage_canister_code,
            MarketCanisterType::TradesStorage =>    &mut data.trades_storage_canister_code,
        };
        *cc = canister_code;
    });
}


// ------------------------------------------------------





// --------

#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct ControllerCreateIcrc1TokenTradeContractMidCallData {
    start_time_nanos: u64,
    lock: bool,
    controller_create_icrc1token_trade_contract_quest: ControllerCreateIcrc1TokenTradeContractQuest,
    // options are for the steps
    icrc1token_trade_contract_canister_id: Option<Principal>,
    icrc1token_trade_contract_data: Option<TradeContractData>
}





fn unlock_and_write_controller_create_icrc1token_trade_contract_mid_call_data(mut mid_call_data: ControllerCreateIcrc1TokenTradeContractMidCallData, cm_main_data: &mut CMMainData) {
    mid_call_data.lock = false;
    cm_main_data.controller_create_icrc1token_trade_contract_mid_call_data = Some(mid_call_data);
}




#[update]
pub async fn controller_create_trade_contract(q: ControllerCreateIcrc1TokenTradeContractQuest) 
 -> Result<ControllerCreateIcrc1TokenTradeContractSuccess, ControllerCreateIcrc1TokenTradeContractError> {

    caller_is_controller_gaurd(&caller());
    
    let mid_call_data: ControllerCreateIcrc1TokenTradeContractMidCallData = with_mut(&CM_MAIN_DATA, |data| {
        match data.controller_create_icrc1token_trade_contract_mid_call_data {
            Some(ref mid_call_data) => {
                return Err(ControllerCreateIcrc1TokenTradeContractError::ControllerIsInTheMiddleOfADifferentCall(ControllerIsInTheMiddleOfADifferentCall{ 
                    kind: ControllerIsInTheMiddleOfADifferentCallKind::ControllerCreateIcrc1TokenTradeContract, 
                    must_call_continue: !mid_call_data.lock 
                }));
            },
            None => {
                let mid_call_data: ControllerCreateIcrc1TokenTradeContractMidCallData = ControllerCreateIcrc1TokenTradeContractMidCallData{
                    start_time_nanos: time_nanos_u64(),
                    lock: true,
                    controller_create_icrc1token_trade_contract_quest: q,
                    icrc1token_trade_contract_canister_id: None,
                    icrc1token_trade_contract_data: None,       
                };
                data.controller_create_icrc1token_trade_contract_mid_call_data = Some(mid_call_data.clone());
                Ok(mid_call_data)
            }
        }
    })?;
    
    controller_create_icrc1token_trade_contract_(mid_call_data).await

}



async fn controller_create_icrc1token_trade_contract_(mut mid_call_data: ControllerCreateIcrc1TokenTradeContractMidCallData) 
 -> Result<ControllerCreateIcrc1TokenTradeContractSuccess, ControllerCreateIcrc1TokenTradeContractError> {
    
    if let Some(tc_id_and_ledger_id) = with(&CM_MAIN_DATA, |cm_main_data| { 
        cm_main_data.trade_contracts.iter()
        .find(|t| { t.0.icrc1_ledger_canister_id == mid_call_data.controller_create_icrc1token_trade_contract_quest.icrc1_ledger_id })
        .map(|t| { t.0 })
    }) {
        with_mut(&CM_MAIN_DATA, |data| {
            data.controller_create_icrc1token_trade_contract_mid_call_data = None;
        });
        return Err(ControllerCreateIcrc1TokenTradeContractError::TradeContractForTheLedgerAlreadyCreated(tc_id_and_ledger_id));    
    }
    
    if mid_call_data.icrc1token_trade_contract_canister_id.is_none() {
        if canister_balance128() < NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES + 20*TRILLION {
            with_mut(&CM_MAIN_DATA, |data| {
                data.controller_create_icrc1token_trade_contract_mid_call_data = None;
            });
            return Err(ControllerCreateIcrc1TokenTradeContractError::CyclesBalanceTooLow{ cycles_balance: canister_balance128() });
        }
        match create_canister(
            ManagementCanisterCreateCanisterQuest{
                settings: Some(ManagementCanisterOptionalCanisterSettings{
                    controllers : Some(vec![
                        ic_cdk::api::id(), 
                        with(&CM_MAIN_DATA, |data| data.cts_id)
                    ]),
                    compute_allocation : None,
                    memory_allocation : Some(TC_CANISTER_NETWORK_MEMORY_ALLOCATION_MiB as u128 * MiB as u128),
                    freezing_threshold : None,
                })
            },
            NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES
        ).await {
            Ok(canister_id) => {
                mid_call_data.icrc1token_trade_contract_canister_id = Some(canister_id);
            }
            Err(create_canister_call_error) => {
                with_mut(&CM_MAIN_DATA, |data| {
                    data.controller_create_icrc1token_trade_contract_mid_call_data = None;
                });
                return Err(ControllerCreateIcrc1TokenTradeContractError::CreateCanisterIcrc1TokenTradeContractCallError(create_canister_call_error));
            }
        }
    }
    
    if mid_call_data.icrc1token_trade_contract_data.is_none() {        
        
        let tc_canister_code: CanisterCode = with(&CM_MAIN_DATA, |data| { 
            data.tc_canister_code.clone() 
        });
        
        let tc_module_hash: [u8; 32] = tc_canister_code.module_hash().clone();
        
        let cm_icrc1token_trade_contract_init: Vec<u8> = match with(&CM_MAIN_DATA, |data| {
            encode_one(CMIcrc1TokenTradeContractInit{
                cts_id: data.cts_id,
                cm_main_id: ic_cdk::api::id(),
                cycles_bank_id: data.cycles_bank_id,
                cycles_bank_transfer_fee: cts_lib::types::bank::BANK_TRANSFER_FEE,
                icrc1_token_ledger: mid_call_data.controller_create_icrc1token_trade_contract_quest.icrc1_ledger_id,
                icrc1_token_ledger_transfer_fee: mid_call_data.controller_create_icrc1token_trade_contract_quest.icrc1_ledger_transfer_fee,
                trades_storage_canister_code: data.trades_storage_canister_code.clone(),
                positions_storage_canister_code: data.positions_storage_canister_code.clone(),
            })
        }) {
            Ok(b) => b,
            Err(e) => {
                with_mut(&CM_MAIN_DATA, |cm_main_data| { unlock_and_write_controller_create_icrc1token_trade_contract_mid_call_data(mid_call_data, cm_main_data); });
                return Err(ControllerCreateIcrc1TokenTradeContractError::MidCallError(ControllerCreateIcrc1TokenTradeContractMidCallError::TCInitCandidEncodeError(format!("{:?}", e))));
            }  
        };
        
        match install_code(
            ManagementCanisterInstallCodeQuest{
                mode: ManagementCanisterInstallCodeMode::install,
                canister_id: mid_call_data.icrc1token_trade_contract_canister_id.as_ref().unwrap().clone(),
                wasm_module: tc_canister_code.module(),
                arg: &cm_icrc1token_trade_contract_init
            }
        ).await {
            Ok(()) => {
                mid_call_data.icrc1token_trade_contract_data = Some(TradeContractData{
                    tc_module_hash,
                    latest_upgrade_timestamp_nanos: time_nanos_u64(),
                });
            } 
            Err(call_error) => {
                with_mut(&CM_MAIN_DATA, |cm_main_data| { unlock_and_write_controller_create_icrc1token_trade_contract_mid_call_data(mid_call_data, cm_main_data); });
                return Err(ControllerCreateIcrc1TokenTradeContractError::MidCallError(ControllerCreateIcrc1TokenTradeContractMidCallError::InstallCodeIcrc1TokenTradeContractCallError(call_error)));
            }
        }
    }
    
    with_mut(&CM_MAIN_DATA, |data| {
        data.controller_create_icrc1token_trade_contract_mid_call_data = None;
        data.trade_contracts.push(
            (
                TradeContractIdAndLedgerId {
                    icrc1_ledger_canister_id: mid_call_data.controller_create_icrc1token_trade_contract_quest.icrc1_ledger_id,
                    trade_contract_canister_id: mid_call_data.icrc1token_trade_contract_canister_id.as_ref().unwrap().clone(),
                },
                mid_call_data.icrc1token_trade_contract_data.unwrap()
            )
        );
    });
    
    Ok(ControllerCreateIcrc1TokenTradeContractSuccess {
        trade_contract_canister_id: mid_call_data.icrc1token_trade_contract_canister_id.unwrap(),
    })

}
  

#[derive(CandidType, Deserialize)]
pub enum ContinueControllerCreateIcrc1TokenTradeContractError {
    ControllerIsNotInTheMiddleOfAControllerCreateIcrc1TokenTradeContractCall,
    ControllerCreateIcrc1TokenTradeContractError(ControllerCreateIcrc1TokenTradeContractError),
}

#[update]
pub async fn continue_controller_create_trade_contract() 
 -> Result<ControllerCreateIcrc1TokenTradeContractSuccess, ContinueControllerCreateIcrc1TokenTradeContractError> {
    
    caller_is_controller_gaurd(&caller());
    
    continue_controller_create_icrc1token_trade_contract_().await

}

async fn continue_controller_create_icrc1token_trade_contract_() 
 -> Result<ControllerCreateIcrc1TokenTradeContractSuccess, ContinueControllerCreateIcrc1TokenTradeContractError> {
    
    let mid_call_data: ControllerCreateIcrc1TokenTradeContractMidCallData = with_mut(&CM_MAIN_DATA, |data| {
        match data.controller_create_icrc1token_trade_contract_mid_call_data {
            Some(ref mut mid_call_data) => {
                if mid_call_data.lock == true {
                    return Err(ContinueControllerCreateIcrc1TokenTradeContractError::ControllerCreateIcrc1TokenTradeContractError(
                        ControllerCreateIcrc1TokenTradeContractError::ControllerIsInTheMiddleOfADifferentCall(ControllerIsInTheMiddleOfADifferentCall{ 
                            kind: ControllerIsInTheMiddleOfADifferentCallKind::ControllerCreateIcrc1TokenTradeContract, 
                            must_call_continue: false
                        })
                    ));
                }
                mid_call_data.lock = true;
                Ok(mid_call_data.clone())
            },
            None => {
                return Err(ContinueControllerCreateIcrc1TokenTradeContractError::ControllerIsNotInTheMiddleOfAControllerCreateIcrc1TokenTradeContractCall);
            }
        }
    })?;

    controller_create_icrc1token_trade_contract_(mid_call_data).await
        .map_err(|controller_create_icrc1token_trade_contract_error| { 
            ContinueControllerCreateIcrc1TokenTradeContractError::ControllerCreateIcrc1TokenTradeContractError(controller_create_icrc1token_trade_contract_error) 
        })
        
}




// ------------

#[query]
pub fn view_icrc1_token_trade_contracts() -> Vec<(TradeContractIdAndLedgerId, TradeContractData)> {
    with(&CM_MAIN_DATA, |cm_main_data| {
        cm_main_data.trade_contracts.clone()
    })
}

// ----------------




#[update]
pub async fn controller_upgrade_tcs(q: ControllerUpgradeCSQuest) -> Vec<(Principal, UpgradeOutcome)> {
    caller_is_controller_gaurd(&caller());
    
    let tc_cc: CanisterCode = with_mut(&CM_MAIN_DATA, |cm_main_data| {
        if let Some(new_canister_code) = q.new_canister_code {
            new_canister_code.verify_module_hash().unwrap();
            cm_main_data.tc_canister_code = new_canister_code; 
        }
        cm_main_data.tc_canister_code.clone()
    });
    
    let tcs: Vec<Principal> = match q.specific_cs {
        Some(tcs) => tcs.into_iter().collect(),
        None => {
            with(&CM_MAIN_DATA, |cm_main_data| {
                cm_main_data.trade_contracts.iter()
                .filter_map(|tc| {
                    if &tc.1.tc_module_hash != tc_cc.module_hash() {
                        Some(tc.0.trade_contract_canister_id.clone())
                    } else {
                        None
                    }
                })
                .take(200)
                .collect()
            })
        }
    };
    
    let rs: Vec<(Principal, UpgradeOutcome)> = upgrade_canisters(tcs, &tc_cc, &q.post_upgrade_quest).await;
    
    // update successes in the main data.
    with_mut(&CM_MAIN_DATA, |cm_main_data| {
        for (tc, uo) in rs.iter() {
            if let Some(ref r) = uo.install_code_result {
                if r.is_ok() {
                    if let Some(i) = cm_main_data.trade_contracts.iter_mut().find(|i| i.0.trade_contract_canister_id == *tc) {
                        i.1.tc_module_hash = tc_cc.module_hash().clone();
                        i.1.latest_upgrade_timestamp_nanos = time_nanos_u64();
                    } else {
                        ic_cdk::print("check this");
                    } 
                }
            }
        } 
    });
    
    return rs;
    
}




#[update]
pub async fn controller_upgrade_tc_log_storage_canisters(tc: Principal, q: ControllerUpgradeCSQuest, log_storage_type: LogStorageType) -> Result<Vec<(Principal, UpgradeOutcome)>, CallError> {
    caller_is_controller_gaurd(&caller());
    
    if let Some(ref new_cc) = q.new_canister_code {
        new_cc.verify_module_hash().unwrap();
        with_mut(&CM_MAIN_DATA, |cm_main_data| {
            let log_storage_cc: &mut CanisterCode = match log_storage_type {
                LogStorageType::Trades => &mut cm_main_data.trades_storage_canister_code,
                LogStorageType::Positions => &mut cm_main_data.positions_storage_canister_code
            };
            *log_storage_cc = new_cc.clone();
        });
    }
    
    call::<(ControllerUpgradeCSQuest, LogStorageType), (Vec<(Principal, UpgradeOutcome)>,)>(
        tc,
        "controller_upgrade_log_storage_canisters",
        (q, log_storage_type)        
    )
    .await
    .map(|t| t.0) // unwrap the one-tuple sponse
    .map_err(call_error_as_u32_and_string)
}


#[update]
pub async fn controller_view_tc_payouts_errors(tc: Principal, chunk_i: u32) -> Result<Vec<u8>, CallError> { 
    caller_is_controller_gaurd(&caller());
    
    call_raw128(
        tc,
        "controller_view_payouts_errors",
        encode_one(chunk_i).unwrap(),        
        0
    )
    .await
    .map_err(call_error_as_u32_and_string)
    
}




// ----- CONTROLLER_CALL_CANISTER-METHOD --------------------------

#[derive(CandidType, Deserialize)]
pub struct ControllerCallCanisterQuest {
    pub callee: Principal,
    pub method_name: String,
    pub arg_raw: Vec<u8>,
    pub cycles: Cycles
}

#[update]
pub async fn controller_call_canister(q: ControllerCallCanisterQuest) -> Result<Vec<u8>, CallError> {
    caller_is_controller_gaurd(&caller());
        
    call_raw128(
        q.callee,
        &q.method_name,
        &q.arg_raw,
        q.cycles
    )
    .await
    .map_err(call_error_as_u32_and_string)
}









ic_cdk::export_candid!();