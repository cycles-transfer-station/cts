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

// when there is name-callbacks the canisters can check the memo's base on the caller

type PositionId = u128;



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


enum Commodity {
    Cycles(Cycles),
    Icp(IcpTokens)
}

struct PositionPurchase {
    position_id: PositionId,
    purchaser: Principal,
    mount: Commodity,
    timestamp_nanos: u64,
    cycles_payout_data: CyclesPayoutData,
    icp_payout: bool
}

#[derive(Clone)]
struct CyclesPayoutData {
    cycles_transferrer_transfer_cycles_call_success: bool,
    cycles_transferrer_transfer_cycles_callback_complete: Option<(CyclesTransferRefund, Option<(u32, String)>)>,
    management_canister_posit_cycles_call_success: bool // this is use for when the payout-cycles-transfer-refund != 0, call the management_canister-deposit_cycles(payout-cycles-transfer-refund)
} 
impl CyclesPayoutData {
    fn new() -> Self {
        Self {
            cycles_transferrer_transfer_cycles_call_success: false,
            cycles_transferrer_transfer_cycles_callback_complete: None,
            management_canister_posit_cycles_call_success: false
        }
    }
}

struct VoidCyclesPosition {
    position_id: PositionId,
    positor: Principal,
    cycles: Cycles,
    cycles_payout_data: CyclesPayoutData
    timestamp_nanos: u64
}




struct CMData {
    cts_id: Principal,
    cycles_transferrers: Vec<Principal>,
    position_id_counter: PositionId,
    cycles_positions: Vec<CyclesPosition>,
    icp_positions: Vec<IcpPosition>,
    positions_purchases: Vec<PositionPurchase>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    
}

impl CMData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            cycles_transferrers: Vec::new(),
            position_id_counter: 0,
            cycles_positions: Vec::new(),
            icp_positions: Vec::new(),
            positions_purchases: Vec::new(),
            void_cycles_positions: Vec::new(),
        }
    }
}



pub const CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;

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

const POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB: u64 = CANISTER_DATA_STORAGE_SIZE_MiB / 5 * 2;
const MAX_POSITIONS_PURCHASES: usize = ( POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<PositionPurchase>() as u64 ) as usize;

const VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: u64 = CANISTER_DATA_STORAGE_SIZE_MiB / 10; // / 5 * 0.5;
const MAX_VOID_CYCLES_POSITIONS: usize = ( VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidCyclesPosition>() as u64 ) as usize;


const DO_VOID_CYCLES_POSITIONS_CHUNK_SIZE: usize = 500;


const VOID_POSITION_CYCLES_TRANSFER_MEMO_START: &[u8; 5] = b"CM-VP";


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
        "purchase_position",
        "void_position",
    ].contains(&&method_name()[..]) {
        trap("this method must be call by a canister.");
    
    }
    
    
    accept_message();    
}


// -------------------------------------------------------------

fn cts_id() -> Principal {
    with(&CM_DATA, |cm_data| { cm_data.cts_id })
}

fn new_position_id() {
    with_mut(&CM_DATA, |cm_data| {
        let position_id: PositionId = cm_data.position_id_counter.clone();
        cm_data.position_id_counter += 1;
        position_id
    })
}

async fn do_cycles_payout(for_the_canister: Principal, cycles: Cycles, cycles_transfer_memo: CyclesTransferMemo, cycles_payout_data: CyclesPayoutData) -> CyclesPayoutData {

    
}

async fn do_void_cycles_positions() {
    
    if with(&CM_DATA, |cm_data| { cm_data.void_cycles_positions.len() == 0 { return; }
    
    let void_cycles_positions_chunk: Vec<VoidCyclesPosition> = Vec::new();
    
    with_mut(&CM_DATA, |cm_data| { 
        void_cycles_positions_chunk = cm_data.void_cycles_positions.drain(..std::cmp::min(cm_data.void_cycles_positions.len(), DO_VOID_CYCLES_POSITIONS_CHUNK_SIZE)).collect::<Vec<VoidCyclesPosition>>();
    });

    void_cycles_positions_chunk = {
        void_cycles_positions_chunk.into_iter()
            .filter(|vcp: &VoidCyclesPosition| { vcp.cycles_payout_data.is_complete() == false })
            .collect::<Vec<VoidCyclesPosition>>()
    };
    
    let rs: Vec<CyclesPayoutData> = futures::future::join_all(
        void_cycles_positions_chunk.iter().map(
            |vcp| {
                do_cycles_payout(
                    vcp.positor,
                    vcp.cycles,
                    CyclesTransferMemo::Blob([VOID_POSITION_CYCLES_TRANSFER_MEMO_START, vcp.position_id.to_be_bytes()].concat().to_vec()),
                    vcp.cycles_payout_data.clone()
                )   
            }
        ).collect::<Vec<_>>()
    ).await;
    
    // mut for the append
    let mut final_void_cycles_positions_chunk: Vec<VoidCyclesPosition> = {
        void_cycles_positions_chunk.into_iter().zip(rs.into_iter())
            .filter(|(vcp, cycles_payout_data): (VoidCyclesPosition, CyclesPayoutData)| {
                cycles_payout_data.is_complete() == false // cause the ones that are complete we don't want to keep
            })
            .map(|(vcp, cycles_payout_data): (VoidCyclesPosition, CyclesPayoutData)| {
                vcp.cycles_payout_data = cycles_payout_data;
                vcp
            })
            .collect::<Vec<VoidCyclesPosition>>()
    };
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.void_cycles_positions.append(&mut final_void_cycles_positions_chunk);
        cm_data.void_cycles_positions.sort_by_key(|vcp: &VoidCyclesPosition| { vcp.position_id });
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

    let msg_cycles_quirement: Cycles = q.cycles.checked_add(CREATE_POSITION_FEE).unwrap_or(Cycles::MAX); 

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
    
    if q.cycles < MINIMUM_CYCLES_POSITION {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumCyclesPosition(MINIMUM_CYCLES_POSITION)),));
        do_void_cycles_positions().await;
        return;
    }
    
    let position_id: PositionId = new_position_id(); 
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.cycles_positions.push(
            CyclesPosition{
                id: position_id,   
                positor,
                cycles: q.cycles,
                minimum_purchase: q.minimum_purchase,
                xdr_permyriad_per_icp_rate: q.xdr_permyriad_per_icp_rate,
                timestamp_nanos: time(),
            }
        );
    });
    
    if msg_cycles_accept128(msg_cycles_quirement) != msg_cycles_quirement { trap("check system guarentees") }

    reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Ok(
        CreateCyclesPositionSuccess{
            position_id
        }
    ),));
    
    do_void_cycles_positions().await;
    return;
}







pub struct CreateIcpPositionQuest {
    icp: IcpTokens,
    minimum_purchase: IcpTokens,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcpRate,
    
}




#[update]
pub async fn create_icp_position(q: CreateIcpPositionQuest) -> Result<CreateIcpPositionSuccess,CreateIcpPositionError>{}



#[update]
pub async fn purchase_position() {}






#[update]
pub async fn void_position() {}






// -------------------------------------------------------------


pub fn cycles_transferrer_transfer_cycles_callback(cycles_transferrer::TransferCyclesCallbackQuest) -> () {
    
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



