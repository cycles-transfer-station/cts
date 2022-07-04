// this canister can safe stop before upgrade

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
            call::{
                msg_cycles_accept128,
                msg_cycles_available128,
                msg_cycles_refunded128,
                RejectionCode,
                reject,
                reply,
                CallResult,
                arg_data,
                call
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
        CyclesTransfer,
        CyclesTransferMemo,
        UserId,
        UserCanisterId,
        UsersMapCanisterId,
        user_canister::{
            UserCanisterInit,
            CTSCyclesTransferIntoUser,
            UserTransferCyclesQuest,
            CyclesTransferPurchaseLogId,
            CTSUserTransferCyclesCallbackQuest,
            CTSUserTransferCyclesCallbackError,
        
        },
        users_map_canister::{
            self,
            UCUserTransferCyclesQuest,
            UCUserTransferCyclesError
            
        }
    },
    consts::{
        WASM_PAGE_SIZE_BYTES
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
    global_allocator_counter::get_allocated_bytes_count
};





// cycles transfer purchases that are complete dont need an id.

#[derive(CandidType, Deserialize, Clone)]
struct UserData {

    cycles_balance: Cycles,
    cycles_transfers_in: Vec<CTSCyclesTransferIntoUser>,
    cycles_transfers_out: Vec<CyclesTransferPurchaseLog>,
    icp_transfers_in: Vec<IcpBlockHeight>,
    icp_transfers_out: Vec<IcpBlockHeight>,
    
}

impl UserData {
    fn new() -> Self {
        Self {
            cycles_balance: 0u128,
            cycles_transfers_in: Vec::new(),
            cycles_transfers_out: Vec::new(),
            icp_transfers_in: Vec::new(),
            icp_transfers_out: Vec::new(),

        }
    }
}



pub const CYCLES_TRANSFER_FEE: Cycles = /*TEST-VALUE*/10; //100_000_000_000;

pub const ICP_PAYOUT_FEE: IcpTokens = IcpTokens::from_e8s(30000);// calculate through the xdr conversion rate ? // 100_000_000_000-cycles

pub const CONVERT_ICP_FOR_THE_CYCLES_WITH_THE_CMC_RATE_FEE: Cycles = /*TEST-VALUE*/1; // 100_000_000_000


pub const CYCLES_BANK_COST: Cycles = 1; // 10_000_000_000_000;
pub const CYCLES_BANK_UPGRADE_COST: Cycles = 5; // 5_000_000_000_000;


const USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE: usize = 32;
const MINIMUM_USER_TRANSFER_CYCLES: Cycles = 1u128;
const USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE: usize = 500usize;
const USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE: usize = 500usize;




const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;



thread_local! {
    static CTS_ID:      Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static UMC_ID:      Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static USER_ID:     Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static USER_ICP_ID: Cell<Option<IcpId>>                     = Cell::new(None); // option cause no way to const an IcpId while checking the crc32 checksum
    static USER_CANISTER_MAX_MEMORY_SIZE: Cell<u64>             = Cell::new(1024*1024*100); // starting at a 100mb-limit 
    static USER_CANISTER_CREATION_TIMESTAMP_NANOS: Cell<u64>    = Cell::new(0); // is with the set in the canister_init
    
    static USER_DATA: RefCell<UserData>                         = RefCell::new(UserData::new());    
    static CYCLES_TRANSFER_PURCHASE_LOG_ID_COUNTER: Cell<u64>   = Cell::new(0);
    static ONGOING_USER_TRANSFER_CYCLES: RefCell<HashMap<CyclesTransferPurchaseLogId, CyclesTransferPurchaseLog>> = RefCell::new(HashMap::new());

    // not save in a UCData
    static     STOP_CALLS: Cell<bool> = Cell::new(false);
    static     STATE_SNAPSHOT_UC_DATA_CANDID_BYTES: RefCell<Vec<u8>> = RefCell::new(Vec::new());

}



#[init]
fn init(user_canister_init: UserCanisterInit) {
    CTS_ID.with(|cts_id| { cts_id.set(user_canister_init.cts_id); });
    UMC_ID.with(|umc_id| { umc_id.set(user_canister_init.umc_id); });
    USER_ID.with(|user_id| { user_id.set(user_canister_init.user_id); });
    USER_ICP_ID.with(|user_icp_id| { user_icp_id.set(Some(cts_lib::tools::user_icp_id(&user_canister_init.cts_id, &user_canister_init.user_id))); });
    USER_CANISTER_CREATION_TIMESTAMP_NANOS.with(|user_canister_creation_timestamp_nanos| { user_canister_creation_timestamp_nanos.set(time()); });
}



/*
#[derive(CandidType, Deserialize)]
struct OldUCData {
    cts_id: Principal,
    umc_id: UsersMapCanisterId,
    user_id: UserId,
    user_icp_id: Option<IcpId>,
    user_canister_max_memory_size: usize,
    user_canister_creation_timestamp_nanos: u64,
    user_data: OldUserData,
    cycles_transfer_purchase_log_id_counter: u64,
    ongoing_user_transfer_cycles: Vec<(CyclesTransferPurchaseLogId, OldCyclesTransferPurchaseLog)>
}



#[derive(CandidType, Deserialize, Clone)]
struct OldUserData {

    cycles_balance: Cycles,
    cycles_transfers_in: Vec<CTSCyclesTransferIntoUser>,
    cycles_transfers_out: Vec<OldCyclesTransferPurchaseLog>,
    icp_transfers_in: Vec<IcpBlockHeight>,
    icp_transfers_out: Vec<IcpBlockHeight>,
    
}

#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub struct OldCyclesTransferPurchaseLog {
    pub canister_id: Principal,
    pub cycles_sent: Cycles,
    pub cycles_accepted: Option<Cycles>, // option cause this field is only filled in the callback and that might not come back because of the callee holding-back the callback cross-upgrades. // if/when a user deletes some CyclesTransferPurchaseLogs, let the user set a special flag to delete the still-not-come-back-user_transfer_cycles by default unset.
    pub cycles_transfer_memo: OldCyclesTransferMemo,
    pub timestamp_nanos: u64, // time sent
    pub call_error: Option<(u32/*reject_code*/, String/*reject_message*/)>, // None means the cycles_transfer-call replied.
    pub fee_paid: u64
}


#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub enum OldCyclesTransferMemo {
    Nat64(u64),
    Text(String),
    Blob(Vec<u8>)
}

fn old_as_a_new_cycles_transfer_purchase_log(old_cycles_transfer_purchase_log: OldCyclesTransferPurchaseLog) -> CyclesTransferPurchaseLog {
    CyclesTransferPurchaseLog{
        canister_id: old_cycles_transfer_purchase_log.canister_id,
        cycles_sent: old_cycles_transfer_purchase_log.cycles_sent,
        cycles_accepted: old_cycles_transfer_purchase_log.cycles_accepted,
        cycles_transfer_memo: match old_cycles_transfer_purchase_log.cycles_transfer_memo {
            OldCyclesTransferMemo::Blob(b) => CyclesTransferMemo::Blob(b),
            OldCyclesTransferMemo::Text(t) => CyclesTransferMemo::Text(t),
            OldCyclesTransferMemo::Nat64(n) => CyclesTransferMemo::Nat(n as u128)
        },
        timestamp_nanos: old_cycles_transfer_purchase_log.timestamp_nanos,
        call_error: old_cycles_transfer_purchase_log.call_error,
        fee_paid: old_cycles_transfer_purchase_log.fee_paid
    }
} 
*/





#[derive(CandidType, Deserialize)]
struct UCData {
    cts_id: Principal,
    umc_id: UsersMapCanisterId,
    user_id: UserId,
    user_icp_id: Option<IcpId>,
    user_canister_max_memory_size: u64,
    user_canister_creation_timestamp_nanos: u64,
    user_data: UserData,
    cycles_transfer_purchase_log_id_counter: u64,
    ongoing_user_transfer_cycles: Vec<(CyclesTransferPurchaseLogId, CyclesTransferPurchaseLog)>
}


fn create_uc_data_candid_bytes() -> Vec<u8> {
    let mut uc_data_candid_bytes: Vec<u8> = encode_one(
        &UCData {
            cts_id:                 get(&CTS_ID),
            umc_id:  get(&UMC_ID),
            user_id:                get(&USER_ID),
            user_icp_id:            get(&USER_ICP_ID),
            user_canister_max_memory_size:              get(&USER_CANISTER_MAX_MEMORY_SIZE),
            user_canister_creation_timestamp_nanos:     get(&USER_CANISTER_CREATION_TIMESTAMP_NANOS),
            user_data: with(&USER_DATA, |user_data| { (*user_data).clone() }),
            cycles_transfer_purchase_log_id_counter:    get(&CYCLES_TRANSFER_PURCHASE_LOG_ID_COUNTER),
            ongoing_user_transfer_cycles: with(&ONGOING_USER_TRANSFER_CYCLES, |ongoing_user_transfer_cycles| { (*ongoing_user_transfer_cycles).clone().into_iter().collect::<Vec<(CyclesTransferPurchaseLogId, CyclesTransferPurchaseLog)>>() }),
        }
    ).unwrap();
    uc_data_candid_bytes.shrink_to_fit();
    uc_data_candid_bytes
}

fn re_store_uc_data_candid_bytes(uc_data_candid_bytes: Vec<u8>) {
    
    /*
    let old_uc_data: OldUCData = decode_one::<OldUCData>(&uc_data_candid_bytes).unwrap();
    
    let uc_data: UCData = UCData{
        cts_id: old_uc_data.cts_id,
        umc_id: old_uc_data.umc_id,
        user_id: old_uc_data.user_id,
        user_icp_id: old_uc_data.user_icp_id,
        user_canister_max_memory_size: old_uc_data.user_canister_max_memory_size as u64,
        user_canister_creation_timestamp_nanos: old_uc_data.user_canister_creation_timestamp_nanos,
        user_data: UserData{
            cycles_balance: old_uc_data.user_data.cycles_balance,
            cycles_transfers_in: old_uc_data.user_data.cycles_transfers_in,
            cycles_transfers_out: old_uc_data.user_data.cycles_transfers_out.into_iter().map(
                |old_cycles_transfer_purchase_log| {
                    old_as_a_new_cycles_transfer_purchase_log(old_cycles_transfer_purchase_log)
                }
            ).collect::<Vec<CyclesTransferPurchaseLog>>(),
            icp_transfers_in: old_uc_data.user_data.icp_transfers_in,
            icp_transfers_out: old_uc_data.user_data.icp_transfers_out,
        },
        cycles_transfer_purchase_log_id_counter: old_uc_data.cycles_transfer_purchase_log_id_counter,
        ongoing_user_transfer_cycles: old_uc_data.ongoing_user_transfer_cycles.into_iter().map(
            |(cycles_transfer_purchase_log_id, old_cycles_transfer_purchase_log)| {
                (cycles_transfer_purchase_log_id, old_as_a_new_cycles_transfer_purchase_log(old_cycles_transfer_purchase_log))
            }
        ).collect::<Vec<(CyclesTransferPurchaseLogId, CyclesTransferPurchaseLog)>>()
    };
    */

    let uc_data: UCData = decode_one::<UCData>(&uc_data_candid_bytes).unwrap();
    std::mem::drop(uc_data_candid_bytes);

    set(&CTS_ID, uc_data.cts_id);
    set(&UMC_ID, uc_data.umc_id);
    set(&USER_ID, uc_data.user_id);
    set(&USER_ICP_ID, uc_data.user_icp_id);
    set(&USER_CANISTER_MAX_MEMORY_SIZE, uc_data.user_canister_max_memory_size);
    set(&USER_CANISTER_CREATION_TIMESTAMP_NANOS, uc_data.user_canister_creation_timestamp_nanos);
    with_mut(&USER_DATA, |user_data| { *user_data = uc_data.user_data; });
    set(&CYCLES_TRANSFER_PURCHASE_LOG_ID_COUNTER, uc_data.cycles_transfer_purchase_log_id_counter);
    with_mut(&ONGOING_USER_TRANSFER_CYCLES, |ongoing_user_transfer_cycles| { 
        *ongoing_user_transfer_cycles = uc_data.ongoing_user_transfer_cycles.into_iter().collect::<HashMap<CyclesTransferPurchaseLogId, CyclesTransferPurchaseLog>>(); 
    });
}


#[pre_upgrade]
fn pre_upgrade() {
    let uc_upgrade_data_candid_bytes: Vec<u8> = create_uc_data_candid_bytes();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES;
    
    let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*len of the uc_upgrade_data_candid_bytes*/ + uc_upgrade_data_candid_bytes.len() as u64; 
    if current_stable_size_bytes < want_stable_memory_size_bytes {
        stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES) + 1).unwrap();
    }
    
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((uc_upgrade_data_candid_bytes.len() as u64).to_be_bytes()));
    stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &uc_upgrade_data_candid_bytes);
}

#[post_upgrade]
fn post_upgrade() {
    let mut uc_upgrade_data_candid_bytes_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut uc_upgrade_data_candid_bytes_len_u64_be_bytes);
    let uc_upgrade_data_candid_bytes_len_u64: u64 = u64::from_be_bytes(uc_upgrade_data_candid_bytes_len_u64_be_bytes); 
    
    let mut uc_upgrade_data_candid_bytes: Vec<u8> = vec![0; uc_upgrade_data_candid_bytes_len_u64 as usize]; // usize is u32 on wasm32 so careful with the cast len_u64 as usize 
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, &mut uc_upgrade_data_candid_bytes);
    
    re_store_uc_data_candid_bytes(uc_upgrade_data_candid_bytes);
    
}




// ------------------------------------
// inline always?

fn cts_id() -> Principal {
    get(&CTS_ID)
    //CTS_ID.with(|cts_id| { cts_id.get() })
}
fn umc_id() -> Principal {
    UMC_ID.with(|umc_id| { umc_id.get() })
}
fn user_id() -> Principal {
    USER_ID.with(|user_id| { user_id.get() })
}
fn user_icp_id() -> IcpId {
    USER_ICP_ID.with(|user_icp_id| { user_icp_id.get().unwrap() }) // unwrap because we put the Some(icp_id) in the canister_init
}

// ------------------------------------


fn is_canister_full() -> bool {
    // FOR THE DO!
    //false
    get_allocated_bytes_count() as u64 >= get(&USER_CANISTER_MAX_MEMORY_SIZE) /*for hashmap and vector [al]locations*/+ 1024*1024*10
}

fn get_new_cycles_transfer_purchase_log_id() -> u64 {
    CYCLES_TRANSFER_PURCHASE_LOG_ID_COUNTER.with(|counter| {
        counter.set(counter.get() + 1);
        counter.get()
    })
}
    







#[derive(CandidType, Deserialize)]
pub enum UserCyclesBalanceError {

}

#[query]
pub fn user_cycles_balance() -> Result<Cycles, UserCyclesBalanceError> {
    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    if get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    Ok(with(&USER_DATA, |user_data| {user_data.cycles_balance}))
}









#[derive(CandidType, Deserialize)]
pub enum UserIcpBalanceError {
    IcpLedgerAccountBalanceCallError(String),
}

#[update]
pub async fn user_icp_balance() -> Result<IcpTokens, UserIcpBalanceError> {
    if caller() != user_id() {
        trap("caller must be the user")
    }

    if get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    let user_icp_ledger_balance: IcpTokens = match icp_account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs{
            account: user_icp_id()
        }
    ).await {
        Ok(icp_tokens) => icp_tokens,
        Err(call_error) => return Err(UserIcpBalanceError::IcpLedgerAccountBalanceCallError(format!("{:?}", call_error)))
    };
    Ok(user_icp_ledger_balance)
}










// CTS-method
#[export_name = "canister_update cts_cycles_transfer_into_user"]
pub fn cts_cycles_transfer_into_user() {

    if caller() != cts_id() {
        trap("this is a CTS-method.")
    }
    
    if get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    if is_canister_full() {
        reject("user is full");
        return;
    }
    
    let (cycles_transfer_into_user,): (CTSCyclesTransferIntoUser,) = arg_data::<(CTSCyclesTransferIntoUser,)>();
    
    with_mut(&USER_DATA, |user_data| {
        user_data.cycles_balance += cycles_transfer_into_user.cycles;
        user_data.cycles_transfers_in.push(cycles_transfer_into_user);
    });
    
    reply::<()>(());
    return;
    
    
} 



// chunk_size = 1024*1024 / 74 = 14169.945945945947
#[export_name = "canister_query user_download_cycles_transfers_in"]
pub fn user_download_cycles_transfers_in() {
    if caller() != get(&USER_ID) {
        trap("Caller must be the user for this method.")
    }
    if get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    with(&USER_DATA, |user_data| {
        let (chunk_i,): (u32,) = arg_data::<(u32,)>(); // starts at 0
        reply::<(Option<&[CTSCyclesTransferIntoUser]>,)>((user_data.cycles_transfers_in.chunks(USER_DOWNLOAD_CYCLES_TRANSFERS_IN_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}











// for these two, make sure the fee for each purchase-type pays for the storage-cost of the Log for a certain amount of time, a year or 3 and then check the timestamp and delete expired ones or option to pay for longer storage
#[derive(CandidType, Deserialize, Clone, serde::Serialize)]
pub struct CyclesTransferPurchaseLog {
    pub canister_id: Principal,
    pub cycles_sent: Cycles,
    pub cycles_accepted: Option<Cycles>, // option cause this field is only filled in the callback and that might not come back because of the callee holding-back the callback cross-upgrades. // if/when a user deletes some CyclesTransferPurchaseLogs, let the user set a special flag to delete the still-not-come-back-user_transfer_cycles by default unset.
    pub cycles_transfer_memo: CyclesTransferMemo,
    pub timestamp_nanos: u64, // time sent
    pub call_error: Option<(u32/*reject_code*/, String/*reject_message*/)>, // None means the cycles_transfer-call replied.
    pub fee_paid: u64
}

#[derive(CandidType, Deserialize)]
pub enum UserTransferCyclesError {
    InvalidCyclesTransferMemoSize{max_size_bytes: usize},
    InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: Cycles },
    CheckUserCyclesBalanceError(UserCyclesBalanceError),
    BalanceTooLow { user_cycles_balance: Cycles, cycles_transfer_fee: Cycles },
    //CyclesTransferCallError { call_error: String, paid_fee: bool, cycles_accepted: Cycles }, // fee_paid: u128 ??
    UCUserTransferCyclesError(UCUserTransferCyclesError),
    UCUserTransferCyclesCallError(String)
}

#[update]
pub async fn user_transfer_cycles(q: UserTransferCyclesQuest) -> Result<CyclesTransferPurchaseLogId, UserTransferCyclesError> {

    if caller() != user_id() {
        trap("caller must be the user")
    }
    
    if get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    if q.cycles < MINIMUM_USER_TRANSFER_CYCLES {
        return Err(UserTransferCyclesError::InvalidTransferCyclesAmount{ minimum_user_transfer_cycles: MINIMUM_USER_TRANSFER_CYCLES });
    }
    
    let user_cycles_balance: Cycles = with(&USER_DATA, |user_data| user_data.cycles_balance);
    if q.cycles + CYCLES_TRANSFER_FEE > user_cycles_balance {
        return Err(UserTransferCyclesError::BalanceTooLow { user_cycles_balance: user_cycles_balance, cycles_transfer_fee: CYCLES_TRANSFER_FEE });
    }
    std::mem::drop(user_cycles_balance);
    
    // check memo size
    match q.cycles_transfer_memo {
        CyclesTransferMemo::Blob(ref b) => {
            if b.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes:USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE}); 
            }
        },
        CyclesTransferMemo::Text(ref b) => {
            if b.len() > USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE {
                return Err(UserTransferCyclesError::InvalidCyclesTransferMemoSize{max_size_bytes:USER_TRANSFER_CYCLES_MEMO_BYTES_MAXIMUM_SIZE}); 
            }
        }
        _ => () 
    }
    

    // take the user-cycles before the transfer, and refund in the callback 
    with_mut(&USER_DATA, |user_data| {
        user_data.cycles_balance -= q.cycles + CYCLES_TRANSFER_FEE;
    });
    let cycles_transfer_purchase_log_id: u64 = get_new_cycles_transfer_purchase_log_id(); 
    with_mut(&ONGOING_USER_TRANSFER_CYCLES, |ongoing_user_transfer_cycles| {
        ongoing_user_transfer_cycles.insert(
            cycles_transfer_purchase_log_id,
            CyclesTransferPurchaseLog{
                canister_id: q.canister_id,
                cycles_sent: q.cycles,
                cycles_accepted: None,   // None means the cycles_transfer-call-callback did not come back yet(did not give-back a reply-or-reject-sponse) 
                cycles_transfer_memo: q.cycles_transfer_memo.clone(),
                timestamp_nanos: time(), // time sent
                call_error: None,
                fee_paid: CYCLES_TRANSFER_FEE as u64
            }
        );
    });
    
    let q_cycles: Cycles = q.cycles; // copy cause want the value to stay on the stack for the closure to run with it. after the q is move into the candid params
    
    let cancel_user_transfer_cycles = || {
        with_mut(&USER_DATA, |user_data| {
            user_data.cycles_balance += q_cycles + CYCLES_TRANSFER_FEE;
        });
        with_mut(&ONGOING_USER_TRANSFER_CYCLES, |ongoing_user_transfer_cycles| {
            ongoing_user_transfer_cycles.remove(&cycles_transfer_purchase_log_id);
        })
    };
        
    match call::<(UCUserTransferCyclesQuest,), (Result<(), UCUserTransferCyclesError>,)>(
        umc_id(),
        "uc_user_transfer_cycles",
        (UCUserTransferCyclesQuest{
            user_id: user_id(),
            cycles_transfer_purchase_log_id: cycles_transfer_purchase_log_id,
            user_transfer_cycles_quest: q,            // move
        },)
    ).await { // it is possible that this callback will be called after the cts calls the cts_user_transfer_cycles_callback
        Ok((uc_user_transfer_cycles_sponse,)) => match uc_user_transfer_cycles_sponse {
            Ok(()) => return Ok(cycles_transfer_purchase_log_id),
            Err(uc_user_transfer_cycles_error) => {
                // error here means the cycles-transfer call wasn't schedule
                // make-vanish the log 
                //fn cancel_user_cycles_transfer(cycles_transfer_purchase_log_id: CyclesTransferPurchaseLogId, q_cycles: Cycles) {
                //    with_mut(&USER_DATA, |user_data| {
                //        user_data.cycles_balance += q_cycles + CYCLES_TRANSFER_FEE;
                //        user_data.cycles_transfer_purchases.remove(&cycles_transfer_purchase_log_id);
                //    });
                //}
                //cancel_user_cycles_transfer(cycles_transfer_purchase_log_id, q_cycles);
                cancel_user_transfer_cycles();
                return Err(UserTransferCyclesError::UCUserTransferCyclesError(uc_user_transfer_cycles_error));
            }
        }, 
        Err(uc_user_transfer_cycles_call_error) => {
            //cancel_user_cycles_transfer(cycles_transfer_purchase_log_id, q_cycles);
            cancel_user_transfer_cycles();
            return Err(UserTransferCyclesError::UCUserTransferCyclesCallError(format!("{:?}", uc_user_transfer_cycles_call_error)));
        },
    }
    
}





#[update]
pub fn cts_user_transfer_cycles_callback(cts_q: CTSUserTransferCyclesCallbackQuest) -> Result<(), CTSUserTransferCyclesCallbackError> {
    
    if caller() != cts_id() {
        trap("caller must be the cts for this method.")
    }
    
    if get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    if cts_q.user_id != user_id() {
        return Err(CTSUserTransferCyclesCallbackError::WrongUserId)
    }

    let opt_cycles_transfer_purchase_log: Option<CyclesTransferPurchaseLog> = with_mut(&ONGOING_USER_TRANSFER_CYCLES, |ongoing_user_transfer_cycles| { 
        ongoing_user_transfer_cycles.remove(&cts_q.cycles_transfer_purchase_log_id)
    });
    
    if let Some(mut cycles_transfer_purchase_log) = opt_cycles_transfer_purchase_log {
        let mut give_back_the_fee: u64 = 0;
        if let Some(ref call_error) = cts_q.cycles_transfer_call_error {
            if let  0 | 1 | 2 = (*call_error).0 {        
                give_back_the_fee = cycles_transfer_purchase_log.fee_paid;
                cycles_transfer_purchase_log.fee_paid = 0u64;
            }
        }
        cycles_transfer_purchase_log.call_error = cts_q.cycles_transfer_call_error;
        cycles_transfer_purchase_log.cycles_accepted = Some(cycles_transfer_purchase_log.cycles_sent - cts_q.cycles_transfer_refund);
        
        with_mut(&USER_DATA, |user_data| {
            user_data.cycles_transfers_out.push(cycles_transfer_purchase_log); 
            user_data.cycles_balance += cts_q.cycles_transfer_refund + (give_back_the_fee as Cycles);     
       });
    }       
    
    Ok(())
}







#[export_name = "canister_query user_download_cycles_transfers_out"]
pub fn user_download_cycles_transfers_out() {
    if caller() != get(&USER_ID) {
        trap("Caller must be the user for this method.")
    }
    if get(&STOP_CALLS) { trap("Maintenance. try again soon.") }
    
    with(&USER_DATA, |user_data| {
        let (chunk_i,): (u32,) = arg_data::<(u32,)>(); // starts at 0
        reply::<(Option<&[CyclesTransferPurchaseLog]>,)>((user_data.cycles_transfers_out.chunks(USER_DOWNLOAD_CYCLES_TRANSFERS_OUT_CHUNK_SIZE).nth(chunk_i as usize),));
    });
}







// -----------------------------------------------------------------------------------








#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the CTS for this method.")
    }
    set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the CTS for this method.")
    }
    get(&STOP_CALLS)
}





#[update]
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot_candid_bytes*/ {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the CTS for this method.")
    }
    with_mut(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        *state_snapshot_uc_data_candid_bytes = create_uc_data_candid_bytes();
        state_snapshot_uc_data_candid_bytes.len() as u64
    })
}



// chunk_size = 1mib


#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the CTS for this method.")
    }
    let chunk_size: usize = 1024*1024;
    with(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        let (chunk_i,): (u32,) = arg_data::<(u32,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot_uc_data_candid_bytes.chunks(chunk_size).nth(chunk_i as usize),));
    })

}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the CTS for this method.")
    }
    with_mut(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        *state_snapshot_uc_data_candid_bytes = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot_candid_bytes(mut append_bytes: Vec<u8>) {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the CTS for this method.")
    }
    with_mut(&STATE_SNAPSHOT_UC_DATA_CANDID_BYTES, |state_snapshot_uc_data_candid_bytes| {
        state_snapshot_uc_data_candid_bytes.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_re_store_uc_data_out_of_the_state_snapshot() {
    if caller() != get(&CTS_ID) {
        trap("Caller must be the CTS for this method.")
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
pub struct UCMetrics {
    
}


#[query]
pub fn cts_see_metrics() -> UCMetrics {
    UCMetrics{
        
    }
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






