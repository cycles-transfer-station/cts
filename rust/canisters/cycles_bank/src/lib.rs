// this canister can safe stop before upgrade


// use the heartbeat/timer for the check of the per_month_storage cost and check the cts-fuel in each call. if either one is 0, send the cycles-balance to the cts to save for the 10 years (calculate that the user pays for these 10 years in the first create_user_contract payment.)and delete 

// once the cts fuel is 0, send the cycles balance and the user_id to the cts for the safekeep. and block user calls (stop-calls flag?). the user can topup the user-contract-ctsfuel through the CTS-MAIN.
// when the storage/contract-duration is done, send the cycles balance and the user_id to the cts for the safekeep. and transfer the user-canister-cycles to the cts-main and delete the user-canister.

// let the user dowload the cycles-transfers (of the specific-logs-ids?)  with an autograph by the cts-main. charge for the cts-fuel. autograph by the cts-main is given when the user-contract is create that this user-contract is a part of the CTS.


//static USER_CANISTER_CTSFUEL_AUTO_TOPUP_PER_MONTH:               Cell<CTSFuel>               = Cell::new(0);   
//static USER_CANISTER_CTSFUEL_AUTO_TOPUP_PER_MONTH_LAST_CHARGE_TIMESTAMP_SECONDS: Cell<u64>   = Cell::new(0);
    

    
// ------------------------------------------------


use std::{
    cell::{RefCell,Cell}
};
use cts_lib::{
    ic_cdk::{
        self,
        api::{
            id,
            trap,
            caller,
            canister_balance128,
            performance_counter,
            call::{
                msg_cycles_accept128,
                msg_cycles_available128,
                msg_cycles_refunded128,
                RejectionCode,
                reject,
                reply,
                CallResult,
                arg_data,
                arg_data_raw_size,
                call,
                call_with_payment128,
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
                utils::{
                    encode_one,
                    decode_one
                }
            },
        }
    },
    ic_cdk_macros::{
        update, 
        query, 
        init, 
        pre_upgrade, 
        post_upgrade
    },
    ic_ledger_types::{
        IcpTokens,
        IcpBlockHeight,
        IcpAccountBalanceArgs,
        IcpId,
        IcpIdSub,
        icp_account_balance,
        MAINNET_LEDGER_CANISTER_ID
    },
    types::{
        Cycles,
        CTSFuel,
        CyclesTransfer,
        CyclesTransferMemo,
        XdrPerMyriadPerIcp,
        cts,
        cycles_transferrer,
        cycles_bank::{
            CyclesBankInit,
            LengthenLifetimeQuest
        },
        management_canister,
        cycles_market,
    },
    consts::{
        KiB,
        MiB,
        GiB,
        NANOS_IN_A_SECOND,
        SECONDS_IN_A_DAY,
        WASM_PAGE_SIZE_BYTES,
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES,
        MANAGEMENT_CANISTER_ID,
    },
    tools::{
        time_nanos,
        time_seconds,
        localkey::{
            self,
            refcell::{
                with, 
                with_mut,
            }
        },
        icptokens_to_cycles,
        cycles_to_icptokens
    },
    global_allocator_counter::get_allocated_bytes_count
};



// -------------------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CyclesTransferIn {
    id: u128,
    by_the_canister: Principal,
    cycles: Cycles,
    cycles_transfer_memo: CyclesTransferMemo,       // save max 32-bytes of the memo, of a Blob or of a Text
    timestamp_nanos: u128
}

#[derive(CandidType, Deserialize)]
struct CyclesTransferOut {
    id: u128,
    for_the_canister: Principal,
    cycles_sent: Cycles,
    cycles_refunded: Option<Cycles>,   // option cause this field is only filled in the callback and that might not come back because of the callee holding-back the callback cross-upgrades. // if/when a user deletes some CyclesTransferPurchaseLogs, let the user set a special flag to delete the still-not-come-back-user_transfer_cycles by default unset.
    cycles_transfer_memo: CyclesTransferMemo,                           // save max 32-bytes of the memo, of a Blob or of a Text
    timestamp_nanos: u128, // time sent
    opt_cycles_transfer_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)>, // None means the cycles_transfer-call replied. // save max 20-bytes of the string
    fee_paid: u64 // cycles_transferrer_fee
}

#[derive(CandidType, Deserialize)]
struct CMCyclesPosition{
    id: cycles_market::PositionId,   
    cycles: Cycles,
    minimum_purchase: Cycles,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    create_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct CMIcpPosition{
    id: cycles_market::PositionId,   
    icp: IcpTokens,
    minimum_purchase: IcpTokens,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    create_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct CMCyclesPositionPurchase{
    cycles_position_id: cycles_market::PositionId,
    cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    id: cycles_market::PurchaseId,
    cycles: Cycles,
    purchase_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct CMIcpPositionPurchase{
    icp_position_id: cycles_market::PositionId,
    icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    id: cycles_market::PurchaseId,
    icp: IcpTokens,
    purchase_position_fee: u64,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct CMIcpTransferOut{
    icp: IcpTokens,
    icp_fee: IcpTokens,
    to: IcpId,
    block_height: u128,
    timestamp_nanos: u128,
    transfer_icp_balance_fee: u64
}


#[derive(CandidType, Deserialize)]
struct UserData {
    cycles_balance: Cycles,
    cycles_transfers_in: Vec<CyclesTransferIn>,
    cycles_transfers_out: Vec<CyclesTransferOut>,
    cm_cycles_positions: Vec<CMCyclesPosition>,
    cm_icp_positions: Vec<CMIcpPosition>,
    cm_cycles_positions_purchases: Vec<CMCyclesPositionPurchase>,
    cm_icp_positions_purchases: Vec<CMIcpPositionPurchase>,    
    cm_icp_transfers_out: Vec<CMIcpTransferOut>
}

impl UserData {
    fn new() -> Self {
        Self {
            cycles_balance: 0u128,
            cycles_transfers_in: Vec::new(),
            cycles_transfers_out: Vec::new(),
            cm_cycles_positions: Vec::new(),
            cm_icp_positions: Vec::new(),
            cm_cycles_positions_purchases: Vec::new(),
            cm_icp_positions_purchases: Vec::new(),    
            cm_icp_transfers_out: Vec::new(),
        }
    }
}



#[derive(CandidType, Deserialize)]
struct CBData {
    user_canister_creation_timestamp_nanos: u128,
    cts_id: Principal,
    cbsm_id: Principal,
    cycles_market_id: Principal,
    user_id: Principal,
    storage_size_mib: u128,
    lifetime_termination_timestamp_seconds: u128,
    cycles_transferrer_canisters: Vec<Principal>,
    user_data: UserData,
    cycles_transfers_id_counter: u128,
}

impl CBData {
    fn new() -> Self {
        Self {
            user_canister_creation_timestamp_nanos: 0,
            cts_id: Principal::from_slice(&[]),
            cbsm_id: Principal::from_slice(&[]),
            cycles_market_id: Principal::from_slice(&[]),
            user_id: Principal::from_slice(&[]),
            storage_size_mib: 0,       // memory-allocation/2 // is with the set in the canister_init // in the mib // starting at a 50mib-storage with a 1-year-user_canister_lifetime with a 5T-cycles-ctsfuel-balance at a cost: 10T-CYCLES   // this value is half of the user-canister-memory_allocation. for the upgrades.  
            lifetime_termination_timestamp_seconds: 0,
            cycles_transferrer_canisters: Vec::new(),
            user_data: UserData::new(),
            cycles_transfers_id_counter: 0,        
        }
    }
}


pub const CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

pub const CYCLES_MARKET_CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const CYCLES_MARKET_PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;
pub const CYCLES_MARKET_TRANSFER_ICP_BALANCE_FEE: Cycles = 50_000_000_000;

const USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE: usize = 32;
const MINIMUM_USER_TRANSFER_CYCLES: Cycles = 10_000_000_000;
const CYCLES_TRANSFER_IN_MINIMUM_CYCLES: Cycles = 10_000_000_000;

const USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE: usize = 500usize;
const USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE: usize = 500usize;

const USER_DOWNLOAD_CM_CYCLES_POSITIONS_CHUNK_SIZE: usize = 500;
const USER_DOWNLOAD_CM_ICP_POSITIONS_CHUNK_SIZE: usize = 500;
const USER_DOWNLOAD_CM_CYCLES_POSITIONS_PURCHASES_CHUNK_SIZE: usize = 500;
const USER_DOWNLOAD_CM_ICP_POSITIONS_PURCHASES_CHUNK_SIZE: usize = 500;
const USER_DOWNLOAD_CM_ICP_TRANSFERS_OUT_CHUNK_SIZE: usize = 500;


const MINIMUM_LENGTHEN_LIFETIME_SECONDS: u128 = SECONDS_IN_A_DAY * 30;

const DELETE_LOG_MINIMUM_WAIT_NANOS: u128 = NANOS_IN_A_SECOND * SECONDS_IN_A_DAY * 45;

const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;

const USER_CANISTER_BACKUP_CYCLES: Cycles = 1_400_000_000_000; 

const CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE: &'static str = "ctsfuel_balance is too low";


thread_local! {
   
    static CB_DATA: RefCell<CBData> = RefCell::new(CBData::new());

    // not save in a CBData
    static MEMORY_SIZE_AT_THE_START: Cell<usize> = Cell::new(0); 
    static CYCLES_TRANSFERRER_CANISTERS_ROUND_ROBIN_COUNTER: Cell<usize> = Cell::new(0);
    static STOP_CALLS: Cell<bool> = Cell::new(false);
    static STATE_SNAPSHOT_CB_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());

}



// ---------------------------------------------------------------------------------


#[init]
fn canister_init(user_canister_init: CyclesBankInit) {
    
    with_mut(&CB_DATA, |cb_data| {
        *cb_data = CBData{
            user_canister_creation_timestamp_nanos:                 time_nanos(),
            cts_id:                                                 user_canister_init.cts_id,
            cbsm_id:                                                user_canister_init.cbsm_id,
            cycles_market_id:                                       user_canister_init.cycles_market_id, 
            user_id:                                                user_canister_init.user_id,
            storage_size_mib:                                       user_canister_init.storage_size_mib,
            lifetime_termination_timestamp_seconds:                 user_canister_init.lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters:                           user_canister_init.cycles_transferrer_canisters,
            user_data:                                              UserData::new(),
            cycles_transfers_id_counter:                            0u128    
        };
    });

    
    localkey::cell::set(&MEMORY_SIZE_AT_THE_START, core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES);
    
}




// ---------------------------------------------------------------------------------




/*

#[derive(CandidType, Deserialize)]
struct OldCBData {
    
}


#[derive(CandidType, Deserialize, Clone)]
struct OldUserData {
    
}

*/






fn create_cb_data_candid_bytes() -> Vec<u8> {
    with_mut(&CB_DATA, |cb_data| { 
        cb_data.user_data.cycles_transfers_in.shrink_to_fit();
        cb_data.user_data.cycles_transfers_out.shrink_to_fit(); 
        cb_data.user_data.cm_cycles_positions.shrink_to_fit();
        cb_data.user_data.cm_icp_positions.shrink_to_fit();
        cb_data.user_data.cm_cycles_positions_purchases.shrink_to_fit();
        cb_data.user_data.cm_icp_positions_purchases.shrink_to_fit();
        cb_data.user_data.cm_icp_transfers_out.shrink_to_fit();
    });
    
    let mut cb_data_candid_bytes: Vec<u8> = with(&CB_DATA, |cb_data| { encode_one(cb_data).unwrap() });
    cb_data_candid_bytes.shrink_to_fit();
    cb_data_candid_bytes
}

fn re_store_cb_data_candid_bytes(cb_data_candid_bytes: Vec<u8>) {
    
    let cb_data: CBData = match decode_one::<CBData>(&cb_data_candid_bytes) {
        Ok(cb_data) => cb_data,
        Err(_) => {
            trap("error decode of the CBData");
            /*
            let old_cb_data: OldCBData = decode_one::<OldCBData>(&cb_data_candid_bytes).unwrap();
            let cb_data: CBData = CBData{
                cts_id: old_cb_data.cts_id,
                .......
            };
            cb_data
            */
       }
    };

    std::mem::drop(cb_data_candid_bytes);

    with_mut(&CB_DATA, |ucd| {
        *ucd = cb_data;
    });

}


// ---------------------------------------------------------------------------------



#[pre_upgrade]
fn pre_upgrade() {
    let uc_upgrade_data_candid_bytes: Vec<u8> = create_cb_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*len of the uc_upgrade_data_candid_bytes*/ + uc_upgrade_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
    }
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((uc_upgrade_data_candid_bytes.len() as u64).to_be_bytes()));
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &uc_upgrade_data_candid_bytes);
}

#[post_upgrade]
fn post_upgrade() {
    
    localkey::cell::set(&MEMORY_SIZE_AT_THE_START, core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES);


    let mut uc_upgrade_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut uc_upgrade_data_candid_bytes_len_u64_be_bytes);
    let uc_upgrade_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(uc_upgrade_data_candid_bytes_len_u64_be_bytes); 
    
    let mut uc_upgrade_data_candid_bytes: Vec<u8> = vec![0; uc_upgrade_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut uc_upgrade_data_candid_bytes);
    
    re_store_cb_data_candid_bytes(uc_upgrade_data_candid_bytes);
    
}



// ---------------------------------------------------------------------------------

#[no_mangle]
async fn canister_global_timer() {
    /*
    if time_nanos()/NANOS_IN_A_SECOND > with(&CB_DATA, |cb_data| { cb_data.lifetime_termination_timestamp_seconds }) - 30/*30 seconds slippage somewhere*/ {
        // call the cts-main for the user-canister-termination
        // the CTS will save the user_id, user_canister_id, and cycles_balance for a minimum of the 10-years.
        match call::<(cts::CyclesBankLifetimeTerminationQuest,), ()>(
            cts_id(),
            "user_canister_lifetime_termination",
            (cts::CyclesBankLifetimeTerminationQuest{
                user_id: user_id(),
                cycles_balance: cycles_balance()
            },),
        ).await {
            Ok(_) => {},
            Err(call_error) => {
                set_global_timer(100); // re-try in the 100-seconds
            }
        }
    }
    */
}


// ---------------------------------------------------------------------------------

fn cts_id() -> Principal {
    with(&CB_DATA, |cb_data| { cb_data.cts_id })
}

fn user_id() -> Principal {
    with(&CB_DATA, |cb_data| { cb_data.user_id })
}

fn cycles_balance() -> Cycles {
    with(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance })
}

fn new_cycles_transfer_id(id_counter: &mut u128) -> u128 {
    let id: u128 = id_counter.clone();
    *id_counter += 1;
    id
}

// round-robin on the cycles-transferrer-canisters
fn next_cycles_transferrer_canister_round_robin() -> Option<Principal> {
    with(&CB_DATA, |cb_data| { 
        let ctcs: &Vec<Principal> = &(cb_data.cycles_transferrer_canisters);
        match ctcs.len() {
            0 => None,
            1 => Some(ctcs[0]),
            l => {
                CYCLES_TRANSFERRER_CANISTERS_ROUND_ROBIN_COUNTER.with(|ctcs_rrc| { 
                    let c_i: usize = ctcs_rrc.get();                    
                    if c_i <= l-1 {
                        let ctc_id: Principal = ctcs[c_i];
                        if c_i == l-1 { ctcs_rrc.set(0); } else { ctcs_rrc.set(c_i + 1); }
                        Some(ctc_id)
                    } else {
                        ctcs_rrc.set(1); // we check before that the len of the ctcs is at least 2 in the first match                         
                        Some(ctcs[0])
                    } 
                })
            }
        } 
    })
}
    
// compute the size of a CyclesTransferIn and of a CyclesTransferOut, check the length of both vectors, and compute the current storage usage. 
fn calculate_current_storage_usage() -> u128 {
    (
        localkey::cell::get(&MEMORY_SIZE_AT_THE_START) 
        + 
        with(&CB_DATA, |cb_data| { 
            cb_data.user_data.cycles_transfers_in.len() * ( std::mem::size_of::<CyclesTransferIn>() + 32/*for the cycles-transfer-memo-heap-size*/ )
            + 
            cb_data.user_data.cycles_transfers_out.len() * ( std::mem::size_of::<CyclesTransferOut>() + 32/*for the cycles-transfer-memo-heap-size*/ + 20/*for the possible-call-error-string-heap-size*/ )
            +
            cb_data.user_data.cm_cycles_positions.len() * std::mem::size_of::<CMCyclesPosition>()
            +
            cb_data.user_data.cm_icp_positions.len() * std::mem::size_of::<CMIcpPosition>()
            +
            cb_data.user_data.cm_cycles_positions_purchases.len() * std::mem::size_of::<CMCyclesPositionPurchase>()
            +
            cb_data.user_data.cm_icp_positions_purchases.len() * std::mem::size_of::<CMIcpPositionPurchase>()
            +
            cb_data.user_data.cm_icp_transfers_out.len() * std::mem::size_of::<CMIcpTransferOut>()
        })
    ) as u128
}

fn calculate_free_storage() -> u128 {
    ( with(&CB_DATA, |cb_data| { cb_data.storage_size_mib }) * MiB as u128 ).saturating_sub(calculate_current_storage_usage())
}


fn ctsfuel_balance() -> CTSFuel {
    canister_balance128()
    .saturating_sub(cycles_balance())
    .saturating_sub(USER_CANISTER_BACKUP_CYCLES)
    .saturating_sub(
        with(&CB_DATA, |cb_data| { 
            cb_data.lifetime_termination_timestamp_seconds.saturating_sub(time_seconds()) 
            * 
            cb_data.storage_size_mib * 2 // canister-memory-allocation in the mib
        })
        *
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES
        /
        1024/*network storage charge per MiB per second*/
    )
}

fn truncate_cycles_transfer_memo(mut cycles_transfer_memo: CyclesTransferMemo) -> CyclesTransferMemo {
    match cycles_transfer_memo {
        CyclesTransferMemo::Nat(_n) => {},
        CyclesTransferMemo::Blob(ref mut b) => {
            b.truncate(32);
            b.shrink_to_fit();
        },
         CyclesTransferMemo::Text(ref mut s) => {
            s.truncate(32);
            s.shrink_to_fit();
        }
    }
    cycles_transfer_memo
}


// ---------------------------------------------------------------------------------






#[derive(CandidType, Deserialize)]
pub struct CTSCyclesTransfer {
    memo: CyclesTransferMemo,
    original_caller: Option<Principal>
}





#[export_name = "canister_update cycles_transfer"]
pub fn cycles_transfer() { // (ct: CyclesTransfer) -> ()

    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); }

    if ctsfuel_balance() < 10_000_000_000 {
        if caller() == cts_id() {
            with_mut(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_accept128(msg_cycles_available128())); });
            reply::<()>(());
            return;            
        }
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }

    if calculate_free_storage() < std::mem::size_of::<CyclesTransferIn>() as u128 + 32 {
        if caller() == cts_id() {
            with_mut(&CB_DATA, |cb_data| { cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(msg_cycles_accept128(msg_cycles_available128())); });
            reply::<()>(());
            return;            
        }
        trap("Canister memory is full, cannot create a log of the cycles-transfer.");
    }

    if arg_data_raw_size() > 150 {
        trap("arg_data_raw_size can be max 150 bytes");
    }

    if msg_cycles_available128() < CYCLES_TRANSFER_IN_MINIMUM_CYCLES {
        trap(&format!("minimum cycles transfer cycles: {}", CYCLES_TRANSFER_IN_MINIMUM_CYCLES));
    }
        
    let cycles_cept: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    let (ct_memo, by_the_canister): (CyclesTransferMemo, Principal) = {
        if with(&CB_DATA, |cb_data| { cb_data.cycles_transferrer_canisters.contains(&caller()) }) { 
            let (ct,): (cycles_transferrer::CyclesTransfer,) = arg_data::<(cycles_transferrer::CyclesTransfer,)>();
            (ct.memo, ct.original_caller.unwrap_or(caller()))
        } else {
            let (ct,): (CyclesTransfer,) = arg_data::<(CyclesTransfer,)>();
            (ct.memo, caller())    
        }
    };
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(cycles_cept);
        cb_data.user_data.cycles_transfers_in.push(
            CyclesTransferIn{
                id: new_cycles_transfer_id(&mut cb_data.cycles_transfers_id_counter),
                by_the_canister,
                cycles: cycles_cept,
                cycles_transfer_memo: truncate_cycles_transfer_memo(ct_memo),
                timestamp_nanos: time_nanos()
            }
        );
    });
    
    reply::<()>(());
    return;
    
}




#[export_name = "canister_query download_cycles_transfers_in"]
pub fn download_cycles_transfers_in() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }    
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); }
    
    with(&CB_DATA, |cb_data| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>(); // starts at 0
        reply::<(Option<&[CyclesTransferIn]>,)>((cb_data.user_data.cycles_transfers_in.chunks(USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE).nth(chunk_i as usize),));
    });
    
}



#[update(manual_reply = true)]
pub fn delete_cycles_transfers_in(delete_cycles_transfers_in_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with_mut(&CB_DATA, |cb_data| {
        for delete_cycles_transfer_in_id in delete_cycles_transfers_in_ids.into_iter() {
            match cb_data.user_data.cycles_transfers_in.binary_search_by_key(&delete_cycles_transfer_in_id, |cycles_transfer_in| { cycles_transfer_in.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cycles_transfers_in[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cycles_transfer_in id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cycles_transfer_in_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cycles_transfers_in.remove(i);
                },
                Err(_) => {
                    trap(&format!("cycles_transfer_in id: {} not found.", delete_cycles_transfer_in_id))
                }
            }
        }
    });
    
    reply::<()>(());
}






#[derive(CandidType, Deserialize, Clone)]
pub struct UserTransferCyclesQuest {
    for_the_canister: Principal,
    cycles: Cycles,
    cycles_transfer_memo: CyclesTransferMemo
}


#[derive(CandidType, Deserialize)]
pub enum UserTransferCyclesError {
    CTSFuelTooLow,
    MemoryIsFull,
    InvalidCyclesTransferMemoSize{max_size_bytes: u128},
    InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: Cycles },
    CyclesBalanceTooLow { cycles_balance: Cycles, cycles_transferrer_transfer_cycles_fee: Cycles },
    CyclesTransferrerTransferCyclesError(cycles_transferrer::TransferCyclesError),
    CyclesTransferrerTransferCyclesCallError((u32, String))
}

#[update]
pub async fn transfer_cycles(mut q: UserTransferCyclesQuest) -> Result<u128, UserTransferCyclesError> {

    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); }
    
    if ctsfuel_balance() < 15_000_000_000 {
        return Err(UserTransferCyclesError::CTSFuelTooLow);
    }
    
    if calculate_free_storage() < std::mem::size_of::<CyclesTransferOut>() as u128 + 32 + 40 {
        return Err(UserTransferCyclesError::MemoryIsFull);
    }
    
    if q.cycles < MINIMUM_USER_TRANSFER_CYCLES {
        return Err(UserTransferCyclesError::InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: MINIMUM_USER_TRANSFER_CYCLES });
    }
    
    if q.cycles + CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE > cycles_balance() {
        return Err(UserTransferCyclesError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), cycles_transferrer_transfer_cycles_fee: CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE });
    }
    
    // check memo size
    match q.cycles_transfer_memo {
        CyclesTransferMemo::Blob(ref mut b) => {
            if b.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u128}); 
            }
            b.shrink_to_fit();
            if b.capacity() > b.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Text(ref mut s) => {
            if s.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u128}); 
            }
            s.shrink_to_fit();
            if s.capacity() > s.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Nat(_n) => {} 
    }
 
    let cycles_transfer_id: u128 = with_mut(&CB_DATA, |cb_data| {
        let cycles_transfer_id: u128 = new_cycles_transfer_id(&mut cb_data.cycles_transfers_id_counter);        
        // take the user-cycles before the transfer, and refund in the callback 
        cb_data.user_data.cycles_balance -= q.cycles + CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE;
        cb_data.user_data.cycles_transfers_out.push(
            CyclesTransferOut{
                id: cycles_transfer_id,
                for_the_canister: q.for_the_canister,
                cycles_sent: q.cycles,
                cycles_refunded: None,   // None means the cycles_transfer-call-callback did not come back yet(did not give-back a reply-or-reject-sponse) 
                cycles_transfer_memo: q.cycles_transfer_memo.clone(),
                timestamp_nanos: time_nanos(), // time sent
                opt_cycles_transfer_call_error: None,
                fee_paid: CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE as u64
            }
        );
        cycles_transfer_id
    });
    
    let q_cycles: Cycles = q.cycles; // copy cause want the value to stay on the stack for the closure to run with it. after the q is move into the candid params
    let cycles_transferrer_transfer_cycles_fee: Cycles = CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE; // copy the value to stay on the stack for the closure to run with it.
    
    let cancel_user_transfer_cycles = || {
        with_mut(&CB_DATA, |cb_data| {
            cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(q_cycles + cycles_transferrer_transfer_cycles_fee);
            
            match cb_data.user_data.cycles_transfers_out.iter().rposition(
                |cycles_transfer_out: &CyclesTransferOut| { 
                    (*cycles_transfer_out).id == cycles_transfer_id
                }
            ) {
                Some(i) => { cb_data.user_data.cycles_transfers_out.remove(i); },
                None => {}
            }
        });
    };
        
    match call_with_payment128::<(cycles_transferrer::TransferCyclesQuest,), (Result<(), cycles_transferrer::TransferCyclesError>,)>(
        next_cycles_transferrer_canister_round_robin().expect("0 known cycles transferrer canisters.")/*before the first await*/,
        "transfer_cycles",
        (cycles_transferrer::TransferCyclesQuest{
            user_cycles_transfer_id: cycles_transfer_id,
            for_the_canister: q.for_the_canister,
            cycles: q.cycles,
            cycles_transfer_memo: q.cycles_transfer_memo
        },),
        q.cycles + cycles_transferrer_transfer_cycles_fee
    ).await { // it is possible that this callback will be called after the cycles_transferrer calls the cycles_transferrer_user_transfer_cycles_callback
        Ok((cycles_transferrer_transfer_cycles_sponse,)) => match cycles_transferrer_transfer_cycles_sponse {
            Ok(()) => return Ok(cycles_transfer_id), // Ok here means the cycles-transfer call will either be delivered, returned because the destination canister does not exist or returned because of an out of cycles condition.
            Err(cycles_transferrer_transfer_cycles_error) => {
                cancel_user_transfer_cycles();
                return Err(UserTransferCyclesError::CyclesTransferrerTransferCyclesError(cycles_transferrer_transfer_cycles_error));
            }
        }, 
        Err(cycles_transferrer_transfer_cycles_call_error) => {
            cancel_user_transfer_cycles();
            return Err(UserTransferCyclesError::CyclesTransferrerTransferCyclesCallError((cycles_transferrer_transfer_cycles_call_error.0 as u32, cycles_transferrer_transfer_cycles_call_error.1)));
        }
    }
    
}



// :lack of the check of the ctsfuel-balance here, cause of the check in the user_transfer_cycles-method. set on the side the ctsfuel for the callback?

#[update]
pub fn cycles_transferrer_transfer_cycles_callback(q: cycles_transferrer::TransferCyclesCallbackQuest) -> () {
    
    if with(&CB_DATA, |cb_data| { cb_data.cycles_transferrer_canisters.contains(&caller()) }) == false {
        trap("caller must be one of the CTS-cycles-transferrer-canisters for this method.");
    }
    
    //if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); } // make sure that when set a stop-call-flag, there are 0 ongoing-$cycles-transfers. cycles-transfer-callback errors will hold for
    
    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128()); 

    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(cycles_transfer_refund);
        if let Some(cycles_transfer_out/*: &mut CyclesTransferOut*/) = cb_data.user_data.cycles_transfers_out.iter_mut().rev().find(|cycles_transfer_out: &&mut CyclesTransferOut| { (**cycles_transfer_out).id == q.user_cycles_transfer_id }) {
            cycles_transfer_out.cycles_refunded = Some(cycles_transfer_refund);
            cycles_transfer_out.opt_cycles_transfer_call_error = q.opt_cycles_transfer_call_error;
        }
    });

}







#[export_name = "canister_query download_cycles_transfers_out"]
pub fn download_cycles_transfers_out() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with(&CB_DATA, |cb_data| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>(); // starts at 0
        reply::<(Option<&[CyclesTransferOut]>,)>((cb_data.user_data.cycles_transfers_out.chunks(USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}





#[update(manual_reply = true)]
pub fn delete_cycles_transfers_out(delete_cycles_transfers_out_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with_mut(&CB_DATA, |cb_data| {
        for delete_cycles_transfer_out_id in delete_cycles_transfers_out_ids.into_iter() {
            match cb_data.user_data.cycles_transfers_out.binary_search_by_key(&delete_cycles_transfer_out_id, |cycles_transfer_out| { cycles_transfer_out.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cycles_transfers_out[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cycles_transfer_out id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cycles_transfer_out_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cycles_transfers_out.remove(i);
                },
                Err(_) => {
                    trap(&format!("cycles_transfer_out id: {} not found.", delete_cycles_transfer_out_id))
                }
            }
        }
    });
    
    reply::<()>(());
}





// ---------------------------------------------------
// cycles-market methods



#[derive(CandidType, Deserialize)]
pub enum UserCMCreateCyclesPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_create_position_fee: Cycles },
    CyclesMarketCreateCyclesPositionCallError((u32, String)),
    CyclesMarketCreateCyclesPositionError(cycles_market::CreateCyclesPositionError)
}


#[update]
pub async fn cm_create_cycles_position(q: cycles_market::CreateCyclesPositionQuest) -> Result<cycles_market::CreateCyclesPositionSuccess, UserCMCreateCyclesPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 30_000_000_000 {
        return Err(UserCMCreateCyclesPositionError::CTSFuelTooLow);
    }
    
    if calculate_free_storage() < std::mem::size_of::<CMCyclesPosition>() as u128 {
        return Err(UserCMCreateCyclesPositionError::MemoryIsFull);
    }
   
    if cycles_balance() < q.cycles + CYCLES_MARKET_CREATE_POSITION_FEE {
        return Err(UserCMCreateCyclesPositionError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), cycles_market_create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE });
    }

    match call_with_payment128::<(&cycles_market::CreateCyclesPositionQuest,), (Result<cycles_market::CreateCyclesPositionSuccess, cycles_market::CreateCyclesPositionError>,)>(
        with(&CB_DATA, |cb_data| { cb_data.cycles_market_id }),
        "create_cycles_position",
        (&q/*.clone()*/,),
        q.cycles + CYCLES_MARKET_CREATE_POSITION_FEE
    ).await {
        Ok((cm_create_cycles_position_result,)) => match cm_create_cycles_position_result {
            Ok(cm_create_cycles_position_success) => {
                with_mut(&CB_DATA, |cb_data| {
                    cb_data.user_data.cm_cycles_positions.push(
                        CMCyclesPosition{
                            id: cm_create_cycles_position_success.position_id,   
                            cycles: q.cycles,
                            minimum_purchase: q.minimum_purchase,
                            xdr_permyriad_per_icp_rate: q.xdr_permyriad_per_icp_rate,
                            create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE as u64,
                            timestamp_nanos: time_nanos(),
                        }
                    );
                });
                Ok(cm_create_cycles_position_success)
            },
            Err(cm_create_cycles_position_error) => {
                return Err(UserCMCreateCyclesPositionError::CyclesMarketCreateCyclesPositionError(cm_create_cycles_position_error));
            }
        },
        Err(call_error) => {
            return Err(UserCMCreateCyclesPositionError::CyclesMarketCreateCyclesPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}



// ------------


#[derive(CandidType, Deserialize)]
pub enum UserCMCreateIcpPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_create_position_fee: Cycles },
    CyclesMarketCreateIcpPositionCallError((u32, String)),
    CyclesMarketCreateIcpPositionError(cycles_market::CreateIcpPositionError)
}


#[update]
pub async fn cm_create_icp_position(q: cycles_market::CreateIcpPositionQuest) -> Result<cycles_market::CreateIcpPositionSuccess, UserCMCreateIcpPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 30_000_000_000 {
        return Err(UserCMCreateIcpPositionError::CTSFuelTooLow);
    }
    
    if calculate_free_storage() < std::mem::size_of::<CMIcpPosition>() as u128 {
        return Err(UserCMCreateIcpPositionError::MemoryIsFull);
    }
   
    if cycles_balance() < CYCLES_MARKET_CREATE_POSITION_FEE {
        return Err(UserCMCreateIcpPositionError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), cycles_market_create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE });
    }

    match call_with_payment128::<(&cycles_market::CreateIcpPositionQuest,), (Result<cycles_market::CreateIcpPositionSuccess, cycles_market::CreateIcpPositionError>,)>(
        with(&CB_DATA, |cb_data| { cb_data.cycles_market_id }),
        "create_icp_position",
        (&q,),
        CYCLES_MARKET_CREATE_POSITION_FEE
    ).await {
        Ok((cm_create_icp_position_result,)) => match cm_create_icp_position_result {
            Ok(cm_create_icp_position_success) => {
                with_mut(&CB_DATA, |cb_data| {
                    cb_data.user_data.cm_icp_positions.push(
                        CMIcpPosition{
                            id: cm_create_icp_position_success.position_id,   
                            icp: q.icp,
                            minimum_purchase: q.minimum_purchase,
                            xdr_permyriad_per_icp_rate: q.xdr_permyriad_per_icp_rate,
                            create_position_fee: CYCLES_MARKET_CREATE_POSITION_FEE as u64,
                            timestamp_nanos: time_nanos(),
                        }
                    );
                });
                Ok(cm_create_icp_position_success)
            },
            Err(cm_create_icp_position_error) => {
                return Err(UserCMCreateIcpPositionError::CyclesMarketCreateIcpPositionError(cm_create_icp_position_error));
            }
        },
        Err(call_error) => {
            return Err(UserCMCreateIcpPositionError::CyclesMarketCreateIcpPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}



// --------------

#[derive(CandidType, Deserialize)]
pub struct UserCMPurchaseCyclesPositionQuest {
    cycles_market_purchase_cycles_position_quest: cycles_market::PurchaseCyclesPositionQuest,
    cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp // for the user_canister-log
}

#[derive(CandidType, Deserialize)]
pub enum UserCMPurchaseCyclesPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_purchase_position_fee: Cycles },
    CyclesMarketPurchaseCyclesPositionCallError((u32, String)),
    CyclesMarketPurchaseCyclesPositionError(cycles_market::PurchaseCyclesPositionError)
}


#[update]
pub async fn cm_purchase_cycles_position(q: UserCMPurchaseCyclesPositionQuest) -> Result<cycles_market::PurchaseCyclesPositionSuccess, UserCMPurchaseCyclesPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 30_000_000_000 {
        return Err(UserCMPurchaseCyclesPositionError::CTSFuelTooLow);
    }
    
    if calculate_free_storage() < std::mem::size_of::<CMCyclesPositionPurchase>() as u128 {
        return Err(UserCMPurchaseCyclesPositionError::MemoryIsFull);
    }
    
    if cycles_balance() < CYCLES_MARKET_PURCHASE_POSITION_FEE {
        return Err(UserCMPurchaseCyclesPositionError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), cycles_market_purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE });
    }

    match call_with_payment128::<(&cycles_market::PurchaseCyclesPositionQuest,), (Result<cycles_market::PurchaseCyclesPositionSuccess, cycles_market::PurchaseCyclesPositionError>,)>(
        with(&CB_DATA, |cb_data| { cb_data.cycles_market_id }),
        "purchase_cycles_position",
        (&q.cycles_market_purchase_cycles_position_quest,),
        CYCLES_MARKET_PURCHASE_POSITION_FEE
    ).await {
        Ok((cm_purchase_cycles_position_result,)) => match cm_purchase_cycles_position_result {
            Ok(cm_purchase_cycles_position_success) => {
                with_mut(&CB_DATA, |cb_data| {
                    cb_data.user_data.cm_cycles_positions_purchases.push(
                        CMCyclesPositionPurchase{
                            cycles_position_id: q.cycles_market_purchase_cycles_position_quest.cycles_position_id,
                            cycles_position_xdr_permyriad_per_icp_rate: q.cycles_position_xdr_permyriad_per_icp_rate,
                            id: cm_purchase_cycles_position_success.purchase_id,
                            cycles: q.cycles_market_purchase_cycles_position_quest.cycles,
                            purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE as u64,
                            timestamp_nanos: time_nanos(),
                        }
                    );
                });
                Ok(cm_purchase_cycles_position_success)
            },
            Err(cm_purchase_cycles_position_error) => {
                return Err(UserCMPurchaseCyclesPositionError::CyclesMarketPurchaseCyclesPositionError(cm_purchase_cycles_position_error));
            }
        },
        Err(call_error) => {
            return Err(UserCMPurchaseCyclesPositionError::CyclesMarketPurchaseCyclesPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}


// ---------------

#[derive(CandidType, Deserialize)]
pub struct UserCMPurchaseIcpPositionQuest {
    cycles_market_purchase_icp_position_quest: cycles_market::PurchaseIcpPositionQuest,
    icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp // for the user_canister-log
}

#[derive(CandidType, Deserialize)]
pub enum UserCMPurchaseIcpPositionError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_purchase_position_fee: Cycles },
    CyclesMarketPurchaseIcpPositionCallError((u32, String)),
    CyclesMarketPurchaseIcpPositionError(cycles_market::PurchaseIcpPositionError)
}


#[update]
pub async fn cm_purchase_icp_position(q: UserCMPurchaseIcpPositionQuest) -> Result<cycles_market::PurchaseIcpPositionSuccess, UserCMPurchaseIcpPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 30_000_000_000 {
        return Err(UserCMPurchaseIcpPositionError::CTSFuelTooLow);
    }
    
    if calculate_free_storage() < std::mem::size_of::<CMIcpPositionPurchase>() as u128 {
        return Err(UserCMPurchaseIcpPositionError::MemoryIsFull);
    }
    
    if cycles_balance() < CYCLES_MARKET_PURCHASE_POSITION_FEE + icptokens_to_cycles(q.cycles_market_purchase_icp_position_quest.icp, q.icp_position_xdr_permyriad_per_icp_rate) {
        return Err(UserCMPurchaseIcpPositionError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), cycles_market_purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE });
    }

    match call_with_payment128::<(&cycles_market::PurchaseIcpPositionQuest,), (Result<cycles_market::PurchaseIcpPositionSuccess, cycles_market::PurchaseIcpPositionError>,)>(
        with(&CB_DATA, |cb_data| { cb_data.cycles_market_id }),
        "purchase_icp_position",
        (&q.cycles_market_purchase_icp_position_quest,),
        CYCLES_MARKET_PURCHASE_POSITION_FEE + icptokens_to_cycles(q.cycles_market_purchase_icp_position_quest.icp, q.icp_position_xdr_permyriad_per_icp_rate)
    ).await {
        Ok((cm_purchase_icp_position_result,)) => match cm_purchase_icp_position_result {
            Ok(cm_purchase_icp_position_success) => {
                with_mut(&CB_DATA, |cb_data| {
                    cb_data.user_data.cm_icp_positions_purchases.push(
                        CMIcpPositionPurchase{
                            icp_position_id: q.cycles_market_purchase_icp_position_quest.icp_position_id,
                            icp_position_xdr_permyriad_per_icp_rate: q.icp_position_xdr_permyriad_per_icp_rate,
                            id: cm_purchase_icp_position_success.purchase_id,
                            icp: q.cycles_market_purchase_icp_position_quest.icp,
                            purchase_position_fee: CYCLES_MARKET_PURCHASE_POSITION_FEE as u64,
                            timestamp_nanos: time_nanos(),
                        }
                    );
                });
                Ok(cm_purchase_icp_position_success)
            },
            Err(cm_purchase_icp_position_error) => {
                return Err(UserCMPurchaseIcpPositionError::CyclesMarketPurchaseIcpPositionError(cm_purchase_icp_position_error));
            }
        },
        Err(call_error) => {
            return Err(UserCMPurchaseIcpPositionError::CyclesMarketPurchaseIcpPositionCallError((call_error.0 as u32, call_error.1)));
        }
    }

}


// ---------------------

#[derive(CandidType, Deserialize)]
pub enum UserCMVoidPositionError {
    CTSFuelTooLow,
    CyclesMarketVoidPositionCallError((u32, String)),
    CyclesMarketVoidPositionError(cycles_market::VoidPositionError)
}


#[update]
pub async fn cm_void_position(q: cycles_market::VoidPositionQuest) -> Result<(), UserCMVoidPositionError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 30_000_000_000 {
        return Err(UserCMVoidPositionError::CTSFuelTooLow);
    }
    
    match call::<(cycles_market::VoidPositionQuest,), (Result<(), cycles_market::VoidPositionError>,)>(
        with(&CB_DATA, |cb_data| { cb_data.cycles_market_id }),
        "void_position",
        (q,)
    ).await {
        Ok((cm_void_position_result,)) => match cm_void_position_result {
            Ok(()) => Ok(()),
            Err(cm_void_position_error) => {
                return Err(UserCMVoidPositionError::CyclesMarketVoidPositionError(cm_void_position_error));
            }
        },
        Err(call_error) => {
            return Err(UserCMVoidPositionError::CyclesMarketVoidPositionCallError((call_error.0 as u32, call_error.1)));
        }
    
    }
    
}


// -------

#[derive(CandidType, Deserialize)]
pub enum UserCMSeeIcpLockError {
    CTSFuelTooLow,
    CyclesMarketSeeIcpLockCallError((u32, String)),
}

#[update]
pub async fn cm_see_icp_lock() -> Result<IcpTokens, UserCMSeeIcpLockError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 30_000_000_000 {
        return Err(UserCMSeeIcpLockError::CTSFuelTooLow);
    }
    
    match call::<(), (IcpTokens,)>(
        with(&CB_DATA, |cb_data| { cb_data.cycles_market_id }),
        "see_icp_lock",
        ()
    ).await {
        Ok((icp_tokens,)) => Ok(icp_tokens),
        Err(call_error) => {
            return Err(UserCMSeeIcpLockError::CyclesMarketSeeIcpLockCallError((call_error.0 as u32, call_error.1)));
        }
    }
} 


// -------

#[derive(CandidType, Deserialize)]
pub enum UserCMTransferIcpBalanceError {
    CTSFuelTooLow,
    MemoryIsFull,
    CyclesBalanceTooLow{ cycles_balance: Cycles, cycles_market_transfer_icp_balance_fee: Cycles },
    CyclesMarketTransferIcpBalanceCallError((u32, String)),
    CyclesMarketTransferIcpBalanceError(cycles_market::TransferIcpBalanceError)
}

#[update]
pub async fn cm_transfer_icp_balance(q: cycles_market::TransferIcpBalanceQuest) -> Result<IcpBlockHeight, UserCMTransferIcpBalanceError> {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 30_000_000_000 {
        return Err(UserCMTransferIcpBalanceError::CTSFuelTooLow);
    }

    if calculate_free_storage() < std::mem::size_of::<CMIcpTransferOut>() as u128 {
        return Err(UserCMTransferIcpBalanceError::MemoryIsFull);
    }

    if cycles_balance() < CYCLES_MARKET_TRANSFER_ICP_BALANCE_FEE {
        return Err(UserCMTransferIcpBalanceError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), cycles_market_transfer_icp_balance_fee: CYCLES_MARKET_TRANSFER_ICP_BALANCE_FEE });
    }

    match call_with_payment128::<(&cycles_market::TransferIcpBalanceQuest,), (Result<IcpBlockHeight, cycles_market::TransferIcpBalanceError>,)>(
        with(&CB_DATA, |cb_data| { cb_data.cycles_market_id }),
        "transfer_icp_balance",
        (&q,),
        CYCLES_MARKET_TRANSFER_ICP_BALANCE_FEE
    ).await {
        Ok((cm_transfer_icp_balance_result,)) => match cm_transfer_icp_balance_result {
            Ok(block_height) => {
                with_mut(&CB_DATA, |cb_data| {
                    cb_data.user_data.cm_icp_transfers_out.push(
                        CMIcpTransferOut{
                            icp: q.icp,
                            icp_fee: q.icp_fee,
                            to: q.to,
                            block_height: block_height as u128,
                            timestamp_nanos: time_nanos(),
                            transfer_icp_balance_fee: CYCLES_MARKET_TRANSFER_ICP_BALANCE_FEE as u64
                        }
                    );
                });
                Ok(block_height)
            },
            Err(cm_transfer_icp_balance_error) => {
                return Err(UserCMTransferIcpBalanceError::CyclesMarketTransferIcpBalanceError(cm_transfer_icp_balance_error));
            }
        },
        Err(call_error) => {
            return Err(UserCMTransferIcpBalanceError::CyclesMarketTransferIcpBalanceCallError((call_error.0 as u32, call_error.1)));
        }
    }

}



// ----------



#[query(manual_reply = true)]
pub fn download_cm_cycles_positions() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with(&CB_DATA, |cb_data| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>(); // starts at 0
        reply::<(Option<&[CMCyclesPosition]>,)>((cb_data.user_data.cm_cycles_positions.chunks(USER_DOWNLOAD_CM_CYCLES_POSITIONS_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}


#[query(manual_reply = true)]
pub fn download_cm_icp_positions() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with(&CB_DATA, |cb_data| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>(); // starts at 0
        reply::<(Option<&[CMIcpPosition]>,)>((cb_data.user_data.cm_icp_positions.chunks(USER_DOWNLOAD_CM_ICP_POSITIONS_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}


#[query(manual_reply = true)]
pub fn download_cm_cycles_positions_purchases() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with(&CB_DATA, |cb_data| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>(); // starts at 0
        reply::<(Option<&[CMCyclesPositionPurchase]>,)>((cb_data.user_data.cm_cycles_positions_purchases.chunks(USER_DOWNLOAD_CM_CYCLES_POSITIONS_PURCHASES_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}



#[query(manual_reply = true)]
pub fn download_cm_icp_positions_purchases() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with(&CB_DATA, |cb_data| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>(); // starts at 0
        reply::<(Option<&[CMIcpPositionPurchase]>,)>((cb_data.user_data.cm_icp_positions_purchases.chunks(USER_DOWNLOAD_CM_ICP_POSITIONS_PURCHASES_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}


#[query(manual_reply = true)]
pub fn download_cm_icp_transfers_out() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with(&CB_DATA, |cb_data| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>();                                   // starts at 0
        reply::<(Option<&[CMIcpTransferOut]>,)>((cb_data.user_data.cm_icp_transfers_out.chunks(USER_DOWNLOAD_CM_ICP_TRANSFERS_OUT_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}



// -----------------------------



#[update(manual_reply = true)]
pub fn delete_cm_cycles_positions(delete_cm_cycles_positions_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cm_cycles_positions.sort_by_key(|cm_cycles_position| { cm_cycles_position.id });
        for delete_cm_cycles_position_id in delete_cm_cycles_positions_ids.into_iter() {
            match cb_data.user_data.cm_cycles_positions.binary_search_by_key(&delete_cm_cycles_position_id, |cm_cycles_position| { cm_cycles_position.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cm_cycles_positions[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cm_cycles_position id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cm_cycles_position_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cm_cycles_positions.remove(i);
                },
                Err(_) => {
                    trap(&format!("cm_cycles_position id: {} not found.", delete_cm_cycles_position_id))
                }
            }
        }
    });
    
    reply::<()>(());
}




#[update(manual_reply = true)]
pub fn delete_cm_icp_positions(delete_cm_icp_positions_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cm_icp_positions.sort_by_key(|cm_icp_position| { cm_icp_position.id });
        for delete_cm_icp_position_id in delete_cm_icp_positions_ids.into_iter() {
            match cb_data.user_data.cm_icp_positions.binary_search_by_key(&delete_cm_icp_position_id, |cm_icp_position| { cm_icp_position.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cm_icp_positions[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cm_icp_position id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cm_icp_position_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cm_icp_positions.remove(i);
                },
                Err(_) => {
                    trap(&format!("cm_icp_position id: {} not found.", delete_cm_icp_position_id))
                }
            }
        }
    });
    
    reply::<()>(());
}





#[update(manual_reply = true)]
pub fn delete_cm_cycles_positions_purchases(delete_cm_cycles_positions_purchases_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cm_cycles_positions_purchases.sort_by_key(|cm_cycles_position_purchase| { cm_cycles_position_purchase.id });
        for delete_cm_cycles_position_purchase_id in delete_cm_cycles_positions_purchases_ids.into_iter() {
            match cb_data.user_data.cm_cycles_positions_purchases.binary_search_by_key(&delete_cm_cycles_position_purchase_id, |cm_cycles_position_purchase| { cm_cycles_position_purchase.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cm_cycles_positions_purchases[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cm_cycles_position_purchase id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cm_cycles_position_purchase_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cm_cycles_positions_purchases.remove(i);
                },
                Err(_) => {
                    trap(&format!("cm_cycles_position_purchase id: {} not found.", delete_cm_cycles_position_purchase_id))
                }
            }
        }
    });
    
    reply::<()>(());
}





#[update(manual_reply = true)]
pub fn delete_cm_icp_positions_purchases(delete_cm_icp_positions_purchases_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cm_icp_positions_purchases.sort_by_key(|cm_icp_position_purchase| { cm_icp_position_purchase.id });
        for delete_cm_icp_position_purchase_id in delete_cm_icp_positions_purchases_ids.into_iter() {
            match cb_data.user_data.cm_icp_positions_purchases.binary_search_by_key(&delete_cm_icp_position_purchase_id, |cm_icp_position_purchase| { cm_icp_position_purchase.id }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cm_icp_positions_purchases[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cm_icp_position_purchase id: {} is too new to delete. must be at least {} days in the past to delete.", delete_cm_icp_position_purchase_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cm_icp_positions_purchases.remove(i);
                },
                Err(_) => {
                    trap(&format!("cm_icp_position_purchase id: {} not found.", delete_cm_icp_position_purchase_id))
                }
            }
        }
    });
    
    reply::<()>(());
}



#[update(manual_reply = true)]
pub fn delete_cm_icp_transfers_out(delete_cm_icp_transfers_out_ids: Vec<u128>) {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cm_icp_transfers_out.sort_by_key(|cm_icp_transfer_out| { cm_icp_transfer_out.block_height });
        for delete_cm_icp_transfer_out_id in delete_cm_icp_transfers_out_ids.into_iter() {
            match cb_data.user_data.cm_icp_transfers_out.binary_search_by_key(&delete_cm_icp_transfer_out_id, |cm_icp_transfer_out| { cm_icp_transfer_out.block_height }) {
                Ok(i) => {
                    if time_nanos().saturating_sub(cb_data.user_data.cm_icp_transfers_out[i].timestamp_nanos) < DELETE_LOG_MINIMUM_WAIT_NANOS {
                        trap(&format!("cm_icp_transfer_out block_height: {} is too new to delete. must be at least {} days in the past to delete.", delete_cm_icp_transfer_out_id, DELETE_LOG_MINIMUM_WAIT_NANOS/NANOS_IN_A_SECOND/SECONDS_IN_A_DAY));
                    }
                    cb_data.user_data.cm_icp_transfers_out.remove(i);
                },
                Err(_) => {
                    trap(&format!("cm_icp_transfer_out block_height: {} not found.", delete_cm_icp_transfer_out_id))
                }
            }
        }
    });
    
    reply::<()>(());
}






// ----------------------------------------------------------


/*
#[export_name = "canister_query cycles_balance"]
pub fn cycles_balance_canister_method() {  // -> Cycles
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    reply::<(Cycles,)>((cycles_balance(),));
    return;
}
*/




#[derive(CandidType, Deserialize)]
pub struct UserUCMetrics {
    cycles_balance: Cycles,
    user_canister_ctsfuel_balance: CTSFuel,
    storage_size_mib: u128,
    lifetime_termination_timestamp_seconds: u128,
    cycles_transferrer_canisters: Vec<Principal>,
    user_id: Principal,
    user_canister_creation_timestamp_nanos: u128,
    storage_usage: u128,
    cycles_transfers_id_counter: u128,
    cycles_transfers_in_len: u128,
    cycles_transfers_out_len: u128,
    download_cycles_transfers_in_chunk_size: u128,
    download_cycles_transfers_out_chunk_size: u128,
    cm_cycles_positions_len: u128,
    cm_icp_positions_len: u128,
    cm_cycles_positions_purchases_len: u128,
    cm_icp_positions_purchases_len: u128,
    cm_icp_transfers_out_len: u128,
    download_cm_cycles_positions_chunk_size: u128,
    download_cm_icp_positions_chunk_size: u128,
    download_cm_cycles_positions_purchases_chunk_size: u128,
    download_cm_icp_positions_purchases_chunk_size: u128,
    download_cm_icp_transfers_out_chunk_size: u128,
}


#[query]
pub fn metrics() -> UserUCMetrics {
    if caller() != user_id() && caller() != cts_id() {
        trap("Caller must be the user for this method.");
    }
    
    with(&CB_DATA, |cb_data| {
        UserUCMetrics{
            cycles_balance: cb_data.user_data.cycles_balance,
            user_canister_ctsfuel_balance: ctsfuel_balance(),
            storage_size_mib: cb_data.storage_size_mib,
            lifetime_termination_timestamp_seconds: cb_data.lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters: cb_data.cycles_transferrer_canisters.clone(),
            user_id: cb_data.user_id,
            user_canister_creation_timestamp_nanos: cb_data.user_canister_creation_timestamp_nanos,
            storage_usage: calculate_current_storage_usage(),
            cycles_transfers_id_counter: cb_data.cycles_transfers_id_counter,
            cycles_transfers_in_len: cb_data.user_data.cycles_transfers_in.len() as u128,
            cycles_transfers_out_len: cb_data.user_data.cycles_transfers_out.len() as u128,
            download_cycles_transfers_in_chunk_size: USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE as u128,
            download_cycles_transfers_out_chunk_size: USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE as u128,
            cm_cycles_positions_len: cb_data.user_data.cm_cycles_positions.len() as u128,
            cm_icp_positions_len: cb_data.user_data.cm_icp_positions.len() as u128,
            cm_cycles_positions_purchases_len: cb_data.user_data.cm_cycles_positions_purchases.len() as u128,
            cm_icp_positions_purchases_len: cb_data.user_data.cm_icp_positions_purchases.len() as u128,
            cm_icp_transfers_out_len: cb_data.user_data.cm_icp_transfers_out.len() as u128,
            download_cm_cycles_positions_chunk_size: USER_DOWNLOAD_CM_CYCLES_POSITIONS_CHUNK_SIZE as u128,
            download_cm_icp_positions_chunk_size: USER_DOWNLOAD_CM_ICP_POSITIONS_CHUNK_SIZE as u128,
            download_cm_cycles_positions_purchases_chunk_size: USER_DOWNLOAD_CM_CYCLES_POSITIONS_PURCHASES_CHUNK_SIZE as u128,
            download_cm_icp_positions_purchases_chunk_size: USER_DOWNLOAD_CM_ICP_POSITIONS_PURCHASES_CHUNK_SIZE as u128,
            download_cm_icp_transfers_out_chunk_size: USER_DOWNLOAD_CM_ICP_TRANSFERS_OUT_CHUNK_SIZE as u128,
        }
    })
}


// --------------------------------------------------------

// method on the cts-main for the ctsfuel-topup of a user-canister using icp - ledger-topup-cycles for the user-canister


#[update]
pub fn user_topup_ctsfuel_with_some_cycles() -> () {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    msg_cycles_accept128(msg_cycles_available128());
}


#[derive(CandidType, Deserialize)]
pub enum UserCyclesBalanceForTheCTSFuelError {
    CyclesBalanceTooLow { cycles_balance: Cycles }
}

#[update]
pub fn cycles_balance_for_the_ctsfuel(cycles_for_the_ctsfuel: Cycles) -> Result<(), UserCyclesBalanceForTheCTSFuelError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance, try soon."); }
    
    if cycles_balance() < cycles_for_the_ctsfuel {
        return Err(UserCyclesBalanceForTheCTSFuelError::CyclesBalanceTooLow{ cycles_balance: cycles_balance() });        
    } 
    
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance -= cycles_for_the_ctsfuel;
        // cycles-transfer-out log? what if storage is full and ctsfuel is empty?
    });
    
    Ok(())
}



// ---------------------------------------------



#[derive(CandidType, Deserialize)]
pub enum LengthenLifetimeError {
    MinimumSetLifetimeTerminationTimestampSeconds(u128),
    CyclesBalanceTooLow{ cycles_balance: Cycles, lengthen_cost_cycles: Cycles },
    CBSMCallError((u32, String))
}

#[update]
pub async fn lengthen_lifetime(q: LengthenLifetimeQuest) -> Result<u128/*new-lifetime-termination-timestamp-seconds*/, LengthenLifetimeError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance, try soon."); }

    let lengthen_cost_cycles: Cycles = {
        q.set_lifetime_termination_timestamp_seconds - with(&CB_DATA, |cb_data| { cb_data.lifetime_termination_timestamp_seconds })
        * with(&CB_DATA, |cb_data| { cb_data.storage_size_mib }) * 2 // canister-memory-allocation in the mib 
        * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES / 1024/*network storage charge per MiB per second*/
    };
    
    if lengthen_cost_cycles > cycles_balance() {
        return Err(LengthenLifetimeError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), lengthen_cost_cycles });
    }
    
    let minimum_set_lifetime_termination_timestamp_seconds: u128 = with(&CB_DATA, |cb_data| { cb_data.lifetime_termination_timestamp_seconds }).checked_add(MINIMUM_LENGTHEN_LIFETIME_SECONDS).unwrap_or_else(|| { trap("time is not support at the moment") });
    if q.set_lifetime_termination_timestamp_seconds <  minimum_set_lifetime_termination_timestamp_seconds {
        return Err(LengthenLifetimeError::MinimumSetLifetimeTerminationTimestampSeconds(minimum_set_lifetime_termination_timestamp_seconds));
    }
    
    // let id for the log
    with_mut(&CB_DATA, |cb_data| {    
        cb_data.user_data.cycles_balance -= lengthen_cost_cycles; 
        // make a log with the id
    });
    
    match call::<(&LengthenLifetimeQuest,),()>(
        with(&CB_DATA, |cb_data| { cb_data.cbsm_id }),
        "cb_lengthen_lifetime",
        (&q,),
    ).await {
        Ok(()) => {
            with_mut(&CB_DATA, |cb_data| {    
                cb_data.lifetime_termination_timestamp_seconds = q.set_lifetime_termination_timestamp_seconds;
                Ok(cb_data.lifetime_termination_timestamp_seconds)
            })                    
        },
        Err(call_error) => {
            with_mut(&CB_DATA, |cb_data| {    
                cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(lengthen_cost_cycles); 
                // delete the log with the id
            });
            return Err(LengthenLifetimeError::CBSMCallError((call_error.0 as u32, call_error.1)));
        }
    }
    
}



// ---------------------------

#[derive(CandidType, Deserialize)]
pub struct UserChangeStorageSizeMibQuest {
    new_storage_size_mib: u128
}

#[derive(CandidType, Deserialize)]
pub enum UserChangeStorageSizeMibError {
    NewStorageSizeMibTooLow{ minimum_new_storage_size_mib: u128 },
    CyclesBalanceTooLow{ cycles_balance: Cycles, new_storage_size_mib_cost_cycles: Cycles },
    ManagementCanisterUpdateSettingsCallError((u32, String))
}

#[update]
pub async fn user_change_storage_size_mib(q: UserChangeStorageSizeMibQuest) -> Result<(), UserChangeStorageSizeMibError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    let minimum_new_storage_size_mib: u128 = with(&CB_DATA, |cb_data| { cb_data.storage_size_mib }) + 10; 
    
    if minimum_new_storage_size_mib > q.new_storage_size_mib {
        return Err(UserChangeStorageSizeMibError::NewStorageSizeMibTooLow{ minimum_new_storage_size_mib }); 
    };
    
    let new_storage_size_mib_cost_cycles: Cycles = {
        ( q.new_storage_size_mib - with(&CB_DATA, |cb_data| { cb_data.storage_size_mib }) ) * 2 // grow canister-memory-allocation in the mib 
        * with(&CB_DATA, |cb_data| { cb_data.lifetime_termination_timestamp_seconds }).checked_sub(time_seconds()).unwrap_or_else(|| { trap("user-contract-lifetime is with the termination.") })
        * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES / 1024 /*network storage charge per MiB per second*/
    };
    
    if cycles_balance() < new_storage_size_mib_cost_cycles {
        return Err(UserChangeStorageSizeMibError::CyclesBalanceTooLow{ cycles_balance: cycles_balance(), new_storage_size_mib_cost_cycles });
    }

    // take the cycles before the .await and if error after here, refund the cycles
    with_mut(&CB_DATA, |cb_data| {
        cb_data.user_data.cycles_balance -= new_storage_size_mib_cost_cycles; 
    });

    match call::<(management_canister::ChangeCanisterSettingsRecord,), ()>(
        MANAGEMENT_CANISTER_ID,
        "update_settings",
        (management_canister::ChangeCanisterSettingsRecord{
            canister_id: ic_cdk::api::id(),
            settings: management_canister::ManagementCanisterOptionalCanisterSettings{
                controllers : None,
                compute_allocation : None,
                memory_allocation : Some((q.new_storage_size_mib * 2 * MiB as u128).into()),
                freezing_threshold : None,
            }
        },)
    ).await {
        Ok(()) => {
            with_mut(&CB_DATA, |cb_data| {
                cb_data.storage_size_mib = q.new_storage_size_mib;
            });
            Ok(())
        },
        Err(call_error) => {
            with_mut(&CB_DATA, |cb_data| {
                cb_data.user_data.cycles_balance = cb_data.user_data.cycles_balance.saturating_add(new_storage_size_mib_cost_cycles); 
            });
            return Err(UserChangeStorageSizeMibError::ManagementCanisterUpdateSettingsCallError((call_error.0 as u32, call_error.1)));
        }
    }


}




// -----------------------------------------------------------------------------------







#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::get(&STOP_CALLS)
}





#[update]
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot_candid_bytes*/ {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT_CB_DATA_CANDID_BYTES, |state_snapshot_cb_data_candid_bytes| {
        *state_snapshot_cb_data_candid_bytes = create_cb_data_candid_bytes();
        state_snapshot_cb_data_candid_bytes.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT_CB_DATA_CANDID_BYTES, |state_snapshot_cb_data_candid_bytes| {
        let (chunk_i,): (u128,) = arg_data::<(u128,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_cb_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    });
}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT_CB_DATA_CANDID_BYTES, |state_snapshot_cb_data_candid_bytes| {
        *state_snapshot_cb_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT_CB_DATA_CANDID_BYTES, |state_snapshot_cb_data_candid_bytes| {
        state_snapshot_cb_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_cb_data_out_of_the_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    re_store_cb_data_candid_bytes(
        with_mut(&STATE_SNAPSHOT_CB_DATA_CANDID_BYTES, |state_snapshot_cb_data_candid_bytes| {
            let mut v: Vec<u8> = Vec::new();
            v.append(state_snapshot_cb_data_candid_bytes);  // moves the bytes out of the state_snapshot vec
            v
        })
    );
}




// -------------------------------------------------------------------------

#[derive(CandidType, Deserialize)]
pub struct CTSUCMetrics {
    canister_cycles_balance: Cycles,
    cycles_balance: Cycles,
    user_canister_ctsfuel_balance: CTSFuel,
    wasm_memory_size_bytes: u128,
    stable_memory_size_bytes: u64,
    storage_size_mib: u128,
    lifetime_termination_timestamp_seconds: u128,
    cycles_transferrer_canisters: Vec<Principal>,
    user_id: Principal,
    user_canister_creation_timestamp_nanos: u128,
    cycles_transfers_id_counter: u128,
    cycles_transfers_out_len: u128,
    cycles_transfers_in_len: u128,
    memory_size_at_the_start: u128,
    storage_usage: u128,
    free_storage: u128,
}


#[query]
pub fn cts_see_metrics() -> CTSUCMetrics {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    with(&CB_DATA, |cb_data| {
        CTSUCMetrics{
            canister_cycles_balance: canister_balance128(),
            cycles_balance: cb_data.user_data.cycles_balance,
            user_canister_ctsfuel_balance: ctsfuel_balance(),
            wasm_memory_size_bytes: ( core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES ) as u128,
            stable_memory_size_bytes: stable64_size() * WASM_PAGE_SIZE_BYTES as u64,
            storage_size_mib: cb_data.storage_size_mib,
            lifetime_termination_timestamp_seconds: cb_data.lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters: cb_data.cycles_transferrer_canisters.clone(),
            user_id: cb_data.user_id,
            user_canister_creation_timestamp_nanos: cb_data.user_canister_creation_timestamp_nanos,
            cycles_transfers_id_counter: cb_data.cycles_transfers_id_counter,
            cycles_transfers_in_len: cb_data.user_data.cycles_transfers_in.len() as u128,
            cycles_transfers_out_len: cb_data.user_data.cycles_transfers_out.len() as u128,
            memory_size_at_the_start: localkey::cell::get(&MEMORY_SIZE_AT_THE_START) as u128,
            storage_usage: calculate_current_storage_usage(),
            free_storage: calculate_free_storage()
        }
    })
}









