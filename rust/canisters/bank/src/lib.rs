// FOR THE CYCLES-BANK.
// --------------------

use std::{
    collections::{
        HashMap,
    },
    cell::RefCell,
    borrow::Cow,
};
use cts_lib::{
    icrc::{
        IcrcId,
        Icrc1TransferQuest,
        Icrc1TransferError,
        BlockId,
        IcrcSubaccount,
        ICRC_DEFAULT_SUBACCOUNT,
        IcrcMetadataValue,
    },
    tools::{
        localkey::refcell::{with, with_mut},
        time_nanos_u64,
        principal_icp_subaccount,
        principal_as_thirty_bytes,
        thirty_bytes_as_principal,
        call_error_as_u32_and_string,
        sns_validation_string,
    },
    management_canister::CanisterIdRecord,
    types::{
        Cycles,
        bank::{*, log_types::{Log, LogTX, Operation, MintKind}},
    },
    cmc::{
        ledger_topup_cycles_cmc_icp_transfer,
        ledger_topup_cycles_cmc_notify,
        LedgerTopupCyclesCmcNotifyError,
        CmcNotifyError
    },
    ic_ledger_types::{IcpBlockHeight, IcpTokens},
    consts::{MiB, KiB, TRILLION, NANOS_IN_A_SECOND, SECONDS_IN_A_DAY, SECONDS_IN_A_MINUTE},
};
use ic_cdk::{
    init,
    pre_upgrade,
    post_upgrade,
    update,
    query,
    api::{
        caller,
        call::{
            msg_cycles_available128,
            msg_cycles_accept128,
            call_with_payment128,
        },
        is_controller,
        canister_balance128,
    },
    trap
};
use candid::{
    Principal,
    CandidType,
    Deserialize,
};
use ic_stable_structures::{StableBTreeMap, StableVec, memory_manager::VirtualMemory, DefaultMemoryImpl, Storable, storable::Bound};
use canister_tools::{
    self,
    MemoryId,
    get_virtual_memory,
};

mod transaction_deduplication;
use transaction_deduplication::{check_for_dup, DedupMap, icrc1_transfer_quest_structural_hash};


// --------- TYPES -----------

#[derive(CandidType, Deserialize)]
pub struct CBData {
    users_mint_cycles: HashMap<Principal, MintCyclesMidCallData>,
    total_supply: Cycles,
    icrc1_transfer_dedup_map: DedupMap,
}

impl CBData {
    fn new() -> Self {
        Self {
            users_mint_cycles: HashMap::new(),    
            total_supply: 0,
            icrc1_transfer_dedup_map: DedupMap::new(),
        }
    }
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct StorableIcrcId(pub IcrcId);
impl Storable for StorableIcrcId {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut v = Vec::<u8>::new();
        v.extend(&principal_as_thirty_bytes(&self.0.owner));
        v.extend(self.0.effective_subaccount());
        Cow::Owned(v)
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let owner = thirty_bytes_as_principal(&bytes[..30].try_into().unwrap());
        let subaccount: IcrcSubaccount = bytes[30..].try_into().unwrap();
        Self(IcrcId{ owner, subaccount: if subaccount == *ICRC_DEFAULT_SUBACCOUNT { None } else { Some(subaccount) }})
    }
    const BOUND: Bound = {
        Bound::Bounded{
            max_size: 62,
            is_fixed_size: true
        }
    };
}
impl From<IcrcId> for StorableIcrcId {
    fn from(icrc_id: IcrcId) -> Self {
        Self(icrc_id)
    }
}

type CyclesBalances = StableBTreeMap<StorableIcrcId, Cycles, VirtualMemory<DefaultMemoryImpl>>;
type Logs = StableVec<Log, VirtualMemory<DefaultMemoryImpl>>;
type UserLogsPointers = HashMap<IcrcId, Vec<u32>>;

// --------- CONSTS --------

pub const CB_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);
pub const CYCLES_BALANCES_MEMORY_ID: MemoryId = MemoryId::new(1);
pub const LOGS_MEMORY_ID: MemoryId = MemoryId::new(2);
pub const USER_LOGS_POINTERS_MEMORY_ID: MemoryId = MemoryId::new(3);

pub const MINIMUM_BURN_ICP: u128 = 10_000_000/*0.1-icp*/; // When changing this value, change the frontcode burn-icp form field validator with the new value.
pub const MAX_USERS_MINT_CYCLES: usize = 170;
pub const MINIMUM_CANISTER_CYCLES_BALANCE_FOR_A_START_MINT_CYCLES_CALL: Cycles = 2 * TRILLION;
pub const MINIMUM_CANISTER_CYCLES_BALANCE_FOR_A_CMC_NOTIFY_MINT_CYCLES_CALL: Cycles = 1 * TRILLION;

pub const ICRC1_NAME: &'static str = "CTS-CYCLES-BANK";
pub const ICRC1_SYMBOL: &'static str = "CTS-CYCLES";
pub const ICRC1_DECIMALS: u8 = 12;

pub const TX_WINDOW_NANOS: u64 = (NANOS_IN_A_SECOND * SECONDS_IN_A_DAY) as u64;
pub const PERMITTED_DRIFT_NANOS: u64 = (NANOS_IN_A_SECOND * SECONDS_IN_A_MINUTE * 2) as u64;
pub const MAX_LEN_OF_A_DEDUP_MAP: usize = 86_400; // this is the max-len of one dedup map. there are three dedup maps.


// --------- GLOBAL-STATE ----------

thread_local!{
    pub static CB_DATA: RefCell<CBData> = RefCell::new(CBData::new());
    pub static USER_LOGS_POINTERS: RefCell<UserLogsPointers> = RefCell::new(UserLogsPointers::new());
    // stable-structures
    pub static CYCLES_BALANCES: RefCell<CyclesBalances> = RefCell::new(CyclesBalances::init(get_virtual_memory(CYCLES_BALANCES_MEMORY_ID)));
    pub static LOGS: RefCell<Logs> = RefCell::new(Logs::init(get_virtual_memory(LOGS_MEMORY_ID)).unwrap());
}

// ---------- LIFECYCLE ---------

#[init]
fn init() {
    canister_tools::init(&CB_DATA, CB_DATA_MEMORY_ID);
    canister_tools::init(&USER_LOGS_POINTERS, USER_LOGS_POINTERS_MEMORY_ID);
} 

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() { 
    //canister_tools::post_upgrade(&CB_DATA, CB_DATA_MEMORY_ID, None::<fn(CBData) -> CBData>);    
    #[derive(CandidType, Deserialize)]
    struct OldCBData {
        users_mint_cycles: HashMap<Principal, MintCyclesMidCallData>,
        total_supply: Cycles,
    }
    canister_tools::post_upgrade(&CB_DATA, CB_DATA_MEMORY_ID, Some::<fn(OldCBData) -> CBData>(
        |old| {
            CBData{
                users_mint_cycles: old.users_mint_cycles,
                total_supply: old.total_supply,
                icrc1_transfer_dedup_map: DedupMap::new(),
            }
        }
    ));
        
    canister_tools::post_upgrade(&USER_LOGS_POINTERS, USER_LOGS_POINTERS_MEMORY_ID, None::<fn(UserLogsPointers) -> UserLogsPointers>);
} 

// ------- FUNCTIONS -------

fn check_if_user_is_in_the_middle_of_a_different_call(cb_data: &CBData, user_id: &Principal) -> Result<(), UserIsInTheMiddleOfADifferentCall> {
    if let Some(mint_cycles_mid_call_data) = cb_data.users_mint_cycles.get(user_id) {
        return Err(UserIsInTheMiddleOfADifferentCall::MintCyclesCall{ must_call_complete: !mint_cycles_mid_call_data.lock });
    }
    Ok(())           
}

fn icrc_id_as_storable(icrc_id: IcrcId) -> StorableIcrcId {
    icrc_id.into()
}

fn cycles_balance(cycles_balances: &CyclesBalances, icrc_id: IcrcId) -> Cycles {
    cycles_balances.get(&icrc_id_as_storable(icrc_id)).unwrap_or(0)    
}

fn add_cycles_balance(cycles_balances: &mut CyclesBalances, cb_data: &mut CBData, icrc_id: IcrcId, add_cycles: Cycles) {
    let icrc_id = icrc_id_as_storable(icrc_id);
    cycles_balances.insert(
        icrc_id,
        cycles_balances.get(&icrc_id).unwrap_or(0).saturating_add(add_cycles)
    );
    cb_data.total_supply = cb_data.total_supply.saturating_add(add_cycles);
}

fn subtract_cycles_balance(cycles_balances: &mut CyclesBalances, cb_data: &mut CBData, icrc_id: IcrcId, sub_cycles: Cycles) {
    let icrc_id = icrc_id_as_storable(icrc_id);
    cycles_balances.insert(
        icrc_id,
        cycles_balances.get(&icrc_id).unwrap_or(0).saturating_sub(sub_cycles)
    );
    cb_data.total_supply = cb_data.total_supply.saturating_sub(sub_cycles);    
}



// ------- METHODS ---------

#[query]
pub fn icrc1_name() -> String {
    ICRC1_NAME.to_string()
}

#[query]
pub fn icrc1_symbol() -> String {
    ICRC1_SYMBOL.to_string()
}

#[query]
pub fn icrc1_decimals() -> u8 {
    ICRC1_DECIMALS
}

#[query]
pub fn icrc1_minting_account() -> Option<IcrcId> {
    None
}

#[query]
pub fn icrc1_fee() -> Cycles {
    BANK_TRANSFER_FEE
}

#[query]
pub fn icrc1_total_supply() -> Cycles {
    with(&CB_DATA, |cb_data| {
        cb_data.total_supply
    })
}

#[query]
pub fn icrc1_metadata() -> Vec<(String, IcrcMetadataValue)> {
    vec![
        ("icrc1:name".to_string(), IcrcMetadataValue::Text(ICRC1_NAME.to_string())),
        ("icrc1:symbol".to_string(), IcrcMetadataValue::Text(ICRC1_SYMBOL.to_string())),
        ("icrc1:decimals".to_string(), IcrcMetadataValue::Nat(ICRC1_DECIMALS.into())),
        ("icrc1:fee".to_string(), IcrcMetadataValue::Nat(BANK_TRANSFER_FEE.into())),
        ("icrc1:logo".to_string(), IcrcMetadataValue::Text("".to_string())),
    ]
}

#[derive(CandidType, Deserialize)]
pub struct SupportedStandard {
    name: String,
    url: String
}

#[query]
pub fn icrc1_supported_standards() -> Vec<SupportedStandard> {
    vec![
        SupportedStandard{
            name: "ICRC-1".to_string(),
            url: "https://github.com/dfinity/ICRC-1".to_string(),
        },
    ]
}


#[query]
pub fn icrc1_balance_of(icrc_id: IcrcId) -> Cycles {
    with(&CYCLES_BALANCES, |cycles_balances| {
        cycles_balance(cycles_balances, icrc_id)
    })
}

// make sure the icrc1_transfer method stays synchronous, within one single message execution. bc the transaction dedup check is only valid within it's message execution and we need it to be valid for the whole of the icrc1-transfer
#[update]
pub fn icrc1_transfer(q: Icrc1TransferQuest) -> Result<BlockId, Icrc1TransferError> {
    let caller = caller();
    let caller_icrc_id: IcrcId = IcrcId{ owner: caller, subaccount: q.from_subaccount };
    
    with_mut(&CB_DATA, |cb_data| {
        check_for_dup(&mut cb_data.icrc1_transfer_dedup_map, caller, &q) // only valid within this message-execution. make sure icrc1_transfer method stays sync.
    })?; 
    
    if let Some(ref memo) = q.memo {
        if memo.len() > 32 {
            trap("Max memo length is 32 bytes.");
        }
    }
    
    if let Some(quest_fee) = q.fee {
        if quest_fee != BANK_TRANSFER_FEE {
            return Err(Icrc1TransferError::BadFee{ expected_fee: BANK_TRANSFER_FEE.into() });
        }    
    }
            
    with_mut(&CYCLES_BALANCES, |cycles_balances| {
        let caller_balance: Cycles = cycles_balance(cycles_balances, caller_icrc_id); 
        if caller_balance < q.amount.saturating_add(BANK_TRANSFER_FEE) {
            return Err(Icrc1TransferError::InsufficientFunds{ balance: caller_balance.into() })
        }
        
        with_mut(&CB_DATA, |cb_data| {
            subtract_cycles_balance(cycles_balances, cb_data, caller_icrc_id, q.amount.saturating_add(BANK_TRANSFER_FEE));
            add_cycles_balance(cycles_balances, cb_data, q.to, q.amount);
        });
        
        Ok(())
    })?;
    
    let q_structural_hash: [u8; 32] = icrc1_transfer_quest_structural_hash(&q); 
    
    let block_height: u64 = with_mut(&LOGS, |logs| {
        logs.push(
            &Log{
                ts: time_nanos_u64(),
                fee: if q.fee.is_none() { Some(BANK_TRANSFER_FEE) } else { None },
                tx: LogTX{
                    op: Operation::Xfer{ from: caller_icrc_id, to: q.to },
                    fee: q.fee,
                    amt: q.amount,
                    memo: q.memo,
                    ts: q.created_at_time,
                }
            }
        ).unwrap(); // if growfailed then trap and roll back the transfer.
        logs.len() - 1
    });
    
    if let Some(created_at_time) = q.created_at_time {
        with_mut(&CB_DATA, |cb_data| {
            cb_data.icrc1_transfer_dedup_map.insert(
                (caller, q_structural_hash),
                (block_height as u128, created_at_time),
            );
        });
    }
    
    with_mut(&USER_LOGS_POINTERS, |user_logs_pointers| {
        user_logs_pointers.entry(caller_icrc_id)
        .or_default()
        .push(block_height as u32);
        if q.to != caller_icrc_id {
            user_logs_pointers.entry(q.to)
            .or_default()
            .push(block_height as u32);
        }
    });
    
    Ok(block_height as u128)
}





const LOGS_CHUNK_SIZE: usize = (1*MiB + 512*KiB) / 400;



#[query]
pub fn get_logs_backwards(icrc_id: IcrcId, opt_start_before_block: Option<u128>) -> GetLogsBackwardsSponse {
    let mut v: Vec<(BlockId, Log)> = Vec::new();
    let mut is_last_chunk = true;
    with(&USER_LOGS_POINTERS, |user_logs_pointers| {
        let list: &Vec<u32> = match user_logs_pointers.get(&icrc_id) {
            Some(list) => list,
            None => return,
        };
        let end_i: usize = if let Some(start_before_block) = opt_start_before_block {
            list.binary_search(&(start_before_block as u32)).unwrap_or_else(|i|i)  
        } else {
            list.len()
        };
        let start_i: usize = end_i.saturating_sub(LOGS_CHUNK_SIZE);
        is_last_chunk = start_i == 0; 
        with(&LOGS, |logs| {
            for block_height in list[start_i..end_i].iter() {
                v.push((block_height.clone() as u128, logs.get(block_height.clone() as u64).unwrap()));     
            }
        });
    });
    GetLogsBackwardsSponse {
        logs: v,
        is_last_chunk,
    }    
}


// in case it gets too large before the scaling code is done.
#[update]
fn controller_clear_user_logs_pointers_cache() {
    if is_controller(&caller()) == false {
        trap("must be the controller for this method.");
    }
    with_mut(&USER_LOGS_POINTERS, |user_logs_pointers| {
        user_logs_pointers.clear();
        user_logs_pointers.shrink_to_fit();
        *user_logs_pointers = UserLogsPointers::new();
    });
}


// cycles_in


#[update]
pub fn cycles_in(q: CyclesInQuest) -> Result<BlockId, CyclesInError> {
    
    if let Some(quest_fee) = q.fee {
        if quest_fee != BANK_TRANSFER_FEE {
            return Err(CyclesInError::BadFee{ expected_fee: BANK_TRANSFER_FEE });
        }    
    }
    
    if let Some(ref memo) = q.memo {
        if memo.len() > 32 {
            trap("Max memo length is 32 bytes.");
        }
    }
    
    let msg_cycles_available = msg_cycles_available128();
    if msg_cycles_available < q.cycles.saturating_add(BANK_TRANSFER_FEE) {
        return Err(CyclesInError::MsgCyclesTooLow);
    }
    
    msg_cycles_accept128(q.cycles.saturating_add(BANK_TRANSFER_FEE));
    
    with_mut(&CYCLES_BALANCES, |cycles_balances| {
        with_mut(&CB_DATA, |cb_data| {
            add_cycles_balance(cycles_balances, cb_data, q.to, q.cycles);
        });
    });
    
    let block_height: u64 = with_mut(&LOGS, |logs| {
        logs.push(
            &Log{
                ts: time_nanos_u64(),
                fee: if q.fee.is_none() { Some(BANK_TRANSFER_FEE) } else { None },
                tx: LogTX{
                    op: Operation::Mint{ to: q.to, kind: MintKind::CyclesIn{ from_canister: caller() } },
                    fee: q.fee,
                    amt: q.cycles,
                    memo: q.memo,
                    ts: None,
                }
            }
        ).unwrap(); // if growfailed then trap and roll back.
        logs.len() - 1
    });
    
    with_mut(&USER_LOGS_POINTERS, |user_logs_pointers| {
        user_logs_pointers.entry(q.to)
        .or_default()
        .push(block_height as u32);        
    });

    Ok(block_height as u128)
}  


// cycles-out

#[query]
pub fn sns_validate_cycles_out(q: CyclesOutQuest) -> Result<String, String> {
    Ok(sns_validation_string(q))    
}

#[update]
pub async fn cycles_out(q: CyclesOutQuest) -> Result<BlockId, CyclesOutError> {
        
    if let Some(quest_fee) = q.fee {
        if quest_fee != BANK_TRANSFER_FEE {
            return Err(CyclesOutError::BadFee{ expected_fee: BANK_TRANSFER_FEE });
        }    
    }
    
    if let Some(ref memo) = q.memo {
        if memo.len() > 32 {
            trap("Max memo length is 32 bytes.");
        }
    }
    
    let caller_icrc_id: IcrcId = IcrcId{ owner: caller(), subaccount: q.from_subaccount };
    
    with_mut(&CYCLES_BALANCES, |cycles_balances| {
        let caller_balance: Cycles = cycles_balance(cycles_balances, caller_icrc_id); 
        if caller_balance < q.cycles.saturating_add(BANK_TRANSFER_FEE) {
            return Err(CyclesOutError::InsufficientFunds{ balance: caller_balance.into() })
        }        
        with_mut(&CB_DATA, |cb_data| {
            subtract_cycles_balance(cycles_balances, cb_data, caller_icrc_id, q.cycles.saturating_add(BANK_TRANSFER_FEE));            
        }); 
        Ok(())
    })?;
    
    let r = call_with_payment128::<_, ()>(
        Principal::management_canister(),
        "deposit_cycles",
        (CanisterIdRecord{canister_id: q.for_canister},),
        q.cycles
    ).await;
    
    match r {
        Ok(()) => {
            let block_height: u64 = with_mut(&LOGS, |logs| {
                logs.push(
                    &Log{
                        ts: time_nanos_u64(),
                        fee: if q.fee.is_none() { Some(BANK_TRANSFER_FEE) } else { None },
                        tx: LogTX{
                            op: Operation::Burn{ from: caller_icrc_id, for_canister: q.for_canister },
                            fee: q.fee,
                            amt: q.cycles.saturating_add(BANK_TRANSFER_FEE), // include the fee in the amount here because icrc1 does not have fees for a burn. so we put the amount here that is getting subtracted from the caller's account.
                            memo: q.memo,
                            ts: None,
                        }
                    }
                ).unwrap(); // what to do if grow-fail here? For now there is a lot of space on the fiduciary subnet so it is ok for now but either put in a buffer wait on a timer or create archives.
                logs.len() - 1
            });
            
            with_mut(&USER_LOGS_POINTERS, |user_logs_pointers| {
                user_logs_pointers.entry(caller_icrc_id)
                .or_default()
                .push(block_height as u32);
            });                    
            
            Ok(block_height as u128)
        }
        Err(call_error) => {
            with_mut(&CYCLES_BALANCES, |cycles_balances| {
                with_mut(&CB_DATA, |cb_data| {
                    add_cycles_balance(cycles_balances, cb_data, caller_icrc_id, q.cycles.saturating_add(BANK_TRANSFER_FEE));
                })
            });
            Err(CyclesOutError::DepositCyclesCallError(call_error_as_u32_and_string(call_error)))
        }
    }
}  



// mint_cycles

#[derive(CandidType, Deserialize, Clone)]
struct MintCyclesMidCallData {
    start_time_nanos: u64,
    lock: bool,
    quest: MintCyclesQuest,
    fee: Cycles,
    cmc_icp_transfer_block_height: Option<IcpBlockHeight>,
    cmc_cycles: Option<Cycles>,
}


#[update]
pub async fn mint_cycles(q: MintCyclesQuest) -> MintCyclesResult {
    if canister_balance128() < MINIMUM_CANISTER_CYCLES_BALANCE_FOR_A_START_MINT_CYCLES_CALL {
        trap("This canister is low on cycles.");
    }
    
    if q.burn_icp > u64::MAX as u128 || q.burn_icp_transfer_fee > u64::MAX as u128 { trap("burn_icp or burn_icp_transfer_fee amount too large. Max u64::MAX."); }
    
    if q.burn_icp < MINIMUM_BURN_ICP {
        return Err(MintCyclesError::MinimumBurnIcp{ minimum_burn_icp: MINIMUM_BURN_ICP });
    }
    
    if let Some(quest_fee) = q.fee {
        if quest_fee != BANK_TRANSFER_FEE {
            return Err(MintCyclesError::BadFee{ expected_fee: BANK_TRANSFER_FEE });
        }
    }
    let fee: Cycles = BANK_TRANSFER_FEE; // save in the call here in case the fee changes while this mint is doing, use the agreed upon fee.  
    
    if let Some(ref memo) = q.memo {
        if memo.len() > 32 {
            trap("Max memo length is 32 bytes.");
        }
    }
    
    let user_id: Principal = caller();
    
    let mid_call_data: MintCyclesMidCallData = with_mut(&CB_DATA, |cb_data| {
        check_if_user_is_in_the_middle_of_a_different_call(cb_data, &user_id).map_err(|e| MintCyclesError::UserIsInTheMiddleOfADifferentCall(e))?;
        if cb_data.users_mint_cycles.len() >= MAX_USERS_MINT_CYCLES {
            return Err(MintCyclesError::CBIsBusy);
        }
        let mid_call_data = MintCyclesMidCallData{
            start_time_nanos: time_nanos_u64(),
            lock: true,
            quest: q, 
            fee: fee,
            cmc_icp_transfer_block_height: None,
            cmc_cycles: None,
        };
        cb_data.users_mint_cycles.insert(user_id.clone(), mid_call_data.clone());
        Ok(mid_call_data)
    })?;

    mint_cycles_(user_id, mid_call_data).await
}

async fn mint_cycles_(user_id: Principal, mut mid_call_data: MintCyclesMidCallData) -> MintCyclesResult {   
    
    if mid_call_data.cmc_icp_transfer_block_height.is_none() {
        match ledger_topup_cycles_cmc_icp_transfer(
            IcpTokens::from_e8s(mid_call_data.quest.burn_icp as u64), 
            IcpTokens::from_e8s(mid_call_data.quest.burn_icp_transfer_fee as u64),
            Some(principal_icp_subaccount(&user_id)),
            ic_cdk::api::id()
        ).await {
            Ok(block_height) => { 
                mid_call_data.cmc_icp_transfer_block_height = Some(block_height); 
            },
            Err(ledger_topup_cycles_cmc_icp_transfer_error) => {
                with_mut(&CB_DATA, |cb_data| { cb_data.users_mint_cycles.remove(&user_id); });
                return Err(MintCyclesError::LedgerTopupCyclesCmcIcpTransferError(ledger_topup_cycles_cmc_icp_transfer_error));
            }
        }
    }
    
    if mid_call_data.cmc_cycles.is_none() {
        if canister_balance128() < MINIMUM_CANISTER_CYCLES_BALANCE_FOR_A_CMC_NOTIFY_MINT_CYCLES_CALL {
            mid_call_data.lock = false;
            with_mut(&CB_DATA, |cb_data| { cb_data.users_mint_cycles.insert(user_id, mid_call_data); });
            return Err(MintCyclesError::MidCallError(MintCyclesMidCallError::CouldNotPerformCmcNotifyCallDueToLowBankCanisterCycles));
        }
        match ledger_topup_cycles_cmc_notify(mid_call_data.cmc_icp_transfer_block_height.unwrap(), ic_cdk::api::id()).await {
            Ok(cmc_cycles) => { 
                mid_call_data.cmc_cycles = Some(cmc_cycles); 
            },
            Err(ledger_topup_cycles_cmc_notify_error) => {
                if let LedgerTopupCyclesCmcNotifyError::CmcNotifyError(CmcNotifyError::Refunded{ block_index: Some(b), reason: r }) = ledger_topup_cycles_cmc_notify_error {
                    with_mut(&CB_DATA, |cb_data| { cb_data.users_mint_cycles.remove(&user_id); });
                    return Err(MintCyclesError::LedgerTopupCyclesCmcNotifyRefund{ block_index: b, reason: r});
                } else {
                    mid_call_data.lock = false;
                    with_mut(&CB_DATA, |cb_data| { cb_data.users_mint_cycles.insert(user_id, mid_call_data); });
                    return Err(MintCyclesError::MidCallError(MintCyclesMidCallError::LedgerTopupCyclesCmcNotifyError(ledger_topup_cycles_cmc_notify_error)));
                }
            }
        }
    }
        
    with_mut(&CYCLES_BALANCES, |cycles_balances| {
        with_mut(&CB_DATA, |cb_data| {
            add_cycles_balance(cycles_balances, cb_data, mid_call_data.quest.to, mid_call_data.cmc_cycles.unwrap().saturating_sub(mid_call_data.fee));        
            cb_data.users_mint_cycles.remove(&user_id);
        });
    });
    
    let block_height: u64 = with_mut(&LOGS, |logs| {
        logs.push(
            &Log{
                ts: time_nanos_u64(),
                fee: if mid_call_data.quest.fee.is_none() { Some(mid_call_data.fee) } else { None },
                tx: LogTX{
                    op: Operation::Mint{ to: mid_call_data.quest.to, kind: MintKind::CMC{ caller: user_id, icp_block_height: mid_call_data.cmc_icp_transfer_block_height.unwrap() } },
                    fee: mid_call_data.quest.fee,
                    amt: mid_call_data.cmc_cycles.unwrap().saturating_sub(mid_call_data.fee),
                    memo: mid_call_data.quest.memo,
                    ts: None,
                }
            }
        ).unwrap(); // look at comment for cycles_out logs.push 
        logs.len() - 1
    });
    
    with_mut(&USER_LOGS_POINTERS, |user_logs_pointers| {
        user_logs_pointers.entry(mid_call_data.quest.to)
        .or_default()
        .push(block_height as u32);        
    });

    Ok(MintCyclesSuccess{
        mint_cycles: mid_call_data.cmc_cycles.unwrap().saturating_sub(mid_call_data.fee),
        mint_cycles_block_height: block_height as u128
    })
}

#[update]
pub async fn complete_mint_cycles(for_user: Option<Principal>) -> CompleteMintCyclesResult {
    complete_mint_cycles_(for_user.unwrap_or(caller())).await
}

async fn complete_mint_cycles_(user_id: Principal) -> Result<MintCyclesSuccess, CompleteMintCyclesError> {
    
    let mid_call_data: MintCyclesMidCallData = with_mut(&CB_DATA, |cb_data| {
        match cb_data.users_mint_cycles.get_mut(&user_id) {
            Some(mid_call_data) => {
                if mid_call_data.lock == true {
                    return Err(CompleteMintCyclesError::MintCyclesError(MintCyclesError::UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall::MintCyclesCall{ must_call_complete: false })));
                }
                mid_call_data.lock = true;
                Ok(mid_call_data.clone())
            },
            None => {
                return Err(CompleteMintCyclesError::UserIsNotInTheMiddleOfAMintCyclesCall);
            }
        }
    })?;

    mint_cycles_(user_id, mid_call_data).await
        .map_err(|mint_cycles_error| { 
            CompleteMintCyclesError::MintCyclesError(mint_cycles_error) 
        })
}


#[query]
pub fn canister_cycles_balance_minus_total_supply() -> i128 {
    (canister_balance128() as i128).saturating_sub(with(&CB_DATA, |cb_data| { cb_data.total_supply }) as i128)
}




ic_cdk::export_candid!();
