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
        TokenTransferError,
        BlockId,
    },
    tools::{
        localkey::refcell::{with, with_mut},
        time_nanos_u64,
        principal_icp_subaccount,
        principal_as_thirty_bytes,
        thirty_bytes_as_principal
    },
    types::{
        Cycles
    },
    cmc::{
        ledger_topup_cycles_cmc_icp_transfer,
        ledger_topup_cycles_cmc_notify,
        LedgerTopupCyclesCmcIcpTransferError,
        LedgerTopupCyclesCmcNotifyError,
        CmcNotifyError
    },
    ic_ledger_types::{IcpBlockHeight, IcpTokens},
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
            reply,
        }
    },
    trap
};
use candid::{
    Principal,
    CandidType,
    Deserialize,
};
use ic_stable_structures::{StableBTreeMap, memory_manager::VirtualMemory, DefaultMemoryImpl, Storable, storable::Bound};
use canister_tools::{
    self,
    MemoryId,
    get_virtual_memory,
};


#[cfg(test)]
mod tests;

// --------- TYPES -----------

#[derive(CandidType, Deserialize)]
pub struct CBData {
    users_mint_cycles: HashMap<Principal, MintCyclesMidCallData>,
    total_supply: Cycles,
}

impl CBData {
    fn new() -> Self {
        Self {
            users_mint_cycles: HashMap::new(),    
            total_supply: 0,
        }
    }
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct StorableCountId(pub CountId);
impl Storable for StorableCountId {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut v = Vec::<u8>::new();
        v.extend(&principal_as_thirty_bytes(&self.0.0));
        v.extend(&self.0.1.unwrap_or([0u8; 32]));
        Cow::Owned(v)
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let owner = thirty_bytes_as_principal(&bytes[..30].try_into().unwrap());
        let subaccount: Subaccount = bytes[30..].try_into().unwrap();
        Self((owner, if subaccount == [0u8; 32] { None } else { Some(subaccount) }))
    }
    const BOUND: Bound = {
        Bound::Bounded{
            max_size: 62,
            is_fixed_size: true
        }
    };
}
impl From<CountId> for StorableCountId {
    fn from(count_id: CountId) -> Self {
        Self(count_id)
    }
}

type Subaccount = [u8; 32];
type CountId = (Principal, Option<Subaccount>);
type CyclesBalances = StableBTreeMap<StorableCountId, Cycles, VirtualMemory<DefaultMemoryImpl>>;

// --------- CONSTS --------

pub const CB_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);
pub const CYCLES_BALANCES_MEMORY_ID: MemoryId = MemoryId::new(1);
pub const MAX_USERS_MINT_CYCLES: usize = 170;
pub const BANK_TRANSFER_FEE: Cycles = 1_000_000_000;

// --------- GLOBAL-STATE ----------

thread_local!{
    pub static CB_DATA: RefCell<CBData> = RefCell::new(CBData::new());
    pub static CYCLES_BALANCES: RefCell<CyclesBalances> = RefCell::new(CyclesBalances::init(get_virtual_memory(CYCLES_BALANCES_MEMORY_ID)));
    /*
    pub static USER_CYCLES_TRANSFERS: RefCell<StableBTreeMap<ic_principal::Principal, Vec<CyclesTransferLog>, VirtualMemory<DefaultMemoryImpl>>> 
        = RefCell::new(StableBTreeMap::init(get_virtual_memory(USER_CYCLES_TRANSFERS_LOGS)));
    */
}

// ---------- LIFECYCLE ---------

#[init]
fn init() {
    canister_tools::init(&CB_DATA, CB_DATA_MEMORY_ID);
} 

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() { 
    canister_tools::post_upgrade(&CB_DATA, CB_DATA_MEMORY_ID, None::<fn(CBData) -> CBData>);    
} 

// ------- FUNCTIONS -------

#[derive(CandidType, Deserialize, Debug)]
pub enum UserIsInTheMiddleOfADifferentCall {
    MintCyclesCall{ must_call_complete: bool },
}

fn check_if_user_is_in_the_middle_of_a_different_call(cb_data: &CBData, user_id: &Principal) -> Result<(), UserIsInTheMiddleOfADifferentCall> {
    if let Some(mint_cycles_mid_call_data) = cb_data.users_mint_cycles.get(user_id) {
        return Err(UserIsInTheMiddleOfADifferentCall::MintCyclesCall{ must_call_complete: !mint_cycles_mid_call_data.lock });
    }
    Ok(())           
}

fn count_id_as_storable(count_id: CountId) -> StorableCountId {
    count_id.into()
}

fn cycles_balance(cycles_balances: &CyclesBalances, count_id: CountId) -> Cycles {
    cycles_balances.get(&count_id_as_storable(count_id)).unwrap_or(0)    
}

fn add_cycles_balance(cycles_balances: &mut CyclesBalances, cb_data: &mut CBData, count_id: CountId, add_cycles: Cycles) {
    let count_id = count_id_as_storable(count_id);
    cycles_balances.insert(
        count_id,
        cycles_balances.get(&count_id).unwrap_or(0).saturating_add(add_cycles)
    );
    cb_data.total_supply = cb_data.total_supply.saturating_add(add_cycles);
}

fn subtract_cycles_balance(cycles_balances: &mut CyclesBalances, cb_data: &mut CBData, count_id: CountId, sub_cycles: Cycles) {
    let count_id = count_id_as_storable(count_id);
    cycles_balances.insert(
        count_id,
        cycles_balances.get(&count_id).unwrap_or(0).saturating_sub(sub_cycles)
    );
    cb_data.total_supply = cb_data.total_supply.saturating_sub(sub_cycles);    
}

fn icrc_id_as_count_id(icrc_id: IcrcId) -> CountId {
    (icrc_id.owner, icrc_id.subaccount)
}



// ------- METHODS ---------

#[query(manual_reply = true)]
pub fn icrc1_name() {
    reply::<(&str,)>(("CYCLES",));
}

#[query(manual_reply = true)]
pub fn icrc1_symbol() {
    reply::<(&str,)>(("CYCLES",));
}

#[query]
pub fn icrc1_decimals() -> u8 {
    12
}

#[query]
pub fn icrc1_minting_account() -> IcrcId {
    IcrcId{owner: ic_cdk::api::id(), subaccount: None}
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
pub fn icrc1_balance_of(icrc_id: IcrcId) -> Cycles {
    with(&CYCLES_BALANCES, |cycles_balances| {
        cycles_balance(cycles_balances, icrc_id_as_count_id(icrc_id))
    })
}

#[derive(CandidType, Deserialize)]
pub struct Icrc1TransferQuest {
    to: IcrcId,
    fee: Option<Cycles>,
    memo: Option<Vec<u8>>,
    from_subaccount: Option<Subaccount>,
    created_at_time: Option<u64>,
    amount: Cycles,
}

#[update]
pub fn icrc1_transfer(q: Icrc1TransferQuest) -> Result<BlockId, TokenTransferError> {
    let caller_count_id: CountId = (caller(), q.from_subaccount);
    
    // check if q.to is the minting account. if it is, make sure the q.fee is set to 0.
    // check if memo is too long
    
    if let Some(quest_fee) = q.fee {
        if quest_fee != BANK_TRANSFER_FEE {
            return Err(TokenTransferError::BadFee{ expected_fee: BANK_TRANSFER_FEE.into() });
        }    
    }
            
    with_mut(&CYCLES_BALANCES, |cycles_balances| {
        let caller_balance: Cycles = cycles_balance(cycles_balances, caller_count_id); 
        if caller_balance < q.amount.saturating_add(BANK_TRANSFER_FEE) {
            return Err(TokenTransferError::InsufficientFunds{ balance: caller_balance.into() })
        }
        
        with_mut(&CB_DATA, |cb_data| {
            subtract_cycles_balance(cycles_balances, cb_data, caller_count_id, q.amount.saturating_add(BANK_TRANSFER_FEE));
            add_cycles_balance(cycles_balances, cb_data, icrc_id_as_count_id(q.to), q.amount);
        });
        
        Ok(())
    })?;
    
    
    Ok(0)
}




// icrc1_metadata
// icrc1_supported_standards





// mint_cycles

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct MintCyclesQuest {
    pub burn_icp: u128, 
    pub burn_icp_transfer_fee: u128,
    pub for_subaccount: Option<Subaccount>   
}

#[derive(CandidType, Deserialize, Debug)]
pub enum MintCyclesError {
    UserIsInTheMiddleOfADifferentCall(UserIsInTheMiddleOfADifferentCall),
    CBIsBusy,
    LedgerTopupCyclesCmcIcpTransferError(LedgerTopupCyclesCmcIcpTransferError),
    LedgerTopupCyclesCmcNotifyRefund{ block_index: u64, reason: String},
    MidCallError(MintCyclesMidCallError)
}

#[derive(CandidType, Deserialize, Debug)]
pub enum MintCyclesMidCallError {
    LedgerTopupCyclesCmcNotifyError(LedgerTopupCyclesCmcNotifyError),
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct MintCyclesSuccess {
    pub mint_cycles: Cycles
}

pub type MintCyclesResult = Result<MintCyclesSuccess, MintCyclesError>;

#[derive(CandidType, Deserialize, Clone)]
pub struct MintCyclesMidCallData {
    start_time_nanos: u64,
    lock: bool,
    quest: MintCyclesQuest,
    cmc_icp_transfer_block_height: Option<IcpBlockHeight>,
    cmc_cycles: Option<Cycles>,
}


#[update]
pub async fn mint_cycles(q: MintCyclesQuest) -> MintCyclesResult {
    
    if q.burn_icp > u64::MAX as u128 || q.burn_icp_transfer_fee > u64::MAX as u128 { trap("burn_icp or burn_icp_transfer_fee amount too large. Max u64::MAX."); }
    
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
            add_cycles_balance(cycles_balances, cb_data, (user_id, mid_call_data.quest.for_subaccount), mid_call_data.cmc_cycles.unwrap());        
            cb_data.users_mint_cycles.remove(&user_id);
        });
    });

    Ok(MintCyclesSuccess{
        mint_cycles: mid_call_data.cmc_cycles.unwrap(),
    })
}

#[derive(CandidType, Deserialize)]
pub enum CompleteMintCyclesError{
    UserIsNotInTheMiddleOfAMintCyclesCall,
    MintCyclesError(MintCyclesError)
}

#[update]
pub async fn complete_mint_cycles(for_user: Option<Principal>) -> Result<MintCyclesSuccess, CompleteMintCyclesError> {
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



