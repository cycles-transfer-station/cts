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
        export::{
            Principal,
            candid::{
                CandidType, 
                Deserialize,
                utils::{encode_one, decode_one}
            },
        },
        init,
        pre_upgrade,
        post_upgrade,
        update,
        query
    },
    types::{
        Cycles,
        canister_code::CanisterCode
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
    },
    stable_memory_tools::{self, MemoryId},
    icrc::Tokens,
};
use std::cell::{RefCell};



#[derive(CandidType, Deserialize)]
pub struct Icrc1TokenTradeContract {
    icrc1_ledger_id: Principal,
    cycles_market_trade_contract_id: Principal,
    opt_cm_caller: Option<Principal>
}


#[derive(CandidType, Deserialize)]
struct Contracts {
    icrc1_token_trade_contracts: Vec<Icrc1TokenTradeContract>
}
impl Contracts {
    fn new() -> Self {
        Self {
            icrc1_token_trade_contracts: Vec::new(),
        }
    }
}

#[derive(CandidType, Deserialize)]
struct CMMainData {
    cts_id: Principal,
    contracts: Contracts,
    icrc1token_trade_contract_canister_code: CanisterCode,
    icrc1token_trade_log_storage_canister_code: CanisterCode,
    cm_caller_canister_code: CanisterCode,
}

impl CMMainData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            contracts: Contracts::new(),
            icrc1token_trade_contract_canister_code: CanisterCode::empty(),
            icrc1token_trade_log_storage_canister_code: CanisterCode::empty(),
            cm_caller_canister_code: CanisterCode::empty()
        }
    }
}

#[derive(CandidType, Deserialize)]
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
    stable_memory_tools::init(&CM_MAIN_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID);

    with_mut(&CM_MAIN_DATA, |cm_main_data| {
        cm_main_data.cts_id = cm_main_init.cts_id;
        cm_main_data.contracts.icrc1_token_trade_contracts = cm_main_init.icrc1_token_trade_contracts;
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    stable_memory_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    stable_memory_tools::post_upgrade(&CM_MAIN_DATA, HEAP_DATA_SERIALIZATION_STABLE_MEMORY_ID, None::<fn(OldCMMainData) -> CMMainData>);

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
        data.icrc1token_trade_log_storage_canister_code = canister_code;
    });
}

#[update]
pub fn controller_upload_cm_caller_canister_code(canister_code: CanisterCode) {
    caller_is_controller_gaurd(&caller());
    if *(canister_code.module_hash()) != sha256(canister_code.module()) {
        trap("module hash is not as given");
    } 
    with_mut(&CM_MAIN_DATA, |data| {
        data.cm_caller_canister_code = canister_code;
    });
}


#[derive(CandidType, Deserialize)]
pub struct ControllerCreateIcrc1TokenTradeContractQuest {
    pub icrc1_ledger_id: Principal,
    pub icrc1_ledger_transfer_fee: Tokens,
}

#[update]
pub fn controller_create_icrc1_token_trade_contract(q: ControllerCreateIcrc1TokenTradeContractQuest) -> Principal/*Icrc1TokenTradeContractId*/ {
    caller_is_controller_gaurd(&caller());
    
    // create canister for the trade-contract
    // create canister for the cm_caller
    // install code onto the cm_caller, with the trade-contract-canister-id in the init-params
    // install code onto the trade-contract
    // save
    // return
    todo!();
}






// ------------

#[query(manual_reply = true)]
pub fn see_icrc1_token_trade_contracts() {
    with(&CM_MAIN_DATA, |cm_main_data| {
        reply::<(&Vec<Icrc1TokenTradeContract>,)>((&(cm_main_data.contracts.icrc1_token_trade_contracts),));
    });
}







