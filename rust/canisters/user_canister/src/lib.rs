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
    call_error: Option<(u32/*reject_code*/, String/*reject_message*/)>, // None means the cycles_transfer-call replied. // save max 20-bytes of the string
    fee_paid: u64 // cycles_transferrer_fee
}




#[derive(CandidType, Deserialize, Clone)]
struct UserData {
    cycles_balance: Cycles,
    cycles_transfers_in: Vec<(u64,CyclesTransferIn)>,
    cycles_transfers_out: Vec<(u64,CyclesTransferOut)>,
    
    //icp_transfers_in: Vec<IcpBlockHeight>,
    //icp_transfers_out: Vec<IcpBlockHeight>,
}

impl UserData {
    fn new() -> Self {
        Self {
            cycles_balance: 0u128,
            cycles_transfers_in: Vec::new(),
            cycles_transfers_out: Vec::new(),
            
            //icp_transfers_in: Vec::new(),
            //icp_transfers_out: Vec::new(),
        }
    }
}



#[derive(CandidType, Deserialize)]
struct UCData {
    user_canister_creation_timestamp_nanos: u64,
    cts_id: Principal,
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
            user_id: Principal::from_slice(&[]),
            user_canister_storage_size_mib: 0u64,       // memory-allocation/2 // is with the set in the canister_init // in the mib // starting at a 50mib-storage with a 1-year-user_canister_lifetime with a 5T-cycles-ctsfuel-balance at a cost: 10T-CYCLES   // this value is half of the user-canister-memory_allocation. for the upgrades.  
            user_canister_lifetime_termination_timestamp_seconds: 0u64,
            cycles_transferrer_canisters: Vec::new(),
            user_data: UserData::new(),
            cycles_transfers_id_counter: 0u64,        
        }
    }
}


pub const CYCLES_TRANSFER_FEE/*CYCLES_TRANSFERRER_FEE*/: Cycles = 10_000_000_000;

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
        reject(CTSFUEL_BALANCE_TOO_LOW_REJECT_MESSAGE);
        return;
    }

    if calculate_free_storage() < std::mem::size_of::<(u64,CyclesTransferIn)>() as u64 + 32 {
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
    UserCyclesBalanceTooLow { user_cycles_balance: Cycles, cycles_transfer_fee: Cycles },
    CyclesTransferrerTransferCyclesError(cycles_transferrer::TransferCyclesError),
    CyclesTransferrerTransferCyclesCallError((u32, String))
}

#[update]
pub async fn user_transfer_cycles(q: UserTransferCyclesQuest) -> Result<u64, UserTransferCyclesError> {

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
    
    if q.cycles + CYCLES_TRANSFER_FEE > user_cycles_balance() {
        return Err(UserTransferCyclesError::UserCyclesBalanceTooLow{ user_cycles_balance: user_cycles_balance(), cycles_transfer_fee: CYCLES_TRANSFER_FEE });
    }
    
    // check memo size
    match q.cycles_transfer_memo {
        CyclesTransferMemo::Blob(ref b) => {
            if b.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u64}); 
            }
            //b.shrink_to_fit();
            if b.capacity() > b.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Text(ref s) => {
            if s.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes: USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE as u64}); 
            }
            //s.shrink_to_fit();
            if s.capacity() > s.len() { trap("check this out"); }
        },
        CyclesTransferMemo::Nat(_n) => {} 
    }

    let cycles_transfer_id: u64 = new_cycles_transfer_id(); 
    
    with_mut(&UC_DATA, |uc_data| {
        // take the user-cycles before the transfer, and refund in the callback 
        uc_data.user_data.cycles_balance -= q.cycles + CYCLES_TRANSFER_FEE;
        uc_data.user_data.cycles_transfers_out.push((
            cycles_transfer_id,
            CyclesTransferOut{
                for_the_canister: q.for_the_canister,
                cycles_sent: q.cycles,
                cycles_refunded: None,   // None means the cycles_transfer-call-callback did not come back yet(did not give-back a reply-or-reject-sponse) 
                cycles_transfer_memo: q.cycles_transfer_memo.clone(),
                timestamp_nanos: time(), // time sent
                call_error: None,
                fee_paid: CYCLES_TRANSFER_FEE as u64
            }
        ));
    });
    
    let q_cycles: Cycles = q.cycles; // copy cause want the value to stay on the stack for the closure to run with it. after the q is move into the candid params
    let cycles_transfer_fee: Cycles = CYCLES_TRANSFER_FEE; // copy the value to stay on the stack for the closure to run with it.
    let cancel_user_transfer_cycles = || {
        with_mut(&UC_DATA, |uc_data| {
            uc_data.user_data.cycles_balance = uc_data.user_data.cycles_balance.checked_add(q_cycles + cycles_transfer_fee).unwrap_or(Cycles::MAX);
            
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
            user_cycles_transfer_id: cycles_transfer_id,
            for_the_canister: q.for_the_canister,
            cycles: q.cycles,
            cycles_transfer_memo: q.cycles_transfer_memo
        },),
        q.cycles + cycles_transfer_fee
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
        if let Some(cycles_transfer_out_log/*: &mut (u64,CyclesTransferOut)*/) = uc_data.user_data.cycles_transfers_out.iter_mut().rev().find(|cycles_transfer_out_log: &&mut (u64,CyclesTransferOut)| { (**cycles_transfer_out_log).0 == q.user_cycles_transfer_id }) {
            (*cycles_transfer_out_log).1.cycles_refunded = Some(cycles_transfer_refund);
            (*cycles_transfer_out_log).1.call_error = q.cycles_transfer_call_error;
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





















// -------------------------------------------------------------------------



    /*
    
    let cycles_transfer_call_candid_bytes = match encode_one(&CyclesTransfer{ memo: q.cycles_transfer_memo }) {
        Ok(cb) => cb,
        Err(ce) => return Err(UserTransferCyclesError::CyclesTransferCallCandidEncodeError("{:?}", ce))
    }; // maybe unwrap it and let it panick and roll back if error
    
    
    
    let cycles_transfer_call = CallResult<Vec<u8>>> = call_raw128(
        q.canister_id,
        "cycles_transfer",
        &cycles_transfer_call_candid_bytes,
        q.cycles
    );
    
    std::mem::drop(cycles_transfer_call_candid_bytes);
    std::mem::drop(q);
    
    let cycles_transfer_call_result: CallResult<Vec<u8>> = cycles_transfer_call.await;
    
    let cycles_refund: Cycles = msg_cycles_refunded128();
    
    with_mut(&USER_DATA, |user_data| {
        user_data.cycles_balance += cycles_refund;
    });
    
    let final_cycles_transfer_purchase_log: CyclesTransferPurchaseLog = with_mut(&USER_DATA, |user_data| {
        match user_data.cycles_transfer_purchases.get(&cycles_transfer_purchase_log_id) {
            Some(cycles_transfer_purchase_log) => {
                cycles_transfer_purchase_log.cycles_accepted = Some(cycles_transfer_purchase_log.cycles_sent - cycles_refund);
                cycles_transfer_purchase_log.clone()
            },
            None => trap("not sure what happen")
        }
    });

    match cycles_transfer_call_result {
        Ok(_) => {
            return Ok(final_cycles_transfer_purchase_log);
        },
        
        // up to here
        Err(cycles_transfer_call_error) => {
            let paid_fee: bool = match cycles_transfer_call_error.0 {
                RejectionCode::DestinationInvalid | RejectionCode::CanisterReject | RejectionCode::CanisterError => {
                    true
                },
                _ => {
                    USERS_DATA.with(|ud| { ud.borrow_mut().get_mut(&user).unwrap().cycles_balance += CYCLES_TRANSFER_FEE; });
                    false
                }
            };
            return CollectBalanceSponse::cycles_payout(Err(UserTransferCyclesError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: paid_fee, cycles_accepted: cycles_accepted }));
        }
    }
    

}

*/    











/*


#[export_name = "canister_query see_cycles_transfer_purchases"]
pub fn see_cycles_transfer_purchases<'a>() -> () {
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    let (param,): (u128,) = ic_cdk::api::call::arg_data::<(u128,)>(); 
    
    let user_cycles_transfer_purchases: *const Vec<CyclesTransferPurchaseLog> = with(&USER_DATA, |user_data| { 
        (&user_data.cycles_transfer_purchases) as *const Vec<CyclesTransferPurchaseLog>
    });

    // check if drop gets called after this call
    ic_cdk::api::call::reply::<(&'a Vec<CyclesTransferPurchaseLog>,)>((unsafe { &*user_cycles_transfer_purchases },))
}



#[export_name = "canister_query see_cycles_bank_purchases"]
pub fn see_cycles_bank_purchases<'a>() -> () {
    if caller() != user_id() {
        trap("caller must be the user")
    }

    let (param,): (u128,) = ic_cdk::api::call::arg_data::<(u128,)>(); 

    let user_cycles_bank_purchases: *const Vec<CyclesBankPurchaseLog> = with(&USER_DATA, |user_data| { 
        (&user_data.cycles_bank_purchases) as *const Vec<CyclesBankPurchaseLog>
    });

    ic_cdk::api::call::reply::<(&'a Vec<CyclesBankPurchaseLog>,)>((unsafe { &*user_cycles_bank_purchases },))

}



















#[derive(CandidType, Deserialize)]
pub struct IcpPayoutQuest {
    icp: IcpTokens,
    payout_icp_id: IcpId
}


#[derive(CandidType, Deserialize)]
pub enum CollectBalanceQuest {
    icp_payout(IcpPayoutQuest),
    cycles_payout(CyclesPayoutQuest)
}

#[derive(CandidType, Deserialize)]
pub enum IcpPayoutError {
    InvalidIcpPayout0Amount,
    IcpLedgerCheckBalanceCallError(String),
    BalanceTooLow { max_icp_payout: IcpTokens },
    IcpLedgerTransferError(IcpTransferError),
    IcpLedgerTransferCallError(String),


}


pub type IcpPayoutSponse = Result<IcpBlockHeight, IcpPayoutError>;



#[derive(CandidType, Deserialize)]
pub enum CollectBalanceSponse {
    icp_payout(IcpPayoutSponse),
    cycles_payout(CyclesPayoutSponse)
}

#[update]
pub async fn collect_balance(collect_balance_quest: CollectBalanceQuest) -> CollectBalanceSponse {
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    let user: Principal = caller();

    check_lock_and_lock_user(&user);

    match collect_balance_quest {

        CollectBalanceQuest::icp_payout(icp_payout_quest) => {
            
            if icp_payout_quest.icp == IcpTokens::ZERO {
                unlock_user(&user);
                return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::InvalidIcpPayout0Amount));
            }
            
            let user_icp_balance: IcpTokens = match check_user_icp_balance(&user).await {
                Ok(icp_tokens) => icp_tokens,
                Err(balance_call_error) => {
                    unlock_user(&user);
                    return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error))));
                }
            };
            
            if icp_payout_quest.icp + ICP_PAYOUT_FEE + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) > user_icp_balance {
                unlock_user(&user);
                return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::BalanceTooLow { max_icp_payout: user_icp_balance - ICP_PAYOUT_FEE - IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) }));
            }
            
            let icp_payout_transfer_call: CallResult<IcpTransferResult> = icp_transfer(
                MAINNET_LEDGER_CANISTER_ID,
                IcpTransferArgs {
                    memo: ICP_PAYOUT_MEMO,
                    amount: icp_payout_quest.icp,
                    fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                    from_subaccount: Some(principal_icp_subaccount(&user)),
                    to: icp_payout_quest.payout_icp_id,
                    created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
                }
            ).await; 
            let icp_payout_transfer_call_block_index: IcpBlockHeight = match icp_payout_transfer_call {
                Ok(transfer_result) => match transfer_result {
                    Ok(block_index) => block_index,
                    Err(transfer_error) => {
                        unlock_user(&user);
                        return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferError(transfer_error)));
                    }
                },
                Err(transfer_call_error) => {
                    unlock_user(&user);
                    return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferCallError(format!("{:?}", transfer_call_error))));
                }
            };

            let icp_payout_take_fee_transfer_call: CallResult<IcpTransferResult> = icp_transfer(
                MAINNET_LEDGER_CANISTER_ID,
                IcpTransferArgs {
                    memo: ICP_TAKE_PAYOUT_FEE_MEMO,
                    amount: ICP_PAYOUT_FEE,
                    fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                    from_subaccount: Some(principal_icp_subaccount(&user)),
                    to: main_cts_icp_id(),                        
                    created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
                }
            ).await;             
            match icp_payout_take_fee_transfer_call {
                Ok(transfer_result) => match transfer_result {
                    Ok(_block_index) => {},
                    Err(_transfer_error) => {
                        USERS_DATA.with(|ud| {
                            ud.borrow_mut().get_mut(&user).unwrap().untaken_icp_to_collect += ICP_PAYOUT_FEE + ICP_LEDGER_TRANSFER_DEFAULT_FEE;
                        });
                    }  // log and take into the count 
                },
                Err(_transfer_call_error) => { // log and take into the count
                    USERS_DATA.with(|ud| {
                        ud.borrow_mut().get_mut(&user).unwrap().untaken_icp_to_collect += ICP_PAYOUT_FEE + ICP_LEDGER_TRANSFER_DEFAULT_FEE;
                    });
                }
            }
            unlock_user(&user);
            return CollectBalanceSponse::icp_payout(Ok(icp_payout_transfer_call_block_index));
        },



    }
}









#[derive(CandidType, Deserialize)]
pub struct ConvertIcpBalanceForTheCyclesWithTheCmcRateQuest {
    icp: IcpTokens
}

#[derive(CandidType, Deserialize)]
pub enum ConvertIcpBalanceForTheCyclesWithTheCmcRateError {
    CmcGetRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    IcpLedgerCheckBalanceCallError(String),
    IcpBalanceTooLow { max_icp_convert_for_the_cycles: IcpTokens },
    LedgerTopupCyclesError(LedgerTopupCyclesError),
}




// ledger takes the fee twice out of the users icp subaccount balance
// now with the new cmc-notify method ledger takes only once fee

// :flat-fee: 10369909-cycles? 20_000_000-cycles - 1/500 of a penny of an xdr

#[update]
pub async fn convert_icp_balance_for_the_cycles_with_the_cmc_rate(q: ConvertIcpBalanceForTheCyclesWithTheCmcRateQuest) -> Result<Cycles, ConvertIcpBalanceForTheCyclesWithTheCmcRateError> {    
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    let user: Principal = caller();

    // check minimum-conversion [a]mount


    // let xdr_permyriad_per_icp: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate().await {
    //     Ok(rate) => rate,
    //     Err(check_current_rate_error) => {
    //         return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::CmcGetRateError(check_current_rate_error));
    //     }
    // };
    // let cycles: u128 = icptokens_to_cycles(q.icp, xdr_permyriad_per_icp);

    check_lock_and_lock_user(&user);

    let user_icp_balance: IcpTokens = match check_user_icp_balance(&user).await {
        Ok(icp_tokens) => icp_tokens,
        Err(balance_call_error) => {
            unlock_user(&user);
            return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error)));
        }
    };

    if q.icp + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) > user_icp_balance {
        unlock_user(&user);
        return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::IcpBalanceTooLow { max_icp_convert_for_the_cycles: user_icp_balance - IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) });
    }

    let topup_cycles: Cycles = match ledger_topup_cycles(q.icp, Some(principal_icp_subaccount(&user)), ic_cdk::api::id()).await {
        Ok(cycles) => cycles,
        Err(ledger_topup_cycles_error) => {
            unlock_user(&user);
            return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::LedgerTopupCyclesError(ledger_topup_cycles_error));
        }
    };

    USERS_DATA.with(|ud| {
        ud.borrow_mut().get_mut(&user).unwrap().cycles_balance += topup_cycles;
    });

    unlock_user(&user);

    Ok(topup_cycles)
}
















#[update]
pub async fn purchase_cycles_transfer(pctq: PurchaseCyclesTransferQuest) -> Result<CyclesTransferPurchaseLog, PurchaseCyclesTransferError> {
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    let user: Principal = caller();
    
    if pctq.cycles == 0 {
        return Err(PurchaseCyclesTransferError::InvalidCyclesTransfer0Amount);
    }

    check_lock_and_lock_user(&user);

    let user_cycles_balance: u128 = match check_user_cycles_balance(&user).await {
        Ok(cycles) => cycles,
        Err(check_user_cycles_balance_error) => {
            unlock_user(&user);
            return Err(PurchaseCyclesTransferError::CheckUserCyclesBalanceError(check_user_cycles_balance_error));
        }
    };

    if user_cycles_balance < pctq.cycles + CYCLES_TRANSFER_FEE {
        unlock_user(&user);
        return Err(PurchaseCyclesTransferError::BalanceTooLow { max_cycles_for_the_transfer: user_cycles_balance - CYCLES_TRANSFER_FEE });
    }

    // change!! take the user-cycles before the transfer, and refund in the callback 

    let cycles_transfer_candid_bytes: Vec<u8> = match encode_one(&pctq.cycles_transfer) {
        Ok(candid_bytes) => candid_bytes,
        Err(candid_error) => {
            unlock_user(&user);
            return Err(PurchaseCyclesTransferError::CyclesTransferCallCandidEncodeError(format!("{}", candid_error)));
        }
    };

    let cycles_transfer_call: CallResult<Vec<u8>> = call_raw128(
        pctq.canister,
        "cycles_transfer",
        &cycles_transfer_candid_bytes,
        pctq.cycles
    ).await;

    unlock_user(&user);

    match cycles_transfer_call {
        Ok(_) => {

            let cycles_accepted: u128 = pctq.cycles - msg_cycles_refunded128();
            
            let cycles_transfer_purchase_log = CyclesTransferPurchaseLog {
                canister: pctq.canister,
                cycles_sent: pctq.cycles,
                cycles_accepted: cycles_accepted,
                cycles_transfer: pctq.cycles_transfer,
                timestamp: time(),
            };

            USERS_DATA.with(|ud| {
                let users_data: &mut HashMap<Principal, UserData> = &mut ud.borrow_mut();
                let user_data: &mut UserData = &mut users_data.get_mut(&user).unwrap();

                user_data.cycles_balance -= cycles_accepted + CYCLES_TRANSFER_FEE;

                user_data.cycles_transfer_purchases.push(cycles_transfer_purchase_log.clone());
            });

            return Ok(cycles_transfer_purchase_log);
        },
        Err(cycles_transfer_call_error) => {
            match cycles_transfer_call_error.0 {
                RejectionCode::DestinationInvalid | RejectionCode::CanisterReject | RejectionCode::CanisterError => {
                    USERS_DATA.with(|ud| { ud.borrow_mut().get_mut(&user).unwrap().cycles_balance -= CYCLES_TRANSFER_FEE; });
                    return Err(PurchaseCyclesTransferError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: true });
                },
                _ => {
                    return Err(PurchaseCyclesTransferError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: false });
                }
            }
        }
    }
}















#[derive(CandidType, Deserialize, Copy, Clone, serde::Serialize)]
pub struct CyclesBankPurchaseLog {
    pub cycles_bank_principal: Principal,
    pub cost_cycles: Cycles,
    pub timestamp: u64,
    // cycles-bank-module_hash?
}



#[derive(CandidType, Deserialize)]
pub enum CyclesPaymentOrIcpPayment {
    cycles_payment,
    icp_payment
}

#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesBankQuest {
    cycles_payment_or_icp_payment: CyclesPaymentOrIcpPayment,
}

#[derive(CandidType, Deserialize)]
pub enum PurchaseCyclesBankError {
    CheckUserCyclesBalanceError(CheckUserCyclesBalanceError),
    CyclesBalanceTooLow { current_user_cycles_balance: u128, current_cycles_bank_cost_cycles: u128 },
    IcpCheckBalanceCallError(String),
    CmcGetRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    IcpBalanceTooLow { current_user_icp_balance: IcpTokens, current_cycles_bank_cost_icp: IcpTokens, current_icp_payment_ledger_transfer_fee: IcpTokens },
    CreateCyclesBankCanisterError(GetNewCanisterError),
    UninstallCodeCallError(String),
    NoCyclesBankCode,
    PutCodeCallError(String),
    CanisterStatusCallError(String),
    CheckModuleHashError{canister_status_record_module_hash: Option<[u8; 32]>, cbc_module_hash: [u8; 32]},
    StartCanisterCallError(String),
    PutCyclesCallError(String),
    UpdateSettingsCallError(String),


}

#[update]
pub async fn purchase_cycles_bank(q: PurchaseCyclesBankQuest) -> Result<CyclesBankPurchaseLog, PurchaseCyclesBankError> {
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    let user: Principal = caller();
    check_lock_and_lock_user(&user);

    let mut cycles_bank_cost_icp: Option<IcpTokens> = None;

    match q.cycles_payment_or_icp_payment {
        
        CyclesPaymentOrIcpPayment::cycles_payment => {
            
            let user_cycles_balance: u128 = match check_user_cycles_balance(&user).await {
                Ok(cycles) => cycles,
                Err(check_user_cycles_balance_error) => {
                    unlock_user(&user);
                    return Err(PurchaseCyclesBankError::CheckUserCyclesBalanceError(check_user_cycles_balance_error));
                }
            };
            
            if user_cycles_balance < CYCLES_BANK_COST {
                unlock_user(&user);
                return Err(PurchaseCyclesBankError::CyclesBalanceTooLow{ current_user_cycles_balance: user_cycles_balance, current_cycles_bank_cost_cycles: CYCLES_BANK_COST });
            }
        },
        
        CyclesPaymentOrIcpPayment::icp_payment => {
            
            let user_icp_balance: IcpTokens = match check_user_icp_balance(&user).await {
                Ok(icp_tokens) => icp_tokens,
                Err(balance_call_error) => {
                    unlock_user(&user);
                    return Err(PurchaseCyclesBankError::IcpCheckBalanceCallError(format!("{:?}", balance_call_error)));
                }
            };
            let xdr_permyriad_per_icp: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate().await {
                Ok(rate) => rate,
                Err(check_current_rate_error) => {
                    unlock_user(&user);
                    return Err(PurchaseCyclesBankError::CmcGetRateError(check_current_rate_error));    
                }
            };
            cycles_bank_cost_icp = Some(cycles_to_icptokens(CYCLES_BANK_COST, xdr_permyriad_per_icp));
            if user_icp_balance < cycles_bank_cost_icp.unwrap() + ICP_LEDGER_TRANSFER_DEFAULT_FEE { // ledger fee for the icp-transfer from user subaccount to cts main
                unlock_user(&user);
                return Err(PurchaseCyclesBankError::IcpBalanceTooLow{ 
                    current_user_icp_balance: user_icp_balance, 
                    current_cycles_bank_cost_icp: cycles_bank_cost_icp.unwrap(), 
                    current_icp_payment_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE 
                });
            }
        }
    }

    // change to a create with the ledger_canister
    let cycles_bank_principal: Principal = match get_new_canister().await {
        Ok(p) => p,
        Err(e) => {
            unlock_user(&user);
            return Err(PurchaseCyclesBankError::CreateCyclesBankCanisterError(e));
        }
    };
            
    // on errors after here make sure to put the cycles-bank-canister into the NEW_CANISTERS list 

    // install code

    let uninstall_code_call: CallResult<()> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "uninstall_code",
        (CanisterIdRecord { canister_id: cycles_bank_principal },),
    ).await; 
    match uninstall_code_call {
        Ok(_) => {},
        Err(uninstall_code_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::UninstallCodeCallError(format!("{:?}", uninstall_code_call_error)));
        }
    }
    
    if CYCLES_BANK_CANISTER_CODE.with(|cbc_refcell| { (*cbc_refcell.borrow()).module().len() == 0 }) {
        unlock_user(&user);
        NEW_CANISTERS.with(|ncs| {
            ncs.borrow_mut().push(cycles_bank_principal);
        });
        return Err(PurchaseCyclesBankError::NoCyclesBankCode);
    }

    let cbc_module_pointer: *const Vec<u8> = CYCLES_BANK_CODE.with(|cbc_refcell| {
        (*cbc_refcell.borrow()).module() as *const Vec<u8>
    });

    let put_code_call: CallResult<()> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "install_code",
        (ManagementCanisterInstallCodeQuest {
            mode : ManagementCanisterInstallCodeMode::install,
            canister_id : cycles_bank_principal,
            wasm_module : unsafe { &*cbc_module_pointer },
            arg : &encode_one(vec![ic_cdk::api::id()]).unwrap() // for now the cycles-bank takes controllers in the init
        },),
    ).await;   
    match put_code_call {
        Ok(_) => {},
        Err(put_code_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::PutCodeCallError(format!("{:?}", put_code_call_error)));
        }
    }

    // check canister status
    let canister_status_call: CallResult<(ManagementCanisterCanisterStatusRecord,)> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "canister_status",
        (CanisterIdRecord { canister_id: cycles_bank_principal },),
    ).await;
    let canister_status_record: ManagementCanisterCanisterStatusRecord = match canister_status_call {
        Ok((canister_status_record,)) => canister_status_record,
        Err(canister_status_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::CanisterStatusCallError(format!("{:?}", canister_status_call_error)));
        }
    };

    // check the wasm hash of the canister
    if canister_status_record.module_hash.is_none() || canister_status_record.module_hash.unwrap() != with(&CYCLES_BANK_CODE, |cbc| *cbc.module_hash()) {
        unlock_user(&user);
        with_mut(&NEW_CANISTERS, |ncs| {
            ncs.push(cycles_bank_principal);
        });
        return Err(PurchaseCyclesBankError::CheckModuleHashError{canister_status_record_module_hash: canister_status_record.module_hash, cbc_module_hash: CYCLES_BANK_CODE.with(|cbc_refcell| { *(*cbc_refcell.borrow()).module_hash() }) });
    }

    // check the running status
    if canister_status_record.status != ManagementCanisterCanisterStatusVariant::running {

        // start canister
        let start_canister_call: CallResult<()> = call(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "start_canister",
            (CanisterIdRecord { canister_id: cycles_bank_principal },),
        ).await;
        match start_canister_call {
            Ok(_) => {},
            Err(start_canister_call_error) => {
                unlock_user(&user);
                NEW_CANISTERS.with(|ncs| {
                    ncs.borrow_mut().push(cycles_bank_principal);
                });
                return Err(PurchaseCyclesBankError::StartCanisterCallError(format!("{:?}", start_canister_call_error)));
            }
        }
    }

    if canister_status_record.cycles < 500_000_000_000 {
        // put some cycles
        let put_cycles_call: CallResult<()> = call_with_payment128(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "deposit_cycles",
            (CanisterIdRecord { canister_id: cycles_bank_principal },),
            500_000_000_000 - canister_status_record.cycles
        ).await;
        match put_cycles_call {
            Ok(_) => {},
            Err(put_cycles_call_error) => {
                unlock_user(&user);
                NEW_CANISTERS.with(|ncs| {
                    ncs.borrow_mut().push(cycles_bank_principal);
                });
                return Err(PurchaseCyclesBankError::PutCyclesCallError(format!("{:?}", put_cycles_call_error)));
            }
        }
    }

    // change canister controllers
    let update_settings_call: CallResult<()> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "update_settings",
        (ChangeCanisterSettingsRecord { 
            canister_id: cycles_bank_principal,
            settings: ManagementCanisterOptionalCanisterSettings {
                controllers: Some(vec![user, cycles_bank_principal]),
                compute_allocation : None,
                memory_allocation : None,
                freezing_threshold : None
            }
        },),
    ).await;
    match update_settings_call {
        Ok(_) => {},
        Err(update_settings_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::UpdateSettingsCallError(format!("{:?}", update_settings_call_error)));
        }
    }

    // // sync_controllers-method on the cycles-bank
    // // let the user call with the frontend to sync the controllers?
    // let sync_controllers_call: CallResult<Vec<Principal>> = call(
    //     cycles_bank_principal,
    //     "sync_controllers",
    //     (,),
    // ).await;
    // match sync_controllers_call {
    //     Ok(synced_controllers) => {},
    //     Err(sync_controllers_call_error) => {

    //     }
    // }

    // make the cycles-bank-purchase-log
    let cycles_bank_purchase_log = CyclesBankPurchaseLog {
        cycles_bank_principal,
        cost_cycles: CYCLES_BANK_COST,
        timestamp: time(),
    };

    // log the cycles-bank-purchase-log within the USERS_DATA.with-closure and collect the icp or cycles cost within the USERS_DATA.with-closure
    with_mut(&USERS_DATA, |users_data| {
        let user_data: &mut UserData = users_data.get_mut(&user).unwrap();

        user_data.cycles_bank_purchases.push(cycles_bank_purchase_log);
        
        match q.cycles_payment_or_icp_payment {   
            CyclesPaymentOrIcpPayment::cycles_payment => {
                user_data.cycles_balance -= CYCLES_BANK_COST;
            },
            CyclesPaymentOrIcpPayment::icp_payment => {
                user_data.untaken_icp_to_collect += cycles_bank_cost_icp.unwrap() + ICP_LEDGER_TRANSFER_DEFAULT_FEE;
            }
        }
    });

    unlock_user(&user);
    Ok(cycles_bank_purchase_log)
}

*/






