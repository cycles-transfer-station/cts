use cts_lib::types::{
    bank::icrc3::{},
};
use ic_stable_structures::{StableVec, memory_manager::{VirtualMemory, MemoryId}};
use candid::{Principal};

mod storable_logs_pointers;
use storable_logs_pointers::StorableLogsPointers;

type Logs = StableVec<Log, VirtualMemory<DefaultMemoryImpl>>; // copied from the bank
type StableUserLogsPointersMap = StableBTreeMap<StorableIcrcId, StorableLogsPointers, VirtualMemory<DefaultMemoryImpl>>



const LOGS_INDEX_COPY_MEMORY_ID: MemoryId = MemoryId::new(0);
const USER_LOGS_POINTERS_MEMORY_ID: MemoryId = MemoryId::new(1);


thread_local!{
    static LOGS_INDEX_COPY: RefCell<Logs> = RefCell::new(Logs::init(get_virtual_memory(LOGS_INDEX_COPY_MEMORY_ID)).unwrap());
    static USER_LOGS_POINTERS: RefCell<StableUserLogsPointersMap> = RefCell::new(StableUserLogsPointersMap::init(get_virtual_memory(USER_LOGS_POINTERS_MEMORY_ID)));
}


#[derive(CandidType, Deserialize)]
pub struct GetAccountTransactionsQuest{
    max_results: u128,
    start: Option<u128>, 
    account: IcrcId,
}



record {id:nat; transaction:record {burn:opt record {from:record {owner:principal; subaccount:opt vec nat8}; memo:opt vec nat8; created_at_time:opt nat64; amount:nat; spender:opt record {owner:principal; subaccount:opt vec nat8}}; kind:text; mint:opt record {to:record {owner:principal; subaccount:opt vec nat8}; memo:opt vec nat8; created_at_time:opt nat64; amount:nat}; approve:opt record {fee:opt nat; from:record {owner:principal; subaccount:opt vec nat8}; memo:opt vec nat8; created_at_time:opt nat64; amount:nat; expected_allowance:opt nat; expires_at:opt nat64; spender:record {owner:principal; subaccount:opt vec nat8}}; timestamp:nat64; transfer:opt record {to:record {owner:principal; subaccount:opt vec nat8}; fee:opt nat; from:record {owner:principal; subaccount:opt vec nat8}; memo:opt vec nat8; created_at_time:opt nat64; amount:nat; spender:opt record {owner:principal; subaccount:opt vec nat8}}}};

pub struct GetAccountTransactionsOk {
    balance: nat, 
    transactions: Vec<>,  
    oldest_tx_id: opt nat,    
}

pub struct GetAccountTransactionsError {
    message: String,
}

#[query]
pub fn get_account_transactions(q: GetAccountTransactionsQuest) -> Result<GetAccountTransactionsOk, GetAccountTransactionsError> {
    
}

