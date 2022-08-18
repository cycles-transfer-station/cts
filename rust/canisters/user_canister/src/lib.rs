// this canister can safe stop before upgrade


// use the heartbeat/timer for the check of the per_month_storage cost and check the cts-fuel in each call. if either one is 0, send the cycles-balance to the cts to save for the 10 years (calculate that the user pays for these 10 years in the first create_user_contract payment.)and delete 

// once the cts fuel is 0, send the cycles balance and the user_id to the cts for the safekeep. and block user calls (stop-calls flag?). the user can topup the user-contract-ctsfuel through the CTS-MAIN.
// when the storage/contract-duration is done, send the cycles balance and the user_id to the cts for the safekeep. and transfer the user-canister-cycles to the cts-main and delete the user-canister.

// let the user dowload the cycles-transfers (of the specific-logs-ids?)  with an autograph by the cts-main. charge for the cts-fuel. autograph by the cts-main is given when the user-contract is create that this user-contract is a part of the CTS.


//static USER_CANISTER_CTSFUEL_AUTO_TOPUP_PER_MONTH:               Cell<CTSFuel>               = Cell::new(0);   
//static USER_CANISTER_CTSFUEL_AUTO_TOPUP_PER_MONTH_LAST_CHARGE_TIMESTAMP_SECONDS: Cell<u64>   = Cell::new(0);
    

    
// ------------------------------------------------


use std::{
    cell::{RefCell,Cell},
    collections::HashMap,
};
use cts_lib::{
    ic_cdk::{
        self,
        api::{
            id,
            time,
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
        UserId,
        UserCanisterId,
        UsersMapCanisterId,
        cts,
        cycles_transferrer,
        user_canister::{
            UserCanisterInit,
            UserTransferCyclesQuest
        },
        management_canister,
    },
    consts::{
        KiB,
        MiB,
        GiB,
        WASM_PAGE_SIZE_BYTES,
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES,
        MANAGEMENT_CANISTER_ID,
    },
    tools::{
        localkey::{
            self,
            refcell::{
                with, 
                with_mut,
            }
        }
    },
    global_allocator_counter::get_allocated_bytes_count
};

fn set_global_timer(seconds: u64) {
    // re-place this with the system-api-call when the feature is in the replica.
}

// -------------------------------------------------------------------------


#[derive(CandidType, Deserialize, Clone)]
struct CyclesTransferIn {
    by_the_canister: Principal,
    cycles: Cycles,
    cycles_transfer_memo: CyclesTransferMemo,       // save max 32-bytes of the memo, of a Blob or of a Text
    timestamp_nanos: u64
}

#[derive(CandidType, Deserialize, Clone)]
struct CyclesTransferOut {
    for_the_canister: Principal,
    cycles_sent: Cycles,
    cycles_refunded: Option<Cycles>,   // option cause this field is only filled in the callback and that might not come back because of the callee holding-back the callback cross-upgrades. // if/when a user deletes some CyclesTransferPurchaseLogs, let the user set a special flag to delete the still-not-come-back-user_transfer_cycles by default unset.
    cycles_transfer_memo: CyclesTransferMemo,                           // save max 32-bytes of the memo, of a Blob or of a Text
    timestamp_nanos: u64, // time sent
    opt_cycles_transfer_call_error: Option<(u32/*reject_code*/, String/*reject_message*/)>, // None means the cycles_transfer-call replied. // save max 20-bytes of the string
    fee_paid: u64 // cycles_transferrer_fee
}



#[derive(CandidType, Deserialize, Clone)]
struct UserData {
    cycles_balance: Cycles,
    cycles_transfers_in: Vec<(u64,CyclesTransferIn)>,
    cycles_transfers_out: Vec<(u64,CyclesTransferOut)>,
}

impl UserData {
    fn new() -> Self {
        Self {
            cycles_balance: 0u128,
            cycles_transfers_in: Vec::new(),
            cycles_transfers_out: Vec::new(),
        }
    }
}



#[derive(CandidType, Deserialize)]
struct UCData {
    user_canister_creation_timestamp_nanos: u64,
    cts_id: Principal,
    cycles_market_id: Principal,
    user_id: UserId,
    user_canister_storage_size_mib: u64,
    user_canister_lifetime_termination_timestamp_seconds: u64,
    cycles_transferrer_canisters: Vec<Principal>,
    user_data: UserData,
    cycles_transfers_id_counter: u64,
}

impl UCData {
    fn new() -> Self {
        Self {
            user_canister_creation_timestamp_nanos: 0u64,
            cts_id: Principal::from_slice(&[]),
            cycles_market_id: Principal::from_slice(&[]),
            user_id: Principal::from_slice(&[]),
            user_canister_storage_size_mib: 0u64,       // memory-allocation/2 // is with the set in the canister_init // in the mib // starting at a 50mib-storage with a 1-year-user_canister_lifetime with a 5T-cycles-ctsfuel-balance at a cost: 10T-CYCLES   // this value is half of the user-canister-memory_allocation. for the upgrades.  
            user_canister_lifetime_termination_timestamp_seconds: 0u64,
            cycles_transferrer_canisters: Vec::new(),
            user_data: UserData::new(),
            cycles_transfers_id_counter: 0u64,        
        }
    }
}


pub const CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

const USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE: usize = 32;
const MINIMUM_USER_TRANSFER_CYCLES: Cycles = 1u128;
const CYCLES_TRANSFER_IN_MINIMUM_CYCLES: Cycles = 10_000_000_000;

const USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE: usize = 500usize;
const USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE: usize = 500usize;

const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;

const USER_CANISTER_BACKUP_CYCLES: Cycles = 1_400_000_000_000; 

const CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE: &'static str = "ctsfuel_balance is too low";


thread_local! {
   
    static UC_DATA: RefCell<UCData> = RefCell::new(UCData::new());

    // not save in a UCData
    static MEMORY_SIZE_AT_THE_START: Cell<usize> = Cell::new(0); 
    static CYCLES_TRANSFERRER_CANISTERS_ROUND_ROBIN_COUNTER: Cell<usize> = Cell::new(0);
    static STOP_CALLS: Cell<bool> = Cell::new(false);
    static STATE_SNAPSHOT_UC_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());

}



// ---------------------------------------------------------------------------------


#[init]
fn canister_init(user_canister_init: UserCanisterInit) {
    
    with_mut(&UC_DATA, |uc_data| {
        *uc_data = UCData{
            user_canister_creation_timestamp_nanos:                 time(),
            cts_id:                                                 user_canister_init.cts_id,
            cycles_market_id:                                       user_canister_init.cycles_market_id, 
            user_id:                                                user_canister_init.user_id,
            user_canister_storage_size_mib:                         user_canister_init.user_canister_storage_size_mib,
            user_canister_lifetime_termination_timestamp_seconds:   user_canister_init.user_canister_lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters:                           user_canister_init.cycles_transferrer_canisters,
            user_data:                                              UserData::new(),
            cycles_transfers_id_counter:                            0u64    
        };
    });

    
    localkey::cell::set(&MEMORY_SIZE_AT_THE_START, core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES);
    
    set_global_timer(user_canister_init.user_canister_lifetime_termination_timestamp_seconds - time()/1_000_000_000);
    
}




// ---------------------------------------------------------------------------------




/*

#[derive(CandidType, Deserialize)]
struct OldUCData {
    
}


#[derive(CandidType, Deserialize, Clone)]
struct OldUserData {
    
}

*/






fn create_uc_data_candid_bytes() -> Vec<u8> {
    with_mut(&UC_DATA, |uc_data| { 
        uc_data.user_data.cycles_transfers_in.shrink_to_fit();
        uc_data.user_data.cycles_transfers_out.shrink_to_fit(); 
    });
    
    let mut uc_data_candid_bytes: Vec<u8> = with(&UC_DATA, |uc_data| { encode_one(uc_data).unwrap() });
    uc_data_candid_bytes.shrink_to_fit();
    uc_data_candid_bytes
}

fn re_store_uc_data_candid_bytes(uc_data_candid_bytes: Vec<u8>) {
    
    let uc_data: UCData = match decode_one::<UCData>(&uc_data_candid_bytes) {
        Ok(uc_data) => uc_data,
        Err(_) => {
            trap("error decode of the UCData");
            /*
            let old_uc_data: OldUCData = decode_one::<OldUCData>(&uc_data_candid_bytes).unwrap();
            let uc_data: UCData = UCData{
                cts_id: old_uc_data.cts_id,
                .......
            };
            uc_data
            */
       }
    };

    std::mem::drop(uc_data_candid_bytes);

    with_mut(&UC_DATA, |ucd| {
        *ucd = uc_data;
    });

}


// ---------------------------------------------------------------------------------



#[pre_upgrade]
fn pre_upgrade() {
    let uc_upgrade_data_candid_bytes: Vec<u8> = create_uc_data_candid_bytes();
    
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
    
    re_store_uc_data_candid_bytes(uc_upgrade_data_candid_bytes);
    
}



// ---------------------------------------------------------------------------------

#[no_mangle]
async fn canister_global_timer() {
    if time()/1_000_000_000 > with(&UC_DATA, |uc_data| { uc_data.user_canister_lifetime_termination_timestamp_seconds }) - 30/*30 seconds slippage somewhere*/ {
        // call the cts-main for the user-canister-termination
        // the CTS will save the user_id, user_canister_id, and user_cycles_balance for a minimum of the 10-years.
        match call::<(cts::UserCanisterLifetimeTerminationQuest,), ()>(
            cts_id(),
            "user_canister_lifetime_termination",
            (cts::UserCanisterLifetimeTerminationQuest{
                user_id: user_id(),
                user_cycles_balance: user_cycles_balance()
            },),
        ).await {
            Ok(_) => {},
            Err(call_error) => {
                set_global_timer(100); // re-try in the 100-seconds
            }
        }
    }
}


// ---------------------------------------------------------------------------------

fn cts_id() -> Principal {
    with(&UC_DATA, |uc_data| { uc_data.cts_id })
}

fn user_id() -> UserId {
    with(&UC_DATA, |uc_data| { uc_data.user_id })
}

fn user_cycles_balance() -> Cycles {
    with(&UC_DATA, |uc_data| { uc_data.user_data.cycles_balance })
}

fn new_cycles_transfer_id() -> u64 {
    with_mut(&UC_DATA, |uc_data| { 
        let id: u64 = uc_data.cycles_transfers_id_counter.clone();
        uc_data.cycles_transfers_id_counter += 1;
        id
    })
}

// round-robin on the cycles-transferrer-canisters
fn next_cycles_transferrer_canister_round_robin() -> Option<Principal> {
    with(&UC_DATA, |uc_data| { 
        let ctcs: &Vec<Principal> = &(uc_data.cycles_transferrer_canisters);
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
fn calculate_current_storage_usage() -> u64 {
    (
        localkey::cell::get(&MEMORY_SIZE_AT_THE_START) 
        + 
        with(&UC_DATA, |uc_data| { 
            uc_data.user_data.cycles_transfers_in.len() * ( std::mem::size_of::<(u64,CyclesTransferIn)>() + 32/*for the cycles-transfer-memo-heap-size*/ )
            + 
            uc_data.user_data.cycles_transfers_out.len() * ( std::mem::size_of::<(u64,CyclesTransferOut)>() + 32/*for the cycles-transfer-memo-heap-size*/ + 20/*for the possible-call-error-string-heap-size*/ )
        })
    ) as u64
}

fn calculate_free_storage() -> u64 {
    ( with(&UC_DATA, |uc_data| { uc_data.user_canister_storage_size_mib }) * MiB ).checked_sub(calculate_current_storage_usage()).unwrap_or(0)
}


fn ctsfuel_balance() -> CTSFuel {
    canister_balance128()
    .checked_sub(user_cycles_balance()).unwrap_or(0)
    .checked_sub(USER_CANISTER_BACKUP_CYCLES).unwrap_or(0)
    .checked_sub(
        with(&UC_DATA, |uc_data| { 
            uc_data.user_canister_lifetime_termination_timestamp_seconds.checked_sub(time()/1_000_000_000).unwrap_or(0) as u128 
            * 
            uc_data.user_canister_storage_size_mib as u128 * 2 // canister-memory-allocation in the mib
        })
        *
        NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES as u128
        /
        1024/*network storage charge per MiB per second*/
    ).unwrap_or(0)
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



// for the now, check the ctsfuel balance on the 
//  user_transfer_cycles,
//  cycles_transfer,
//  download-cts-out,
//  download-cts-in 
//methods 









#[export_name = "canister_update cycles_transfer"]
pub fn cycles_transfer() { // (ct: CyclesTransfer) -> ()

    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); }

    if ctsfuel_balance() < 10_000_000_000 {
        if caller() == cts_id() {
            with_mut(&UC_DATA, |uc_data| { uc_data.user_data.cycles_balance = uc_data.user_data.cycles_balance.checked_add(msg_cycles_accept128(msg_cycles_available128())).unwrap_or(Cycles::MAX); });
            reply::<()>(());
            return;            
        }
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }

    if calculate_free_storage() < std::mem::size_of::<(u64,CyclesTransferIn)>() as u64 + 32 {
        if caller() == cts_id() {
            with_mut(&UC_DATA, |uc_data| { uc_data.user_data.cycles_balance = uc_data.user_data.cycles_balance.checked_add(msg_cycles_accept128(msg_cycles_available128())).unwrap_or(Cycles::MAX); });
            reply::<()>(());
            return;            
        }
        trap("Canister memory is full, cannot create a log of the cycles-transfer.");
    }

    if arg_data_raw_size() > 100 {
        trap("arg_data_raw_size can be max 100 bytes");
    }

    if msg_cycles_available128() < CYCLES_TRANSFER_IN_MINIMUM_CYCLES {
        trap(&format!("minimum cycles transfer cycles: {}", CYCLES_TRANSFER_IN_MINIMUM_CYCLES));
    }
        
    let cycles_cept: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    let (ct,): (CyclesTransfer,) = arg_data::<(CyclesTransfer,)>();
    
    with_mut(&UC_DATA, |uc_data| {
        uc_data.user_data.cycles_balance = uc_data.user_data.cycles_balance.checked_add(cycles_cept).unwrap_or(Cycles::MAX);
        uc_data.user_data.cycles_transfers_in.push((
            new_cycles_transfer_id(),
            CyclesTransferIn{
                by_the_canister: caller(),
                cycles: cycles_cept,
                cycles_transfer_memo: truncate_cycles_transfer_memo(ct.memo),
                timestamp_nanos: time()
            }
        ));
    });
    
    reply::<()>(());
    return;
    
}




#[export_name = "canister_query user_download_cycles_transfers_in"]
pub fn user_download_cycles_transfers_in() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }    
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); }
    
    with(&UC_DATA, |uc_data| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[(u64,CyclesTransferIn)]>,)>((uc_data.user_data.cycles_transfers_in.chunks(USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE).nth(chunk_i as usize),));
    });
    
}












#[derive(CandidType, Deserialize)]
pub enum UserTransferCyclesError {
    UserCanisterCTSFuelBalanceTooLow,
    UserCanisterMemoryIsFull,
    InvalidCyclesTransferMemoSize{max_size_bytes: u64},
    InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: Cycles },
    UserCyclesBalanceTooLow { user_cycles_balance: Cycles, cycles_transferrer_transfer_cycles_fee: Cycles },
    CyclesTransferrerTransferCyclesError(cycles_transferrer::TransferCyclesError),
    CyclesTransferrerTransferCyclesCallError((u32, String))
}

#[update]
pub async fn user_transfer_cycles(mut q: UserTransferCyclesQuest) -> Result<u64, UserTransferCyclesError> {

    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); }
    
    if ctsfuel_balance() < 15_000_000_000 {
        return Err(UserTransferCyclesError::UserCanisterCTSFuelBalanceTooLow);
    }
    
    if calculate_free_storage() < std::mem::size_of::<(u64,CyclesTransferOut)>() as u64 + 32 + 40 {
        return Err(UserTransferCyclesError::UserCanisterMemoryIsFull);
    }
    
    if q.cycles < MINIMUM_USER_TRANSFER_CYCLES {
        return Err(UserTransferCyclesError::InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: MINIMUM_USER_TRANSFER_CYCLES });
    }
    
    if q.cycles + CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE > user_cycles_balance() {
        return Err(UserTransferCyclesError::UserCyclesBalanceTooLow{ user_cycles_balance: user_cycles_balance(), cycles_transferrer_transfer_cycles_fee: CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE });
    }
    
    // check memo size
    match q.cycles_transfer_memo {
        CyclesTransferMemo::Blob(ref mut b) => {
            if b.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u64}); 
            }
            b.shrink_to_fit();
            if b.capacity() > b.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Text(ref mut s) => {
            if s.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u64}); 
            }
            s.shrink_to_fit();
            if s.capacity() > s.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Nat(_n) => {} 
    }

    let cycles_transfer_id: u64 = new_cycles_transfer_id(); 
     
    with_mut(&UC_DATA, |uc_data| {
        // take the user-cycles before the transfer, and refund in the callback 
        uc_data.user_data.cycles_balance -= q.cycles + CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE;
        uc_data.user_data.cycles_transfers_out.push((
            cycles_transfer_id,
            CyclesTransferOut{
                for_the_canister: q.for_the_canister,
                cycles_sent: q.cycles,
                cycles_refunded: None,   // None means the cycles_transfer-call-callback did not come back yet(did not give-back a reply-or-reject-sponse) 
                cycles_transfer_memo: q.cycles_transfer_memo.clone(),
                timestamp_nanos: time(), // time sent
                opt_cycles_transfer_call_error: None,
                fee_paid: CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE as u64
            }
        ));
    });
    
    let q_cycles: Cycles = q.cycles; // copy cause want the value to stay on the stack for the closure to run with it. after the q is move into the candid params
    let cycles_transferrer_transfer_cycles_fee: Cycles = CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE; // copy the value to stay on the stack for the closure to run with it.
   
    let cancel_user_transfer_cycles = || {
        with_mut(&UC_DATA, |uc_data| {
            uc_data.user_data.cycles_balance = uc_data.user_data.cycles_balance.checked_add(q_cycles + cycles_transferrer_transfer_cycles_fee).unwrap_or(Cycles::MAX);
            
            match uc_data.user_data.cycles_transfers_out.iter().rposition(
                |cycles_transfer_out_log: &(u64,CyclesTransferOut)| { 
                    (*cycles_transfer_out_log).0 == cycles_transfer_id
                }
            ) {
                Some(i) => { uc_data.user_data.cycles_transfers_out.remove(i); },
                None => {}
            }
        });
    };
        
    match call_with_payment128::<(cycles_transferrer::TransferCyclesQuest,), (Result<(), cycles_transferrer::TransferCyclesError>,)>(
        next_cycles_transferrer_canister_round_robin().expect("0 known cycles transferrer canisters.")/*before the first await*/,
        "transfer_cycles",
        (cycles_transferrer::TransferCyclesQuest{
            user_cycles_transfer_id: cycles_transfer_id as u128,
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
    
    if with(&UC_DATA, |uc_data| { uc_data.cycles_transferrer_canisters.contains(&caller()) }) == false {
        trap("caller must be one of the CTS-cycles-transferrer-canisters for this method.");
    }
    
    //if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon."); } // make sure that when set a stop-call-flag, there are 0 ongoing-$cycles-transfers. cycles-transfer-callback errors will hold for
    
    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128()); 

    with_mut(&UC_DATA, |uc_data| {
        uc_data.user_data.cycles_balance = uc_data.user_data.cycles_balance.checked_add(cycles_transfer_refund).unwrap_or(u128::MAX);
        if let Some(cycles_transfer_out_log/*: &mut (u64,CyclesTransferOut)*/) = uc_data.user_data.cycles_transfers_out.iter_mut().rev().find(|cycles_transfer_out_log: &&mut (u64,CyclesTransferOut)| { (**cycles_transfer_out_log).0 as u128 == q.user_cycles_transfer_id }) {
            (*cycles_transfer_out_log).1.cycles_refunded = Some(cycles_transfer_refund);
            (*cycles_transfer_out_log).1.opt_cycles_transfer_call_error = q.opt_cycles_transfer_call_error;
        }
    });

}







#[export_name = "canister_query user_download_cycles_transfers_out"]
pub fn user_download_cycles_transfers_out() {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    if ctsfuel_balance() < 10_000_000_000 {
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try soon."); }
    
    with(&UC_DATA, |uc_data| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[(u64,CyclesTransferOut)]>,)>((uc_data.user_data.cycles_transfers_out.chunks(USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}


// ---------------------------------------------------





// ---------------------------------------------------


/*
#[export_name = "canister_query user_cycles_balance"]
pub fn user_cycles_balance_canister_method() {  // -> Cycles
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    reply::<(Cycles,)>((user_cycles_balance(),));
    return;
}
*/




#[derive(CandidType, Deserialize)]
pub struct UserUCMetrics {
    user_cycles_balance: Cycles,
    user_canister_ctsfuel_balance: CTSFuel,
    user_canister_storage_size_mib: u64,
    user_canister_lifetime_termination_timestamp_seconds: u64,
    cycles_transferrer_canisters: Vec<Principal>,
    user_id: UserId,
    user_canister_creation_timestamp_nanos: u64,
    cycles_transfers_id_counter: u64,
    cycles_transfers_out_len: u64,
    cycles_transfers_in_len: u64,
    storage_usage: u64,
    user_download_cycles_transfers_in_chunk_size: u64,
    user_download_cycles_transfers_out_chunk_size: u64
}


#[query]
pub fn user_see_metrics() -> UserUCMetrics {
    if caller() != user_id() {
        trap("Caller must be the user for this method.");
    }
    
    with(&UC_DATA, |uc_data| {
        UserUCMetrics{
            user_cycles_balance: uc_data.user_data.cycles_balance,
            user_canister_ctsfuel_balance: ctsfuel_balance(),
            user_canister_storage_size_mib: uc_data.user_canister_storage_size_mib,
            user_canister_lifetime_termination_timestamp_seconds: uc_data.user_canister_lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters: uc_data.cycles_transferrer_canisters.clone(),
            user_id: uc_data.user_id,
            user_canister_creation_timestamp_nanos: uc_data.user_canister_creation_timestamp_nanos,
            cycles_transfers_id_counter: uc_data.cycles_transfers_id_counter,
            cycles_transfers_in_len: uc_data.user_data.cycles_transfers_in.len() as u64,
            cycles_transfers_out_len: uc_data.user_data.cycles_transfers_out.len() as u64,
            storage_usage: calculate_current_storage_usage(),
            user_download_cycles_transfers_in_chunk_size: USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE as u64,
            user_download_cycles_transfers_out_chunk_size: USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE as u64
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
    UserCyclesBalanceTooLow { user_cycles_balance: Cycles }
}

#[update]
pub fn user_cycles_balance_for_the_ctsfuel(cycles_for_the_ctsfuel: Cycles) -> Result<(), UserCyclesBalanceForTheCTSFuelError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance, try soon."); }
    
    if user_cycles_balance() < cycles_for_the_ctsfuel {
        return Err(UserCyclesBalanceForTheCTSFuelError::UserCyclesBalanceTooLow{ user_cycles_balance: user_cycles_balance() });        
    } 
    
    with_mut(&UC_DATA, |uc_data| {
        uc_data.user_data.cycles_balance -= cycles_for_the_ctsfuel;
        // cycles-transfer-out log? what if storage is full and ctsfuel is empty?
    });
    
    Ok(())
}



// ---------------------------------------------

// cts or user? 
// cts, cause the cts must change the canister-memory-allocation. 
// but the user-canister can be one of its own controllers and change the canister-memory-allocation itself?

#[derive(CandidType, Deserialize)]
pub struct UserLengthenUserCanisterLifetimeTerminationQuest {
    lengthen_seconds: u64
}

#[derive(CandidType, Deserialize)]
pub enum UserLengthenUserCanisterLifetimeTerminationError {
    UserCyclesBalanceTooLow{ user_cycles_balance: Cycles, lengthen_seconds_cost_cycles: Cycles }
}

#[update]
pub fn user_lengthen_user_canister_lifetime_termination(q: UserLengthenUserCanisterLifetimeTerminationQuest) -> Result<u64/*new-lifetime-termination-timestamp-seconds*/, UserLengthenUserCanisterLifetimeTerminationError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    if localkey::cell::get(&STOP_CALLS) { trap("Maintenance, try soon."); }
    
    let lengthen_seconds_cost_cycles: Cycles = {
        (
            q.lengthen_seconds 
            * with(&UC_DATA, |uc_data| { uc_data.user_canister_storage_size_mib }) * 2 // canister-memory-allocation in the mib 
            * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES / 1024/*network storage charge per MiB per second*/
        ).into()
    };
    
    if lengthen_seconds_cost_cycles > user_cycles_balance() {
        return Err(UserLengthenUserCanisterLifetimeTerminationError::UserCyclesBalanceTooLow{ user_cycles_balance: user_cycles_balance(), lengthen_seconds_cost_cycles });
    }
    
    with_mut(&UC_DATA, |uc_data| {
        uc_data.user_canister_lifetime_termination_timestamp_seconds += q.lengthen_seconds;
        
        set_global_timer(uc_data.user_canister_lifetime_termination_timestamp_seconds - time()/1_000_000_000);
    
        uc_data.user_data.cycles_balance -= lengthen_seconds_cost_cycles; 
    
        Ok(uc_data.user_canister_lifetime_termination_timestamp_seconds)
    })
}



// ---------------------------

#[derive(CandidType, Deserialize)]
pub struct UserChangeUserCanisterStorageSizeMibQuest {
    new_storage_size_mib: u64
}

#[derive(CandidType, Deserialize)]
pub enum UserChangeUserCanisterStorageSizeMibError {
    NewStorageSizeMibTooLow{ minimum_new_storage_size_mib: u64 },
    UserCyclesBalanceTooLow{ user_cycles_balance: Cycles, new_storage_size_mib_cost_cycles: Cycles },
    ManagementCanisterUpdateSettingsCallError((u32, String))
}

#[update]
pub async fn user_change_user_canister_storage_size_mib(q: UserChangeUserCanisterStorageSizeMibQuest) -> Result<(), UserChangeUserCanisterStorageSizeMibError> {
    if caller() != user_id() {
        trap("caller must be the user for this method.");
    }
    
    let minimum_new_storage_size_mib: u64 = with(&UC_DATA, |uc_data| { uc_data.user_canister_storage_size_mib }) + 5; 
    
    if minimum_new_storage_size_mib > q.new_storage_size_mib {
        return Err(UserChangeUserCanisterStorageSizeMibError::NewStorageSizeMibTooLow{ minimum_new_storage_size_mib }); 
    };
    
    let new_storage_size_mib_cost_cycles: Cycles = {
        (
            ( q.new_storage_size_mib - with(&UC_DATA, |uc_data| { uc_data.user_canister_storage_size_mib }) ) * 2 // grow canister-memory-allocation in the mib 
            * with(&UC_DATA, |uc_data| { uc_data.user_canister_lifetime_termination_timestamp_seconds }).checked_sub(time()/1_000_000_000).expect("user-contract-lifetime is with the termination.")
            * NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES / 1024 /*network storage charge per MiB per second*/
        ).into()
    };
    
    if user_cycles_balance() < new_storage_size_mib_cost_cycles {
        return Err(UserChangeUserCanisterStorageSizeMibError::UserCyclesBalanceTooLow{ user_cycles_balance: user_cycles_balance(), new_storage_size_mib_cost_cycles });
    }

    // take the cycles before the .await and if error after here, refund the cycles
    with_mut(&UC_DATA, |uc_data| {
        uc_data.user_data.cycles_balance -= new_storage_size_mib_cost_cycles; 
    });

    match call::<(management_canister::ChangeCanisterSettingsRecord,), ()>(
        MANAGEMENT_CANISTER_ID,
        "update_settings",
        (management_canister::ChangeCanisterSettingsRecord{
            canister_id: ic_cdk::api::id(),
            settings: management_canister::ManagementCanisterOptionalCanisterSettings{
                controllers : None,
                compute_allocation : None,
                memory_allocation : Some((q.new_storage_size_mib * 2 * MiB).into()),
                freezing_threshold : None,
            }
        },)
    ).await {
        Ok(()) => {
            with_mut(&UC_DATA, |uc_data| {
                uc_data.user_canister_storage_size_mib = q.new_storage_size_mib;
            });
            Ok(())
        },
        Err(call_error) => {
            with_mut(&UC_DATA, |uc_data| {
                uc_data.user_data.cycles_balance = uc_data.user_data.cycles_balance.checked_add(new_storage_size_mib_cost_cycles).unwrap_or(Cycles::MAX); 
            });
            return Err(UserChangeUserCanisterStorageSizeMibError::ManagementCanisterUpdateSettingsCallError((call_error.0 as u32, call_error.1)));
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
    with_mut(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        *state_snapshot_uc_data_candid_bytes = create_uc_data_candid_bytes();
        state_snapshot_uc_data_candid_bytes.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_uc_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    });
}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        *state_snapshot_uc_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        state_snapshot_uc_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_uc_data_out_of_the_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    re_store_uc_data_candid_bytes(
        with_mut(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
            let mut v: Vec<u8> = Vec::new();
            v.append(state_snapshot_uc_data_candid_bytes);  // moves the bytes out of the state_snapshot vec
            v
        })
    );
}




// -------------------------------------------------------------------------

#[derive(CandidType, Deserialize)]
pub struct CTSUCMetrics {
    canister_cycles_balance: Cycles,
    user_cycles_balance: Cycles,
    user_canister_ctsfuel_balance: CTSFuel,
    wasm_memory_size_bytes: u64,
    stable_memory_size_bytes: u64,
    user_canister_storage_size_mib: u64,
    user_canister_lifetime_termination_timestamp_seconds: u64,
    cycles_transferrer_canisters: Vec<Principal>,
    user_id: UserId,
    user_canister_creation_timestamp_nanos: u64,
    cycles_transfers_id_counter: u64,
    cycles_transfers_out_len: u64,
    cycles_transfers_in_len: u64,
    memory_size_at_the_start: u64,
    storage_usage: u64,
    free_storage: u64,
}


#[query]
pub fn cts_see_metrics() -> CTSUCMetrics {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    with(&UC_DATA, |uc_data| {
        CTSUCMetrics{
            canister_cycles_balance: canister_balance128(),
            user_cycles_balance: uc_data.user_data.cycles_balance,
            user_canister_ctsfuel_balance: ctsfuel_balance(),
            wasm_memory_size_bytes: ( core::arch::wasm32::memory_size(0)*WASM_PAGE_SIZE_BYTES ) as u64,
            stable_memory_size_bytes: stable64_size() * WASM_PAGE_SIZE_BYTES as u64,
            user_canister_storage_size_mib: uc_data.user_canister_storage_size_mib,
            user_canister_lifetime_termination_timestamp_seconds: uc_data.user_canister_lifetime_termination_timestamp_seconds,
            cycles_transferrer_canisters: uc_data.cycles_transferrer_canisters.clone(),
            user_id: uc_data.user_id,
            user_canister_creation_timestamp_nanos: uc_data.user_canister_creation_timestamp_nanos,
            cycles_transfers_id_counter: uc_data.cycles_transfers_id_counter,
            cycles_transfers_in_len: uc_data.user_data.cycles_transfers_in.len() as u64,
            cycles_transfers_out_len: uc_data.user_data.cycles_transfers_out.len() as u64,
            memory_size_at_the_start: localkey::cell::get(&MEMORY_SIZE_AT_THE_START) as u64,
            storage_usage: calculate_current_storage_usage(),
            free_storage: calculate_free_storage()
        }
    })
}









