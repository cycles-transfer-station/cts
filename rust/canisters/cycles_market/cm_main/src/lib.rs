use cts_lib::{
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                call_with_payment128,
                call_raw128,
                CallResult,
                arg_data,
                arg_data_raw_size,
                reply,
                RejectionCode,
                msg_cycles_available128,
                msg_cycles_accept128,
                msg_cycles_refunded128    
            },
            stable::{
                stable64_grow,
                stable64_read,
                stable64_size,
                stable64_write,
            }
        },
        export::{
            Principal,
            candid::{
                self, 
                CandidType, 
                Deserialize,
                utils::{encode_one, decode_one}
            },
        },
    },
    ic_cdk_macros::{
        init,
        pre_upgrade,
        post_upgrade,
        update,
        query
    },
    types::{
        Cycles,
        cm_caller::*
    },
    tools::{
        localkey::{
            self,
            refcell::{
                with, 
                with_mut,
            },
            cell::{
                get,
                set
            }
        }
    },
    consts::{
        WASM_PAGE_SIZE_BYTES,
        MANAGEMENT_CANISTER_ID,
    }
};
use std::cell::{Cell, RefCell};




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
}

impl CMMainData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            contracts: Contracts::new()
        }
    }
}







const NewIcrc1TokenTradeContractCycles: Cycles = 5_000_000_000_000;
const NewIcrc1TokenTradeContractCMCallerCycles: Cycles = 5_000_000_000_000;



thread_local! {
    static CM_MAIN_DATA: RefCell<CMMainData> = RefCell::new(CMMainData::new());
}


#[derive(CandidType, Deserialize)]
struct CMMainInit {
    cts_id: Principal,
    icrc1_token_trade_contracts: Vec<Icrc1TokenTradeContract>
}

#[init]
fn init(init: CMMainInit) {
    with_mut(&CM_MAIN_DATA, |cm_main_data| {
        cm_main_data.cts_id = init.cts_id;
        cm_main_data.contracts.icrc1_token_trade_contracts = init.icrc1_token_trade_contracts;
    });
}




// upgrades





// ----------------------------



#[query(manual_reply = true)]
pub fn see_icrc1_token_trade_contracts() {
    with(&CM_MAIN_DATA, |cm_main_data| {
        reply::<(&Vec<Icrc1TokenTradeContract>,)>((&(cm_main_data.contracts.icrc1_token_trade_contracts),));
    });
}


#[update]
pub fn create_icrc1_token_trade_contract(icrc1_ledger_id: Principal) -> Principal/*Icrc1TokenTradeContractId*/ {
    //if authorized_callers.contains(caller()) == false { trap("caller is with a lack of an authorization."); }
    
    // create canister for the trade-contract
    // create canister for the cm_caller
    // install code onto the cm_caller, with the trade-contract-canister-id in the init-params
    // install code onto the trade-contract
    // save
    // return
}





















