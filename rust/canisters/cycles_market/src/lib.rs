use std::cell::{Cell, RefCell};
use cts_lib::{
    tools::{
        localkey::{
            self,
            refcell::{with, with_mut}
        }
    },
    types::{
        XdrPerMyriadPerIcp
    },
    ic_cdk::{
        api::{
            id as cycles_market_canister_id,
            call,
            trap,
            
        },
        export::{
            Principal,
            candid::{
                self, 
                utils::{encode_one, decode_one}
            }
        }
    },
    ic_cdk_macros::{
        update,
        query,
    }
};


// on a cycles-payout, the cycles-market will try once to send the cycles with a cycles_transfer-method call and if it fails, the cycles-market will use the deposit_cycles management canister method and close the position.

// make sure the positors and purchaser are secret and hidden. public-data is the position-id, the commodity, the minimum purchase, and the rate, (and the timestamp? no that makes it traceable)

type PositionId = u128;
type PurchaseId = u128;


struct CyclesPosition {
    id: PositionId,   
    positor: Principal,
    cycles: Cycles,
    minimum_purchase: Cycles
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    timestamp_nanos: u64,
}

struct IcpPosition {
    id: PositionId,   
    positor: Principal,
    icp: IcpTokens,
    minimum_purchase: IcpTokens
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    timestamp_nanos: u64,
}


struct CyclesPositionPurchase {
    cycles_position_id: PositionId,
    cycles_position_positor: Principal,
    cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    id: PurchaseId,
    purchaser: Principal,
    cycles: Cycles,
    timestamp_nanos: u64,
    lock: bool, // lock for the payouts
    cycles_payout_data: CyclesPayoutData,
    icp_payout: bool
}

struct IcpPositionPurchase {
    icp_position_id: PositionId,
    icp_position_positor: Principal,
    icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    id: PurchaseId,
    purchaser: Principal,
    icp: IcpTokens,
    timestamp_nanos: u64,
    lock: bool, // lock for the payouts
    cycles_payout_data: CyclesPayoutData,
    icp_payout: bool
}



#[derive(Clone)]
struct CyclesPayoutData {
    cycles_transferrer_transfer_cycles_call_success_timestamp_nanos: Option<u64>,
    cycles_transferrer_transfer_cycles_callback_complete: Option<(CyclesTransferRefund, Option<(u32, String)>)>,
    management_canister_posit_cycles_call_success: bool // this is use for when the payout-cycles-transfer-refund != 0, call the management_canister-deposit_cycles(payout-cycles-transfer-refund)
} 
impl CyclesPayoutData {
    fn new() -> Self {
        Self {
            cycles_transferrer_transfer_cycles_call_success_timestamp_nanos: None,
            cycles_transferrer_transfer_cycles_callback_complete: None,
            management_canister_posit_cycles_call_success: false
        }
    }
    fn is_waiting_for_the_cycles_transferrer_transfer_cycles_callback(&self) -> bool {
        self.cycles_transferrer_transfer_cycles_call_success_timestamp_nanos.is_some() 
        && self.cycles_transferrer_transfer_cycles_callback_complete.is_none()
    }
    fn is_complete(&self) -> bool {
        if let Some((cycles_transfer_refund, _)) = self.cycles_transferrer_transfer_cycles_callback_complete {
            if cycles_transfer_refund == 0 || self.management_canister_posit_cycles_call_success == true {
                return true;
            }
        }
        false
    }
}

struct VoidCyclesPosition {
    position_id: PositionId,
    positor: Principal,
    cycles: Cycles,
    lock: bool,  // lock for the payout
    cycles_payout_data: CyclesPayoutData,
    timestamp_nanos: u64
}




struct CMData {
    cts_id: Principal,
    cycles_transferrers: Vec<Principal>,
    id_counter: u128,
    mid_call_user_icp_balance_locks: HashSet<Principal>,
    cycles_positions: Vec<CyclesPosition>,
    icp_positions: Vec<IcpPosition>,
    cycles_positions_purchases: Vec<CyclesPositionPurchase>,
    icp_positions_purchases: Vec<IcpPositionPurchase>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    
}

impl CMData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            cycles_transferrers: Vec::new(),
            id_counter: 0,
            mid_call_user_icp_balance_locks: HashSet::new(),
            cycles_positions: Vec::new(),
            icp_positions: Vec::new(),
            cycles_positions_purchases: Vec::new(),
            icp_positions_purchases: Vec::new(),
            void_cycles_positions: Vec::new(),
        }
    }
}



pub const CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;

pub const TRANSFER_ICP_BALANCE_FEE: Cycles = 50_000_000_000;

pub const CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

pub const MAX_WAIT_TIME_NANOS_FOR_A_CYCLES_TRANSFERRER_TRANSFER_CYCLES_CALLBACK: u64 = 1_000_000_000*60*60*10;

pub const MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP: Cycles = 20_000_000_000_000;
pub const MINIMUM_CYCLES_POSITION: Cycles = 5_000_000_000_000;

pub const MINIMUM_ICP_POSITION_FOR_AN_ICP_POSITION_BUMP: IcpTokens = IcpTokens::from_e8s(200000000);
pub const MINIMUM_ICP_POSITION: IcpTokens = IcpTokens::from_e8s(50000000);



const CANISTER_NETWORK_MEMORY_ALLOCATION_MiB: u64 = 500; // multiple of 10
const CANISTER_DATA_STORAGE_SIZE_MiB = CANISTER_NETWORK_MEMORY_ALLOCATION_MiB / 2 - 20/*memory-size at the start [re]placement*/; // multiple of 5 

const CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: u64 = CANISTER_DATA_STORAGE_SIZE_MiB / 5 * 1;
const MAX_CYCLES_POSITIONS: usize = ( CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<CyclesPosition>() as u64 ) as usize;

const ICP_POSITIONS_MAX_STORAGE_SIZE_MiB: u64 = CANISTER_DATA_STORAGE_SIZE_MiB / 5 * 1;
const MAX_ICP_POSITIONS: usize = ( ICP_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<IcpPosition>() as u64 ) as usize;

const ICP_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB: u64 = CANISTER_DATA_STORAGE_SIZE_MiB / 5 * 1;
const MAX_ICP_POSITIONS_PURCHASES: usize = ( ICP_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<IcpPositionPurchase>() as u64 ) as usize;

const CYCLES_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB: u64 = CANISTER_DATA_STORAGE_SIZE_MiB / 5 * 1;
const MAX_CYCLES_POSITIONS_PURCHASES: usize = ( CYCLES_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<CyclesPositionPurchase>() as u64 ) as usize;

const VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: u64 = CANISTER_DATA_STORAGE_SIZE_MiB / 5 * 1;
const MAX_VOID_CYCLES_POSITIONS: usize = ( VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidCyclesPosition>() as u64 ) as usize;


const DO_VOID_CYCLES_POSITIONS_CHUNK_SIZE: usize = 500;

const VOID_POSITION_CYCLES_TRANSFER_MEMO_START: &[u8; 5] = b"CM-VP";

const MAX_MID_CALL_USER_ICP_BALANCE_LOCKS: usize = 5000;


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;


thread_local! {

    static CM_DATA: RefCell<CMData> = RefCell::new(CMData::new()); 
    
    // not save through the upgrades
    static STOP_CALLS: Cell<bool> = Cell::new(false);
    static STATE_SNAPSHOT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    
}


// -------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CMInit {
    cts_id: Principal,
    cycles_transferrers: Vec<Principal>,
} 

#[init]
fn init(cm_init: CMInit) {
    with_mut(&CM_DATA, |cm_data| { 
        cm_data.cts_id = cm_init.cts_id; 
        cm_data.cycles_transferrers = cm_init.cycles_transferrers;
    });
} 


// -------------------------------------------------------------


fn create_state_snapshot() {
    let mut cm_data_candid_bytes: Vec<u8> = with(&CM_DATA, |cm_data| { encode_one(cm_data).unwrap() });
    cm_data_candid_bytes.shrink_to_fit();
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = cm_data_candid_bytes; 
    });
}

fn load_state_snapshot_data() {
    
    let cm_data_of_the_state_snapshot: CMData = with(&STATE_SNAPSHOT, |state_snapshot| {
        match decode_one::<CMData>(state_snapshot) {
            Ok(cm_data) => cm_data,
            Err(_) => {
                trap("error decode of the state-snapshot CMData");
                /*
                let old_cm_data: OldCMData = decode_one::<OldCMData>(state_snapshot).unwrap();
                let cm_data: CMData = CMData{
                    cts_id: old_cm_data.cts_id
                    ........
                };
                cm_data
                */
            }
        }
    });

    with_mut(&CM_DATA, |cm_data| {
        *cm_data = cm_data_of_the_state_snapshot;    
    });
    
}

// -------------------------------------------------------------


#[pre_upgrade]
fn pre_upgrade() {
    
    create_state_snapshot();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;

    with(&STATE_SNAPSHOT, |state_snapshot| {
        let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*len of the state_snapshot*/ + state_snapshot.len() as u64; 
        if current_stable_size_bytes < want_stable_memory_size_bytes {
            stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
        }
        stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((state_snapshot.len() as u64).to_be_bytes()));
        stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, state_snapshot);
    });
}

#[post_upgrade]
fn post_upgrade() {
    let mut state_snapshot_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut state_snapshot_len_u64_be_bytes);
    let state_snapshot_len_u64: u64 = u64::from_be_bytes(state_snapshot_len_u64_be_bytes); 
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = vec![0; state_snapshot_len_u64 as usize]; 
        stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, state_snapshot);
    });
    
    load_state_snapshot_data();
} 


// -------------------------------------------------------------

#[no_mangle]
fn canister_inspect_message() {
    use cts_lib::ic_cdk::api::call::{method_name, accept_message};
    
    if [
        "create_cycles_position",
        "create_icp_position",
        "purchase_cycles_position",
        "purchase_icp_position",
        "void_position",
        "see_icp_lock",
        "transfer_icp_balance",
        "cycles_transferrer_transfer_cycles_callback",
    ].contains(&&method_name()[..]) {
        trap("this method must be call by a canister.");
    
    }
    
    
    accept_message();    
}


// -------------------------------------------------------------

fn cts_id() -> Principal {
    with(&CM_DATA, |cm_data| { cm_data.cts_id })
}

fn new_id() {
    with_mut(&CM_DATA, |cm_data| {
        let id: u128 = cm_data.id_counter.clone();
        cm_data.id_counter += 1;
        id
    })
}


async fn check_user_cycles_market_icp_ledger_balance(user_id: &Principal) -> CallResult<IcpTokens> {
    icp_account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs { account: user_icp_id(&cycles_market_canister_id(), user_id) }    
    ).await
}


fn check_user_icp_balance_in_the_lock(user_id: &Principal) -> IcpTokens {
    with(&CM_DATA, |cm_data| {
        cm_data.icp_positions.iter()
            .filter(|icp_position: &&IcpPosition| { icp_position.positor == *user_id })
            .fold(IcpTokens::from_e8s(0), |cummulator: IcpTokens, user_icp_position: &IcpPosition| {
                cummulator + user_icp_position.icp + ( user_icp_position.icp / user_icp_position.minumum_purchase * ICP_LEDGER_TRANSFER_DEFAULT_FEE ) 
            })
        +
        cm_data.cycles_positions_purchases.iter()
            .filter(|cycles_position_purchase: &&CyclesPositionPurchase| {
                cycles_position_purchase.purchaser == *user_id && cycles_position_purchase.icp_payout_status == false 
            })
            .fold(IcpTokens::from_e8s(0), |cummulator: IcpTokens, user_cycles_position_purchase_with_unpaid_icp: &CyclesPositionPurchase| {
                cummulator + cycles_to_icptokens(user_cycles_position_purchase_with_unpaid_icp.cycles, user_cycles_position_purchase_with_unpaid_icp.cycles_position_xdr_permyriad_per_icp_rate) + ICP_LEDGER_TRANSFER_DEFAULT_FEE
            })
        +
        cm_data.icp_positions_purchases.iter()
            .filter(|icp_position_purchase: &&IcpPositionPurchase| {
                icp_position_purchase.icp_position_positor == *user_id && icp_position_purchase.icp_payout_status == false 
            })
            .fold(IcpTokens::from_e8s(0), |cummulator: IcpTokens, icp_position_purchase_with_the_user_as_the_positor_with_unpaid_icp: &IcpPositionPurchase| {
                cummulator + icp_position_purchase_with_the_user_as_the_positor_with_unpaid_icp.icp + ICP_LEDGER_TRANSFER_DEFAULT_FEE
            })
    })
}



async fn do_cycles_payout(cycles_transferrer_transfer_cycles_quest: cycles_transferrer::TransferCyclesQuest, mut cycles_payout_data: CyclesPayoutData) -> CyclesPayoutData {
    
    if cycles_payout_data.cycles_transferrer_transfer_cycles_call_success_timestamp_nanos.is_none() {
        let cycles_for_the_cycles_transferrer_transfer_cycles_call: Cycles = cycles_transferrer_transfer_cycles_quest.cycles + CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE;
        match call_with_payment128::<(cycles_transferrer::TransferCyclesQuest,),(Result<(), cycles_transferrer::TransferCyclesError>,)>(
            cycles_transferrer_round_robin(),
            "transfer_cycles",
            (cycles_transferrer_transfer_cycles_quest,),
            cycles_for_the_cycles_transferrer_transfer_cycles_call
        ).await {
            Ok(_) => {
                cycles_payout_data.cycles_transferrer_transfer_cycles_call_success_timestamp_nanos = Some(time());
            }, 
            Err(cycles_transferrer_transfer_cycles_error) => {}
        }
        return cycles_payout_data;
    }
    
    if let Some((cycles_transfer_refund, _)) = cycles_payout_data.cycles_transferrer_transfer_cycles_callback_complete {
        if cycles_transfer_refund != 0 {
            if cycles_payout_data.management_canister_posit_cycles_call_success == false {
                match call_with_payment128::<(management_canister::CanisterIdRecord,),()>(
                    MANAGEMENT_CANISTER_ID,
                    "deposit_cycles",
                    (management_canister::CanisterIdRecord{
                        canister_id: cycles_transferrer_transfer_cycles_quest.for_the_canister
                    },),
                    cycles_transfer_refund
                ).await {
                    Ok(_) => {
                        cycles_payout_data.management_canister_posit_cycles_call_success = true;
                    },
                    Err(_) => {},
                }            
            }
        }
    }
    
    return cycles_payout_data;
}

async fn do_void_cycles_positions() {
    
    if with(&CM_DATA, |cm_data| { cm_data.void_cycles_positions.len() == 0 { return; }
    
    let void_cycles_positions_chunk: Vec<(VoidCyclesPositionId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new();
    
    with_mut(&CM_DATA, |cm_data| {
        let mut i = 0;
        while i < cm_data.void_cycles_positions.len() && void_cycles_positions_chunk.len() < DO_VOID_CYCLES_POSITIONS_CHUNK_SIZE {
            let vcp: &mut VoidCyclesPosition = &mut cm_data.void_cycles_positions[i];
            if vcp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() {
                if time().saturating_sub(vcp.cycles_transferrer_transfer_cycles_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CYCLES_TRANSFERRER_TRANSFER_CYCLES_CALLBACK {
                    std::mem::drop(vcp);
                    cm_data.void_cycles_positions.remove(i);
                    continue;
                }
                // skip
            } else if vcp.lock == true { 
                // skip
            } else {
                vcp.lock = true;
                void_cycles_positions_chunk.push(
                    (
                        vcp.position_id,
                        do_cycles_payout(
                            cycles_transferrer::TransferCyclesQuest{
                                user_cycles_transfer_id: vcp.position_id,
                                for_the_canister: vcp.positor,
                                cycles: vcp.cycles,
                                cycles_transfer_memo: CyclesTransferMemo::Blob([VOID_POSITION_CYCLES_TRANSFER_MEMO_START, vcp.position_id.to_be_bytes()].concat().to_vec()),
                            },
                            vcp.cycles_payout_data.clone()
                        )                     
                    )  
                );                 
            }
            i += 1;
        }
    });
    
    let (vcps_ids, do_cycles_payouts_futures): (Vec<VoidCyclesPositionId>, Vec<_/*anonymous-future of the do_cycles_payout*/>) = void_cycles_positions_chunk.into_iter().unzip();
    
    let rs: Vec<CyclesPayoutData> = futures::future::join_all(do_cycles_payouts_futures).await;
    
    let final_void_cycles_positions_chunk: Vec<(VoidCyclesPositionId, CyclesPayoutData)> = vcp_ids.into_iter().zip(rs.into_iter()).collect::<Vec<(VoidCyclesPositionId, CyclesPayoutData)>>();
    
    with_mut(&CM_DATA, |cm_data| {
        for (vcp_id, cycles_payout_data) in final_void_cycles_positions_chunk.into_iter() {
            let vcp_void_cycles_positions_i: usize = {
                match cm_data.void_cycles_positions.binary_search_by_key(&vcp_id, |vcp| { vcp.position_id }) {
                    Ok(i) => i,
                    Err(_) => continue;
                }  
            };
            if cycles_payout_data.is_complete() {
                cm_data.void_cycles_positions.remove(vcp_void_cycles_positions_i);
            } else {
                cm_data.void_cycles_positions[vcp_void_cycles_positions_i].lock = false;
                cm_data.void_cycles_positions[vcp_void_cycles_positions_i].cycles_payout_data = cycles_payout_data;
            }
        }
    });
    
}


// -------------------------------------------------------------



pub struct CreateCyclesPositionQuest {
    cycles: Cycles,
    minimum_purchase: Cycles,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcpRate,
    
}


pub enum CreateCyclesPositionError{
    MinimumPurchaseMustBeEqualOrLessThanTheCyclesPosition,
    MsgCyclesTooLow{ create_position_fee: Cycles },
    CyclesMarketIsFull,
    CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: XdrPerMyriadPerIcp, minimum_cycles_position_for_a_bump: Cycles },
    MinimumCyclesPosition(Cycles),
    
}


pub struct CreateCyclesPositionSuccess {
    position_id: PositionId,
}

#[update(manual_reply = true)]
pub async fn create_cycles_position(create_cycles_position_quest: CreateCyclesPositionQuest) { // -> Result<CreateCyclesPositionSuccess, CreateCyclesPositionError> {

    let positor: Principal = caller();

    if q.minimum_purchase > q.cycles {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumPurchaseMustBeEqualOrLessThanTheCyclesPosition),));    
        do_void_cycles_positions().await;
        return;
    }

    if q.minimum_purchase < MINIMUM_CYCLES_POSITION {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumCyclesPosition(MINIMUM_CYCLES_POSITION)),));
        do_void_cycles_positions().await;
        return;
    }


    let msg_cycles_quirement: Cycles = CREATE_POSITION_FEE.checked_add(q.cycles).unwrap_or(Cycles::MAX); 

    if msg_cycles_available128() < msg_cycles_quirement {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MsgCyclesTooLow{ create_position_fee: CREATE_POSITION_FEE  }),));
        do_void_cycles_positions().await;
        return;
    }

    if canister_balance128().checked_add(msg_cycles_quirement).is_none() {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsFull),));
        do_void_cycles_positions().await;
        return;
    }

    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.cycles_positions.len() >= MAX_CYCLES_POSITIONS {
            if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsFull),));
                return Err(());
            }
            let cycles_position_highest_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp = { 
                cm_data.cycles_positions.iter()
                    .max_by_key(|cycles_position: &&CyclesPosition| { cycles_position.xdr_permyriad_per_icp_rate })
                    .unwrap()
                    .xdr_permyriad_per_icp_rate
            };
            if q.xdr_permyriad_per_icp_rate > cycles_position_highest_xdr_permyriad_per_icp_rate && q.cycles >= MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP {
                // bump
                let cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id: PositionId = {
                    cm_data.cycles_positions.iter()
                        .min_by_key(|cycles_position: &&CyclesPosition| { cycles_position.xdr_permyriad_per_icp_rate })
                        .unwrap()
                        .id
                };
                let cycles_position_lowest_xdr_permyriad_per_icp_rate_cycles_positions_i: usize = {
                    cm_data.cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id,
                        |cycles_position| { cycles_position.id }
                    ).unwrap()
                };
                let cycles_position_lowest_xdr_permyriad_per_icp_rate: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_lowest_xdr_permyriad_per_icp_rate_cycles_positions_i);
                if cycles_position_lowest_xdr_permyriad_per_icp_rate.id != cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id { trap("outside the bounds of the contract.") }
                let cycles_position_lowest_xdr_permyriad_per_icp_rate_void_cycles_positions_insertion_i = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_lowest_xdr_permyriad_per_icp_rate_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_lowest_xdr_permyriad_per_icp_rate.id,
                        positor:        cycles_position_lowest_xdr_permyriad_per_icp_rate.positor,
                        cycles:         cycles_position_lowest_xdr_permyriad_per_icp_rate.cycles,
                        lock:           false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time()
                    }
                );
                Ok(())
            } else {
                reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: cycles_position_highest_xdr_permyriad_per_icp_rate + 1, minimum_cycles_position_for_a_bump: MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP }),));
                return Err(());
            }
        } else {
            Ok(())
        }
    }) {
        Ok(()) => {},
        Err(()) => {
            do_void_cycles_positions().await;
            return;
        }
    }
        
    let id: PositionId = new_id(); 
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.cycles_positions.push(
            CyclesPosition{
                id,   
                positor,
                cycles: q.cycles,
                minimum_purchase: q.minimum_purchase,
                xdr_permyriad_per_icp_rate: q.xdr_permyriad_per_icp_rate,
                timestamp_nanos: time(),
            }
        );
    });
    
    msg_cycles_accept128(msg_cycles_quirement);

    reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Ok(
        CreateCyclesPositionSuccess{
            position_id: id
        }
    ),));
    
    do_void_cycles_positions().await;
    return;
}




// ------------------


pub struct CreateIcpPositionQuest {
    icp: IcpTokens,
    minimum_purchase: IcpTokens,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcpRate,
}

pub struct CreateIcpPositionError {
    MinimumPurchaseMustBeEqualOrLessThanTheIcpPosition,
    MsgCyclesTooLow{ create_position_fee: Cycles },
    CyclesMarketIsFull,
    CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
    CheckUserCyclesMarketIcpLedgerBalanceError((u32, String)),
    UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
    CyclesMarketIsFull_MaximumRateAndMinimumIcpPositionForABump{ maximum_rate_for_a_bump: XdrPerMyriadPerIcp, minimum_icp_position_for_a_bump: IcpTokens },
    MinimumIcpPosition(IcpTokens),
}

pub struct CreateIcpPositionSuccess {
    position_id: PositionId

}


#[update(manual_reply = true)]
pub async fn create_icp_position(q: CreateIcpPositionQuest) { //-> Result<CreateIcpPositionSuccess,CreateIcpPositionError> {

    let positor: Principal = caller();

    if q.minimum_purchase > q.icp {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MinimumPurchaseMustBeEqualOrLessThanTheIcpPosition),));    
        do_payouts().await;
        return;
    }

    if q.minimum_purchase < MINIMUM_ICP_POSITION {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MinimumIcpPosition(MINIMUM_ICP_POSITION)),));
        do_payouts().await;
        return;
    }


    let msg_cycles_quirement: Cycles = CREATE_POSITION_FEE; 

    if msg_cycles_available128() < msg_cycles_quirement {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MsgCyclesTooLow{ create_position_fee: CREATE_POSITION_FEE  }),));
        do_payouts().await;
        return;
    }

    if canister_balance128().checked_add(msg_cycles_quirement).is_none() {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CyclesMarketIsFull),));
        do_payouts().await;
        return;
    }

    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_icp_balance_locks.len() >= MAX_MID_CALL_USER_ICP_BALANCE_LOCKS {
            reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CyclesMarketIsFull),));
            return Err(());
        }
        if cm_data.mid_call_user_icp_balance_locks.contains(&positor) {
            reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall),));
            return Err(());
        }
        cm_data.mid_call_user_icp_balance_locks.insert(positor);
        Ok(())
    }) {
        Ok(()) => {},
        Err(()) => {
            do_payouts().await;
            return;
        }
    }
    
    // check icp balance and make sure to unlock the user on returns after here 
    let user_icp_ledger_balance: IcpTokens = match check_user_cycles_market_icp_ledger_balance(&positor).await {
        Ok(icp_ledger_balance) => icp_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
            reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CheckUserCyclesMarketIcpLedgerBalanceError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    }
    
    let user_icp_balance_in_the_lock: IcpTokens = check_user_icp_balance_in_the_lock(&positor);
    
    let usable_user_icp_balance: IcpTokens = IcpTokens::from_e8s(user_icp_ledger_balance.e8s().saturating_sub(user_icp_balance_in_the_lock.e8s()));
    
    if usable_user_icp_balance < q.icp + ( q.icp / q.minimum_purchase * ICP_LEDGER_TRANSFER_DEFAULT_FEE ) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::UserIcpBalanceTooLow{ user_icp_balance: usable_user_icp_balance }),));
        do_payouts().await;
        return;
    }
    
    
    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.icp_positions.len() >= MAX_ICP_POSITIONS {            
            let icp_position_lowest_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp = { 
                cm_data.icp_positions.iter()
                    .min_by_key(|icp_position: &&IcpPosition| { icp_position.xdr_permyriad_per_icp_rate })
                    .unwrap()
                    .xdr_permyriad_per_icp_rate
            };
            if q.xdr_permyriad_per_icp_rate < icp_position_lowest_xdr_permyriad_per_icp_rate && q.icp >= MINIMUM_ICP_POSITION_FOR_AN_ICP_POSITION_BUMP {
                // bump
                let icp_position_highest_xdr_permyriad_per_icp_rate_position_id: PositionId = {
                    cm_data.icp_positions.iter()
                        .max_by_key(|icp_position: &&IcpPosition| { icp_position.xdr_permyriad_per_icp_rate })
                        .unwrap()
                        .id
                };
                let icp_position_highest_xdr_permyriad_per_icp_rate_icp_positions_i: usize = {
                    cm_data.icp_positions.binary_search_by_key(
                        &icp_position_highest_xdr_permyriad_per_icp_rate_position_id,
                        |icp_position| { icp_position.id }
                    ).unwrap()
                };
                let icp_position_highest_xdr_permyriad_per_icp_rate: IcpPosition = cm_data.icp_positions.remove(icp_position_highest_xdr_permyriad_per_icp_rate_icp_positions_i);                
                Ok(())
            } else {
                reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MaximumRateAndMinimumIcpPositionForABump{ maximum_rate_for_a_bump: icp_position_lowest_xdr_permyriad_per_icp_rate - 1, minimum_icp_position_for_a_bump: MINIMUM_ICP_POSITION_FOR_AN_ICP_POSITION_BUMP }),));
                return Err(());
            }
        } else {
            Ok(())    
        }
    }) {
        Ok(()) => {},
        Err(()) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
            do_payouts().await;
            return;
        }
    }
    
    
    let id: PositionId = new_id(); 
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.icp_positions.push(
            IcpPosition{
                id,   
                positor,
                icp: q.icp,
                minimum_purchase: q.minimum_purchase,
                xdr_permyriad_per_icp_rate: q.xdr_permyriad_per_icp_rate,
                timestamp_nanos: time(),
            }
        );
    });
    
    msg_cycles_accept128(msg_cycles_quirement);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
    
    reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Ok(
        CreateIcpPositionSuccess{
            position_id: id
        }
    ),));
    do_payouts().await;
    return;
}


// ------------------


pub struct PurchaseCyclesPositionQuest {
    cycles_position_id: PositionId,
    cycles: Cycles
}

pub enum PurchaseCyclesPositionError {
    MsgCyclesTooLow{ purchase_position_fee: Cycles },
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
    CheckUserCyclesMarketIcpLedgerBalanceError((u32, String)),
    UserIcpBalanceTooLow{ user_icp_balance: IcpTokens }
    CyclesPositionNotFound,
    CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: Cycles },
    CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: Cycles },
}

pub struct PurchaseCyclesPositionSuccess {
    purchase_id: PurchaseId,
}

pub type PurchaseCyclesPositionResult = Result<PurchaseCyclesPositionSuccess, PurchaseCyclesPositionError>;

#[update(manual_reply = true)]
pub async fn purchase_cycles_position(q: PurchaseCyclesPositionQuest) { // -> Result<PurchaseCyclesPositionSuccess, PurchaseCyclesPositionError>
    
    let purchaser: Principal = caller();
    
    if msg_cycles_available128() < PURCHASE_POSITION_FEE {
        reply::<(PurchaseCyclesPositionResult,)>((Err(PurchaseCyclesPositionError::MsgCyclesTooLow{ purchase_position_fee: PURCHASE_POSITION_FEE }),));
        do_payouts().await;
        return;
    }
    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_icp_balance_locks.len() >= MAX_MID_CALL_USER_ICP_BALANCE_LOCKS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_icp_balance_locks.contains(&purchaser) {
            return Err(PurchaseCyclesPositionError::CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall);
        }
        cm_data.mid_call_user_icp_balance_locks.insert(purchaser);
        Ok(())
    }) {
        Ok(()) => {},
        Err(purchase_cycles_position_error) => {
            reply::<(PurchaseCyclesPositionResult,)>((Err(purchase_cycles_position_error),));
            do_payouts().await;
            return;
        }
    }
    
    // check icp balance and make sure to unlock the user on returns after here 
    let user_icp_ledger_balance: IcpTokens = match check_user_cycles_market_icp_ledger_balance(&purchaser).await {
        Ok(icp_ledger_balance) => icp_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&purchaser); });
            reply::<(PurchaseCyclesPositionResult,)>((Err(PurchaseCyclesPositionError::CheckUserCyclesMarketIcpLedgerBalanceError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    }
    
    let user_icp_balance_in_the_lock: IcpTokens = check_user_icp_balance_in_the_lock(&purchaser);
    
    let usable_user_icp_balance: IcpTokens = IcpTokens::from_e8s(user_icp_ledger_balance.e8s().saturating_sub(user_icp_balance_in_the_lock.e8s()));

    let cycles_position_purchase_id: PurchaseId = match with_mut(&CM_DATA, |cm_data| {
        if cm_data.cycles_positions_purchases.len >= MAX_CYCLES_POSITIONS_PURCHASES {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
        let cycles_position_cycles_positions_i: usize = match cm_data.cycles_positions.binary_search_by_key(
            &q.cycles_position_id,
            |cycles_position| { cycles_position.id }
        ) {
            Ok(i) => i,
            Err(_) => return Err(PurchaseCyclesPositionError::CyclesPositionNotFound);
        };
        let cycles_position_ref: &CyclesPosition = &cm_data.cycles_positions[cycles_position_cycles_positions_i];
        if cycles_position_ref.cycles < q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: cycles_position_ref.cycles });
        }
        if cycles_position_ref.minimum_purchase > q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: cycles_position_ref.minimum_purchase });
        }        
        
        if usable_user_icp_balance < cycles_to_icptokens(q.cycles, cycles_position_ref.xdr_permyriad_per_icp_rate) + ICP_LEDGER_TRANSFER_DEFAULT_FEE {
            return Err(PurchaseCyclesPositionError::UserIcpBalanceTooLow{ user_icp_balance: usable_user_icp_balance });
        }
        
        if cycles_position_ref.cycles - q.cycles < cycles_position_ref.minimum_purchase 
        && cycles_position_ref.cycles - q.cycles != 0
        && cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
                
        let cycles_position_purchase_id: PurchaseId = {
            let id: PurchaseId = cm_data.id_counter.clone();
            cm_data.id_counter += 1;
            id
        }; 
        cm_data.cycles_positions_purchases.push(
            CyclesPositionPurchase {
                cycles_position_id: cycles_position_ref.id,
                cycles_position_positor: cycles_position_ref.positor,
                cycles_position_xdr_permyriad_per_icp_rate: cycles_position_ref.xdr_permyriad_per_icp_rate,
                id: cycles_position_purchase_id,
                purchaser,
                cycles: q.cycles,
                timestamp_nanos: time(),
                lock: false, // lock for the payouts
                cycles_payout_data: CyclesPayoutData::new(),
                icp_payout_status: false
            }
        );

        std::mem::drop(cycles_position_ref);
        cm_data.cycles_positions[cycles_position_cycles_positions_i].cycles -= q.cycles;
        if cm_data.cycles_positions[cycles_position_cycles_positions_i].cycles < cm_data.cycles_positions[cycles_position_cycles_positions_i].minimum_purchase {
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_cycles_positions_i);
            if cycles_position_for_the_void.cycles != 0 {
                let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_for_the_void.id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_for_the_void_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_for_the_void.id,
                        positor:        cycles_position_for_the_void.positor,
                        cycles:         cycles_position_for_the_void.cycles,
                        lock:           false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time()
                    }
                );
            }
        }   
        
        Ok(cycles_position_purchase_id)
    }) {
        Ok(cycles_position_purchase_id) => cycles_position_purchase_id,
        Err(purchase_cycles_position_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&purchaser); });
            reply::<(PurchaseCyclesPositionResult,)>((Err(purchase_cycles_position_error),));
            do_payouts().await;
            return;
        }
    }
    
    msg_cycles_accept128(PURCHASE_POSITION_FEE);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&purchaser); });
    reply::<(PurchaseCyclesPositionResult,)>((Ok(PurchaseCyclesPositionSuccess{
        purchase_id: cycles_position_purchase_id
    }),));
    do_payouts().await;
    return;

}





// -------------------


pub struct PurchaseIcpPositionQuest {
    icp_position_id: PositionId,
    icp: IcpTokens
}

pub enum PurchaseIcpPositionError {
    MsgCyclesTooLow{ purchase_position_fee: Cycles },
    CyclesMarketIsBusy,
    IcpPositionNotFound,
    IcpPositionIcpIsLessThanThePurchaseQuest{ icp_position_icp: IcpTokens },
    IcpPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ icp_position_minimum_purchase: IcpTokens },

}

pub struct PurchaseIcpPositionSuccess {
    purchase_id: PurchaseId
}

pub type PurchaseIcpPositionResult = Result<PurchaseIcpPositionSuccess, PurchaseIcpPositionError>;

#[update(manual_reply = true)]
pub async fn purchase_icp_position() {

    let purchaser: Principal = caller();

    let icp_position_purchase_id: PurchaseId = match with_mut(&CM_DATA, |cm_data| {
        if cm_data.icp_positions_purchases.len >= MAX_ICP_POSITIONS_PURCHASES {
            return Err(PurchaseIcpPositionError::CyclesMarketIsBusy);
        }
        let icp_position_icp_positions_i: usize = match cm_data.icp_positions.binary_search_by_key(
            &q.icp_position_id,
            |icp_position| { icp_position.id }
        ) {
            Ok(i) => i,
            Err(_) => return Err(PurchaseIcpPositionError::IcpPositionNotFound);
        };
        let icp_position_ref: &IcpPosition = &cm_data.icp_positions[icp_position_icp_positions_i];
        if icp_position_ref.icp < q.icp {
            return Err(PurchaseIcpPositionError::IcpPositionIcpIsLessThanThePurchaseQuest{ icp_position_icp: icp_position_ref.icp });
        }
        if icp_position_ref.minimum_purchase > q.icp {
            return Err(PurchaseIcpPositionError::IcpPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ icp_position_minimum_purchase: icp_position_ref.minimum_purchase });
        }        

        let msg_cycles_quirement: Cycles = PURCHASE_POSITION_FEE + icptokens_to_cycles(q.icp, icp_position_ref.xdr_permyriad_per_icp_rate); 
        if msg_cycles_available128() < msg_cycles_quirement {
            return Err(PurchaseIcpPositionError::MsgCyclesTooLow{ purchase_position_fee: PURCHASE_POSITION_FEE });
        }
        msg_cycles_accept128(msg_cycles_quirement);
        
        let icp_position_purchase_id: PurchaseId = {
            let id: PurchaseId = cm_data.id_counter;
            cm_data.id_counter += 1;
            id
        };        
        
        cm_data.icp_positions_purchases.push(
            IcpPositionPurchase{
                icp_position_id: icp_position_ref.id,
                icp_position_positor: icp_position_ref.positor,
                icp_position_xdr_permyriad_per_icp_rate: icp_position_ref.xdr_permyriad_per_icp_rate,
                id: icp_position_purchase_id,
                purchaser,
                icp: q.icp,
                timestamp_nanos: time(),
                lock: false,
                cycles_payout_data: CyclesPayoutData::new(),
                icp_payout_status: false
            }
        );

        std::mem::drop(icp_position_ref);
        cm_data.icp_positions[icp_position_icp_positions_i].icp -= q.icp;
        if cm_data.icp_positions[icp_position_icp_positions_i].icp < cm_data.icp_positions[icp_position_icp_positions_i].minimum_purchase {
            cm_data.icp_positions.remove(icp_position_icp_positions_i);
        }
        
        Ok(icp_position_purchase_id)
    }) {
        Ok(icp_position_purchase_id) => icp_position_purchase_id,
        Err(purchase_icp_position_error) => {
            reply::<(PurchaseIcpPositionResult,)>((Err(purchase_icp_position_error),));
            do_payouts().await;
            return;
        }
    }
    
    
    reply::<(PurchaseIcpPositionResult,)>((Ok(PurchaseIcpPositionSuccess{
        purchase_id: icp_position_purchase_id
    }),));
    do_payouts().await;
    return;
    
}




// --------------------------


pub struct VoidPositionQuest {
    position_id: PositionId
}

pub enum VoidPositionError {
    WrongCaller,
    CyclesMarketIsBusy,
    PositionNotFound,
}

pub type VoidPositionResult = Result<(), VoidPositionError>;


#[update(manual_reply = true)]
pub async fn void_position(q: VoidPositionQuest) {
    match with_mut(&CM_DATA, |cm_data| {
        if let Ok(cycles_position_i) = cm_data.cycles_positions.binary_search_by_key(&q.position_id, |cycles_position| { cycles_position.id }) {
            if cm_data.cycles_positions[cycles_position_i].positor != caller() {
                return Err(VoidPositionError::WrongCaller);
            }
            if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                return Err(VoidPositionError::CyclesMarketIsBusy);
            }
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_i);
            let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = cm_data.void_cycles_positions.binary_search_by_key(&cycles_position_for_the_void.id, |vcp| { vcp.id }).unwrap_err();
            cm_data.void_cycles_positions.insert(
                cycles_position_for_the_void_void_cycles_positions_insertion_i,
                VoidCyclesPosition{
                    position_id:    cycles_position_for_the_void.id,
                    positor:        cycles_position_for_the_void.positor,
                    cycles:         cycles_position_for_the_void.cycles,
                    lock:           false,
                    cycles_payout_data: CyclesPayoutData::new(),
                    timestamp_nanos: time()                
                }
            );
            Ok(())
        } else if let Ok(icp_position_i) = cm_data.icp_positions.binary_search_by_key(&q.position_id, |icp_position| { icp_position.id }) {
            if cm_data.icp_positions[icp_position_i].positor != caller() {
                return Err(VoidPositionError::WrongCaller);
            }
            cm_data.icp_positions.remove(icp_position_i);
            Ok(())
        } else {
            return Err(VoidPositionError::PositionNotFound);
        }
    }) {
        Ok(()) => {
            reply::<(VoidCyclesPositionResult,)>((Ok(()),));
        },
        Err(void_cycles_position_error) => {
            reply::<VoidCyclesPositionResult,>((Err(void_cycles_position_error),));
        }
    }
    
    do_payouts().await;
    return;    
}


// ----------------

#[query]
pub fn see_icp_lock() -> IcpTokens {
    check_user_icp_balance_in_the_lock(&caller())
}


// ----------------


pub struct TransferIcpBalanceQuest {
    icp: IcpTokens,
    icp_fee: Option<IcpTokens>,
    to: IcpId
}

pub enum TransferIcpBalanceError {
    MsgCyclesTooLow{ transfer_icp_balance_fee: Cycles },
    CyclesMarketIsBusy,
    CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall,
    CheckUserCyclesMarketIcpLedgerBalanceCallError((u32, String)),
    UserIcpBalanceTooLow{ user_icp_balance: IcpTokens },
    IcpTransferCallError((u32, String)),
    IcpTransferError(IcpTransferError)
}

pub type TransferIcpBalanceResult = Result<IcpBlockHeight, TransferIcpBalanceError>

#[update(manual_reply = true)]
pub async fn transfer_icp_balance(q: TransferIcpBalanceQuest) {
    
    let user_id: Principal = caller();
    
    if msg_cycles_available128() < TRANSFER_ICP_BALANCE_FEE {
        reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::MsgCyclesTooLow{ transfer_icp_balance_fee: TRANSFER_ICP_BALANCE_FEE }),));
        do_payouts().await;
        return;
    }

    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_icp_balance_locks.len() >= MAX_MID_CALL_USER_ICP_BALANCE_LOCKS {
            return Err(TransferIcpBalanceError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_icp_balance_locks.contains(&user_id) {
            return Err(TransferIcpBalanceError::CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall);
        }
        cm_data.mid_call_user_icp_balance_locks.insert(user_id);
        Ok(())
    }) {
        Ok(()) => {},
        Err(transfer_icp_balance_error) => {
            reply::<(TransferIcpBalanceResult,)>((Err(transfer_icp_balance_error),));
            do_payouts().await;
            return;
        }
    }
    
    // check icp balance and make sure to unlock the user on returns after here 
    let user_icp_ledger_balance: IcpTokens = match check_user_cycles_market_icp_ledger_balance(&user_id).await {
        Ok(icp_ledger_balance) => icp_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&user_id); });
            reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::CheckUserCyclesMarketIcpLedgerBalanceCallError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    }
    
    let user_icp_balance_in_the_lock: IcpTokens = check_user_icp_balance_in_the_lock(&user_id);
    
    let usable_user_icp_balance: IcpTokens = IcpTokens::from_e8s(user_icp_ledger_balance.e8s().saturating_sub(user_icp_balance_in_the_lock.e8s()));
    
    if usable_user_icp_balance < q.icp + q.icp_fee.unwrap_or(ICP_LEDGER_TRANSFER_DEFAULT_FEE) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&user_id); });
        reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::UserIcpBalanceTooLow{ user_icp_balance: usable_user_icp_balance }),));
        do_payouts().await;
        return;          
    }

    match icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: TRANSFER_ICP_BALANCE_MEMO,
            amount: q.icp,
            fee: q.icp_fee.unwrap_or(ICP_LEDGER_TRANSFER_DEFAULT_FEE),
            from_subaccount: Some(principal_icp_subaccount(&user_id)),
            to,
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time()-1_000_000_000 })
        }   
    ).await {
        Ok(icp_transfer_result) => match icp_transfer_result {
            Ok(icp_transfer_block_height) => {
                msg_cycles_accept128(TRANSFER_ICP_BALANCE_FEE);
                reply::<(TransferIcpBalanceResult,)>((Ok(icp_transfer_block_height),));
            },
            Err(icp_transfer_error) => {
                match icp_transfer_error {
                    IcpTransferError::BadFee{ .. } => {
                        msg_cycles_accept128(TRANSFER_ICP_BALANCE_FEE);
                    },
                    _ => {}
                }
                reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::IcpTransferError(icp_transfer_error)),));
            }
        },
        Err(icp_transfer_call_error) => {
            reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::IcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1))),));
        }
    }

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&user_id); });
    do_payouts().await;
    return;
}



// -------------------------------

pub struct SeeCyclesPositionsQuest {
    chunk_i: u64
}

#[query(manual_reply = true)]
pub fn see_cycles_positions(q: SeeCyclesPositionsQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[CyclesPosition]>,)>((
            cm_data.cycles_positions.chunks(SEE_CYCLES_POSITIONS_CHUNK_SIZE).nth(q.chunk_i as usize)
        ,));
    });
}



pub struct SeeIcpPositionsQuest {
    chunk_i: u64
}

#[query(manual_reply = true)]
pub fn see_icp_positions(q: SeeIcpPositionsQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[IcpPosition]>,)>((
            cm_data.icp_positions.chunks(SEE_ICP_POSITIONS_CHUNK_SIZE).nth(q.chunk_i as usize)
        ,));
    });
}



// -------------------------------------------------------------

#[update(manual_reply = true)]
pub async fn cycles_transferrer_transfer_cycles_callback(q: cycles_transferrer::TransferCyclesCallbackQuest) -> () {
    if with(&CM_DATA, |cm_data| { cm_data.cycles_transferrers.contains(&caller()) }) == false {
        trap("caller must be one of the CTS-cycles-transferrer-canisters for this method.");
    }
    
    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(vcp_void_cycles_positions_i: usize) = cm_data.void_cycles_positions.binary_search_by_key(&q.user_cycles_transfer_id, |vcp| { vcp.position_id }) {
            if cycles_transfer_refund == 0 {
                cm_data.void_cycles_positions.remove(vcp_void_cycles_positions_i);
            } else {
                cm_data.void_cycles_positions[vcp_void_cycles_positions_i]
                    .cycles_payout_data
                    .cycles_transferrer_transfer_cycles_callback_complete = Some((cycles_transfer_refund, q.opt_cycles_transfer_call_error));
            }
        } else if let Ok() = cm_data.positions_purchases.binary_search_by_key /*FIGURE*/ {
        
        }
        
        
    });
    
    reply::<()>(());
    do_void_cycles_positions().await;
    return;
    
} 











// -------------------------------------------------------------



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
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot*/ {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    create_state_snapshot();
    
    with(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT, |state_snapshot| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot.chunks(chunk_size).nth(chunk_i as usize),));
    });
}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_load_state_snapshot_data() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    load_state_snapshot_data();
}



// -------------------------------------------------------------



