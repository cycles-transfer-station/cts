use cts_lib::{
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                reply,
            },
        },
        init,
        pre_upgrade,
        post_upgrade,
        update,
        query
    },
    management_canister::*,
    types::{
        Cycles,
        CallError,
        canister_code::CanisterCode,
        cycles_market::{cm_main::*, tc::CMIcrc1TokenTradeContractInit}
    },
    tools::{
        localkey::{
            refcell::{
                with, 
                with_mut,
            }
        },
        caller_is_controller_gaurd,
        sha256,
        time_nanos_u64,
    },
    icrc::Tokens,
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
        



#[derive(CandidType, Serialize, Deserialize)]
struct TradeContracts {
    icrc1_token_trade_contracts: Vec<Icrc1TokenTradeContract>
}
impl TradeContracts {
    fn new() -> Self {
        Self {
            icrc1_token_trade_contracts: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CMMainData {
    cts_id: Principal,
    trade_contracts: TradeContracts,
    icrc1token_trade_contract_canister_code: CanisterCode,
    icrc1token_trades_storage_canister_code: CanisterCode,
    icrc1token_positions_storage_canister_code: CanisterCode,
    controller_create_icrc1token_trade_contract_mid_call_data: Option<ControllerCreateIcrc1TokenTradeContractMidCallData>,
}

impl CMMainData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            trade_contracts: TradeContracts::new(),
            icrc1token_trade_contract_canister_code: CanisterCode::empty(),
            icrc1token_trades_storage_canister_code: CanisterCode::empty(),
            icrc1token_positions_storage_canister_code: CanisterCode::empty(),
            controller_create_icrc1token_trade_contract_mid_call_data: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct OldCMMainData {}






const NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES: Cycles = 5_000_000_000_000;
const NEW_ICRC1TOKEN_TRADE_CONTRACT_CM_CALLER_CYCLES: Cycles = 5_000_000_000_000;

const HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID: MemoryId = MemoryId::new(0);


thread_local! {
    static CM_MAIN_DATA: RefCell<CMMainData> = RefCell::new(CMMainData::new());
}


#[derive(CandidType, Deserialize)]
struct CMMainInit {
    cts_id: Principal,
    icrc1_token_trade_contracts: Vec<Icrc1TokenTradeContract>
}

#[init]
fn init(cm_main_init: CMMainInit) {
    canister_tools::init(&CM_MAIN_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID);

    with_mut(&CM_MAIN_DATA, |cm_main_data| {
        cm_main_data.cts_id = cm_main_init.cts_id;
        cm_main_data.trade_contracts.icrc1_token_trade_contracts = cm_main_init.icrc1_token_trade_contracts;
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    canister_tools::post_upgrade(&CM_MAIN_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID, None::<fn(OldCMMainData) -> CMMainData>);

}



// ----------------- UPLOAD-CANISTER-CODE --------------------

#[update]
pub fn controller_upload_icrc1token_trade_contract_canister_code(canister_code: CanisterCode) {
    caller_is_controller_gaurd(&caller());
    if *(canister_code.module_hash()) != sha256(canister_code.module()) {
        trap("module hash is not as given");
    } 
    with_mut(&CM_MAIN_DATA, |data| {
        data.icrc1token_trade_contract_canister_code = canister_code;
    });
}

#[update]
pub fn controller_upload_icrc1token_trade_log_storage_canister_code(canister_code: CanisterCode) {
    caller_is_controller_gaurd(&caller());
    if *(canister_code.module_hash()) != sha256(canister_code.module()) {
        trap("module hash is not as given");
    } 
    with_mut(&CM_MAIN_DATA, |data| {
        data.icrc1token_trades_storage_canister_code = canister_code;
    });
}

#[update]
pub fn controller_upload_icrc1token_positions_storage_canister_code(canister_code: CanisterCode) {
    caller_is_controller_gaurd(&caller());
    if *(canister_code.module_hash()) != sha256(canister_code.module()) {
        trap("module hash is not as given");
    } 
    with_mut(&CM_MAIN_DATA, |data| {
        data.icrc1token_positions_storage_canister_code = canister_code;
    });
}


// ------------------------------------------------------




#[derive(CandidType, Deserialize)]
pub struct ControllerIsInTheMiddleOfADifferentCall{
    kind: ControllerIsInTheMiddleOfADifferentCallKind,
    must_call_continue: bool
}
#[derive(CandidType, Deserialize)]
pub enum ControllerIsInTheMiddleOfADifferentCallKind {
    ControllerCreateIcrc1TokenTradeContract
}



// --------

#[derive(Serialize, Deserialize, Clone)]
pub struct ControllerCreateIcrc1TokenTradeContractMidCallData {
    start_time_nanos: u64,
    lock: bool,
    controller_create_icrc1token_trade_contract_quest: ControllerCreateIcrc1TokenTradeContractQuest,
    // options are for the steps
    icrc1token_trade_contract_canister_id: Option<Principal>,
    icrc1token_trade_contract_module_hash: Option<[u8; 32]>
}


#[derive(CandidType, Serialize, Deserialize, Clone)]
pub struct ControllerCreateIcrc1TokenTradeContractQuest {
    icrc1_ledger_id: Principal,
    icrc1_ledger_transfer_fee: Tokens,
}

#[derive(CandidType, Deserialize)]
pub struct ControllerCreateIcrc1TokenTradeContractSuccess {
    icrc1token_trade_contract_canister_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub enum ControllerCreateIcrc1TokenTradeContractError {
    ControllerIsInTheMiddleOfADifferentCall(ControllerIsInTheMiddleOfADifferentCall),
    CreateCanisterIcrc1TokenTradeContractCallError(CallError),
    MidCallError(ControllerCreateIcrc1TokenTradeContractMidCallError),
}

#[derive(CandidType, Deserialize)]
pub enum ControllerCreateIcrc1TokenTradeContractMidCallError {
    InstallCodeIcrc1TokenTradeContractCallError(CallError),
}



fn unlock_and_write_controller_create_icrc1token_trade_contract_mid_call_data(mut controller_create_icrc1token_trade_contract_mid_call_data: ControllerCreateIcrc1TokenTradeContractMidCallData) {
    controller_create_icrc1token_trade_contract_mid_call_data.lock = false;
    with_mut(&CM_MAIN_DATA, |data| {
        data.controller_create_icrc1token_trade_contract_mid_call_data = Some(controller_create_icrc1token_trade_contract_mid_call_data);
    });
}




#[update]
pub async fn controller_create_icrc1token_trade_contract(q: ControllerCreateIcrc1TokenTradeContractQuest) 
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
                    icrc1token_trade_contract_module_hash: None,       
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
    
    if mid_call_data.icrc1token_trade_contract_canister_id.is_none() {
        match create_canister(
            ManagementCanisterCreateCanisterQuest{
                settings: Some(ManagementCanisterOptionalCanisterSettings{
                    controllers : Some(vec![
                        ic_cdk::api::id(), 
                        with(&CM_MAIN_DATA, |data| data.cts_id)
                    ]),
                    compute_allocation : None,
                    memory_allocation : None,
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
    
    if mid_call_data.icrc1token_trade_contract_module_hash.is_none() {
        
        let icrc1token_trade_contract_canister_code: CanisterCode = with(&CM_MAIN_DATA, |data| { data.icrc1token_trade_contract_canister_code.clone() });
        
        let module_hash: [u8; 32] = icrc1token_trade_contract_canister_code.module_hash().clone();
        
        let cm_icrc1token_trade_contract_init: Vec<u8> = with(&CM_MAIN_DATA, |data| {
            encode_one(CMIcrc1TokenTradeContractInit{
                cts_id: data.cts_id,
                cm_main_id: ic_cdk::api::id(),
                icrc1_token_ledger: mid_call_data.controller_create_icrc1token_trade_contract_quest.icrc1_ledger_id,
                icrc1_token_ledger_transfer_fee: mid_call_data.controller_create_icrc1token_trade_contract_quest.icrc1_ledger_transfer_fee,
                trades_storage_canister_code: data.icrc1token_trades_storage_canister_code.clone(),
                positions_storage_canister_code: data.icrc1token_positions_storage_canister_code.clone(),
            }).unwrap()
        });
        
        match install_code(
            ManagementCanisterInstallCodeQuest{
                mode: ManagementCanisterInstallCodeMode::install,
                canister_id: mid_call_data.icrc1token_trade_contract_canister_id.as_ref().unwrap().clone(),
                wasm_module: icrc1token_trade_contract_canister_code.module(),
                arg: &cm_icrc1token_trade_contract_init
            }
        ).await {
            Ok(()) => {
                mid_call_data.icrc1token_trade_contract_module_hash = Some(module_hash);
            } 
            Err(call_error) => {
                unlock_and_write_controller_create_icrc1token_trade_contract_mid_call_data(mid_call_data);
                return Err(ControllerCreateIcrc1TokenTradeContractError::MidCallError(ControllerCreateIcrc1TokenTradeContractMidCallError::InstallCodeIcrc1TokenTradeContractCallError(call_error)));
            }
        }
    }
    
    with_mut(&CM_MAIN_DATA, |data| {
        data.controller_create_icrc1token_trade_contract_mid_call_data = None;
        data.trade_contracts.icrc1_token_trade_contracts.push(
            Icrc1TokenTradeContract {
                icrc1_ledger_canister_id: mid_call_data.controller_create_icrc1token_trade_contract_quest.icrc1_ledger_id,
                trade_contract_canister_id: mid_call_data.icrc1token_trade_contract_canister_id.as_ref().unwrap().clone(),
            }
        );
    });
    
    Ok(ControllerCreateIcrc1TokenTradeContractSuccess {
        icrc1token_trade_contract_canister_id: mid_call_data.icrc1token_trade_contract_canister_id.unwrap(),
    })

}
  

#[derive(CandidType, Deserialize)]
pub enum ContinueControllerCreateIcrc1TokenTradeContractError {
    ControllerIsNotInTheMiddleOfAControllerCreateIcrc1TokenTradeContractCall,
    ControllerCreateIcrc1TokenTradeContractError(ControllerCreateIcrc1TokenTradeContractError),
}

#[update]
pub async fn continue_controller_create_icrc1token_trade_contract() 
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

#[query(manual_reply = true)]
pub fn view_icrc1_token_trade_contracts() {
    with(&CM_MAIN_DATA, |cm_main_data| {
        reply::<(&Vec<Icrc1TokenTradeContract>,)>((&(cm_main_data.trade_contracts.icrc1_token_trade_contracts),));
    });
}






// ----------------


