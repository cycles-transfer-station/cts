use ic_cdk::{
    init,
    pre_upgrade,
    post_upgrade,
	update, 
    query,
    call,
};
use cts_lib::{
    types::{
        Cycles,
        CallError,
        cm::tc::ShareholderPayoutsCollectTradeFeesSponse,
    },
    tools::{
        localkey::refcell::{with, with_mut},
        call_error_as_u32_and_string,
    }
};
use ic_stable_structures::{StableBTreeMap, memory_manager::{MemoryId, VirtualMemory}, DefaultMemoryImpl};
use std::time::Duration;
use std::collections::{HashSet, HashMap, BTreeSet, BTreeMap};
use std::cell::RefCell;
use cts_lib::types::cm::cm_main::{TradeContractIdAndLedgerId, TradeContractData};
use outsiders::sns_governance::{Service as SNSGovernanceService, ListNeurons, /*ListNeuronsResponse*/, NeuronId};	
use candid::{Principal, CandidType, Deserialize};

#[cfg(test)]
mod tests;

#[derive(CandidType, Deserialize)]
pub struct SPData {
    sns_governance_canister_id: Principal,
    cm_main: Principal,
    // contains both the caller-cts-user-ids and the neuron-owner-ids.
    register_neuron_owner_locks: HashSet<Principal>,
    // reward-events happen at the same time as the sns-governance reward events unless there is not enough in the pool to do a reward-event.
    last_sns_reward_event_of_the_previous_cts_reward_event: u64, // sns-governance.reward-event.end_timestamp_seconds is the id of the reward-event.
    latest_sns_reward_event_done_parsing: u64,
    // locks when parsing sns_reward_events and when doing cts-payout-event and when filling up the current_built_up_cts_rewards.
    lock: bool,
    current_built_up_cts_rewards_cycles: Cycles, // the built-up rewards of the cycles currency in all the tcs combined. value will be reset after a payout-event of the cycles-rewards.
    current_built_up_cts_rewards_tokens: Vec<u64>, // in the same sequence as the cm_main. each value will be reset after a payout-event of that cm_tc.
    sum_built_up_neuron_rewards: u64, // the sum of the built_up_neuron_reward_e8s of every neuron. useful for performance. this is reset after a cts-payout-event.  
}
impl SPData {
    fn sns_governance_service(&self) -> SNSGovernanceService {
        SNSGovernanceService(self.sns_governance_canister_id)
    }
    fn new() -> Self {
        Self {
            sns_governance_canister_id: Principal::from_slice(&[]),
            cm_main: Principal::from_slice(&[]),
            register_neuron_owner_locks: HashSet::new(),
            last_sns_reward_event_of_the_previous_cts_reward_event: 0,
            latest_sns_reward_event_done_parsing: 0,
            lock: false,
            current_built_up_cts_rewards_cycles: 0,
            current_built_up_cts_rewards_tokens: Vec::new(),
            sum_built_up_neuron_rewards: 0,         
        }
    }
}


pub struct Shareholder {
    // can only be registered by the neuron owner putting this principal as a hotkey on one of his/her neurons, 
    // so then this canister will check and register this field for the neuron-owner. The payouts go to the cts-website-ii-principal  
    neuron_owner_ids: Vec<Principal>, // max MAX_NEURON_OWNER_IDS_PER_SHAREHOLDER
    cycles_maturity: Cycles, // the cts-maturity currently held by this neuron for the cycles currency.
    token_maturities: Vec<u64>, // a list of the cts-maturity currently held by this neuron for each cm_tc in the same sequence as the cm_tcs are held in the cm_main.trade_contracts. 
    built_up_neuron_rewards: u64, // built-up neuron_reward_e8s sum of each neuron_reward_e8s for every sns-governance-reward-event within this cts-payout-event.
} 
// make sure to keep in the mind MAX_NUMBER_OF_TCS_ABLE_TO_BE_HANDLED.
use ic_stable_structures::storable::{Storable, Bound};
use std::borrow::Cow;
impl Storable for Shareholder {
    const BOUND: Bound = Bound::Bounded {
        max_size: 1024,
        is_fixed_size: false
    };

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        unimplemented!()
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        unimplemented!()
    }
}



const SP_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);
const SHAREHOLDERS_MEMORY_ID: MemoryId = MemoryId::new(1);
const NEURON_OWNERS_AS_CTS_USERS_MEMORY_ID: MemoryId = MemoryId::new(2);


pub const MAX_NEURON_OWNER_IDS_PER_SHAREHOLDER: usize = 5;

pub const MAX_REGISTER_NEURON_OWNER_LOCKS: usize = 200;

pub const CTS_USER_NEURON_PERMISSIONS: [i32; 2] = [3, 4]; // voting and submitting a proposal
pub const NEURON_OWNER_NEURON_PERMISSIONS: [i32; 11] = [0,1,2,3,4,5,6,7,8,9,10]; // every permission

pub const MAX_LIMIT_LIST_NEURONS: u32 = 500;

// THIS CONSTANT IS NOT A DIAL, IT IS A MARKER FOR THE CURRENT AMOUNT OF TCS THAT THIS CANISTER CAN HANDLE WITH IT'S CURRENT IMPLEMENTATION OF THE STABLE-STRUCTURES-BOUNDS.
// EACH SHAREHOLDER HOLDS A Vec<u64> THE LENGTH OF THE NUMBER OF THIS CONSTANT. 
// CHANGING THIS CONSTANT WILL BREAK THE CURRENT SHAREHOLDERS STORABLE IMPLEMENTATION.
pub const MAX_NUMBER_OF_TCS_ABLE_TO_BE_HANDLED: usize = 500; // check in the heartebeat timer cts-reward-event that the number of tcs in the cm-main has not exceeded this constant. if it has, then stop and wait for manual upgrade, don't do any reward events.  



thread_local! {
    static SP_DATA: RefCell<SPData> = RefCell::new(SPData::new());
    
    // stable-structures
    // cts-user-principal and data.
    static SHAREHOLDERS: RefCell<StableBTreeMap<Principal, Shareholder, VirtualMemory<DefaultMemoryImpl>>> = RefCell::new(StableBTreeMap::init(canister_tools::get_virtual_memory(SHAREHOLDERS_MEMORY_ID)));
    // neuron-owner-ids as cts-user-ids
    static NEURON_OWNERS_AS_CTS_USERS: RefCell<StableBTreeMap<Principal, Principal, VirtualMemory<DefaultMemoryImpl>>> = RefCell::new(StableBTreeMap::init(canister_tools::get_virtual_memory(NEURON_OWNERS_AS_CTS_USERS_MEMORY_ID)));
}

#[derive(CandidType, Deserialize)]
pub struct ShareholderPayoutsCanisterInit {
    sns_governance_canister_id: Principal,
    cm_main: Principal,
}

#[init]
fn init(q: ShareholderPayoutsCanisterInit) {
    canister_tools::init(&SP_DATA, SP_DATA_MEMORY_ID);   
	
    with_mut(&SP_DATA, |sp_data| {
        sp_data.sns_governance_canister_id = q.sns_governance_canister_id;
        sp_data.cm_main = q.cm_main;
    }); 
	
    set_timer();
}

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();    
}

#[post_upgrade]
fn post_upgrade() {
    canister_tools::post_upgrade(&SP_DATA, SP_DATA_MEMORY_ID, None::<fn(SPData) -> SPData>);    

    set_timer();
}



fn check_neuron_permissions(neuron_permission_types: &Vec<i32>, check_permissions: &[i32]) -> bool {
    let mut good = true;
    for check_permission in check_permissions.iter() {
        if neuron_permission_types.contains(check_permission) == false {
            good = false;
            break;
        }
    }
    good
}

#[derive(CandidType, Deserialize)]
pub struct RegisterNeuronOwnerQuest {
    neuron_owner_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub enum RegisterNeuronOwnerError {
    // neurons are not found with a hotkey set as this cts-user
    NeuronsNotFound,
	// a neuron-owner must only be registered to one cts user.
    NeuronOwnerAlreadyRegistered,
    
    // only a certain amount of neuron-owner-ids can be registered per shareholder.
    MaxNeuronOwnerIdsPerShareholder,
    
    CallerIsInADifferentCall,
    NeuronOwnerIdIsInADifferentCall,
    
    MaxNumberOfOngoingCalls, // canister is busy, locks are full
	
    ListNeuronsCallError(CallError),
	
}

type RegisterNeuronOwnerResult = Result<(), RegisterNeuronOwnerError>;

// lock the caller and neuron-owner-id on this method, add both principals into the lock-hashset.

// also make function to change the cts-user of a neuron-owner in case the neuron sells on id-geek. not a priority though.

#[update]
pub async fn register_neuron_owner(q: RegisterNeuronOwnerQuest) -> RegisterNeuronOwnerResult {
 	
    let cts_user: Principal = ic_cdk::api::caller();
 	
    with(&NEURON_OWNERS_AS_CTS_USERS, |d| {
        // check if neuron-owner is already registered. 
        // must only be able to register a neuron-owner-id with a cts-user once.  
        if let Some(_) = d.get(&q.neuron_owner_id) {
            return Err(RegisterNeuronOwnerError::NeuronOwnerAlreadyRegistered);
        }
        Ok(())
    })?;
 	
    with(&SHAREHOLDERS, |shareholders| {
        if let Some(shareholder) = shareholders.get(&cts_user) {
            if shareholder.neuron_owner_ids.len() >= MAX_NEURON_OWNER_IDS_PER_SHAREHOLDER {
                return Err(RegisterNeuronOwnerError::MaxNeuronOwnerIdsPerShareholder);
            }
        }
        Ok(())
    })?;
 	
    with_mut(&SP_DATA, |d| {
        // check lock
        if d.register_neuron_owner_locks.contains(&cts_user) {
            return Err(RegisterNeuronOwnerError::CallerIsInADifferentCall);
        }
        if d.register_neuron_owner_locks.contains(&q.neuron_owner_id) {
            return Err(RegisterNeuronOwnerError::NeuronOwnerIdIsInADifferentCall);
        }
        if d.register_neuron_owner_locks.len() >= MAX_REGISTER_NEURON_OWNER_LOCKS {
            return Err(RegisterNeuronOwnerError::MaxNumberOfOngoingCalls);
        }	
        // lock	    
        d.register_neuron_owner_locks.insert(cts_user);
        d.register_neuron_owner_locks.insert(q.neuron_owner_id);
        Ok(())
    })?;
	
    // make sure to unlock cts_user and q.neuron_owner_id on errors after here
    let unlock = |sp_data: &mut SPData| {
        sp_data.register_neuron_owner_locks.remove(&cts_user);
        sp_data.register_neuron_owner_locks.remove(&q.neuron_owner_id);
    };
	
    match with(&SP_DATA, |d| d.sns_governance_service()).list_neurons(
        ListNeurons{
            of_principal: Some(q.neuron_owner_id),
            limit: MAX_LIMIT_LIST_NEURONS,
            start_page_at: None,
        }
    ).await {
        Ok((list_neurons_response,)) => {
            let mut good: bool = false;
            'outer: for neuron in list_neurons_response.neurons.iter() {
                // must be both true for the same neuron.
                let mut found_permission_for_the_cts_user = false;
                let mut found_permission_for_the_neuron_owner_id = false;
                for neuron_permission in neuron.permissions.iter() {
                    if let Some(neuron_permission_principal) = neuron_permission.principal {
                        
                        if neuron_permission_principal == cts_user 
                        && check_neuron_permissions(&neuron_permission.permission_type, &CTS_USER_NEURON_PERMISSIONS[..]) { 
                            found_permission_for_the_cts_user = true;
                        } 
                        else if neuron_permission_principal == q.neuron_owner_id 
                        && check_neuron_permissions(&neuron_permission.permission_type, &NEURON_OWNER_NEURON_PERMISSIONS[..]) { 
                            found_permission_for_the_neuron_owner_id = true;
                        }
                        if found_permission_for_the_cts_user == true 
                        && found_permission_for_the_neuron_owner_id == true {
                            good = true;
                            break 'outer;
                        }
                    }
                }
            }
	        
            if good == true {
	                        	
                with_mut(&NEURON_OWNERS_AS_CTS_USERS, |d| {
                    d.insert(q.neuron_owner_id, cts_user);
                });

                with_mut(&SHAREHOLDERS, |shareholders| {
                    match shareholders.get(&cts_user) {
                        None => {
                            shareholders.insert(
                                cts_user,
                                Shareholder{
                                    neuron_owner_ids: vec![q.neuron_owner_id],
                                    cycles_maturity: 0,
                                    token_maturities: Vec::new(), 
                                    built_up_neuron_rewards: 0,
                                }   
                            );
                        }
                        Some(mut shareholder) => {
                            shareholder.neuron_owner_ids.push(q.neuron_owner_id);
                            shareholders.insert(cts_user, shareholder);
                        }
                    }
                });

                return Ok(());
	        
            } else {
                // if no neuron is found with a hotkey as the cts_user, return err
                with_mut(&SP_DATA, unlock);
                return Err(RegisterNeuronOwnerError::NeuronsNotFound);
            }
        }
        Err(call_error) => {
            with_mut(&SP_DATA, unlock);
            return Err(RegisterNeuronOwnerError::ListNeuronsCallError(call_error_as_u32_and_string(call_error)));
        }
    }
}




fn set_timer() {
    ic_cdk_timers::set_timer_interval(
        Duration::from_secs(60 * 60 * 24),
        || ic_cdk::spawn(timer()),
    );
}

// once a day
async fn timer() {
    
    // check if lock is on
    let turn_lock_on_result: Result<(), ()> = with_mut(&SP_DATA, |sp_data| {
        if sp_data.lock == true {
            return Err(()); 
        }
        // lock on
        sp_data.lock = true;
        Ok(())
    });
    if turn_lock_on_result.is_err() {
        return;
    }
        
    timer_with_lock_on().await;
        
    // lock off
    with_mut(&SP_DATA, |sp_data| {
        sp_data.lock = false;
    });
}

// return on an error.
async fn timer_with_lock_on() {
    // call list_neurons and catch up with the latest reward-events. add the neuron_reward_e8s of each unread-sns-reward-event onto the shareholder.built_up_neuron_reward_e8s
    // there might be chunked list_neurons calls, on the first list_neurons call, save the latest reward-event.end_timestamp_seconds and only count till that saved reward-event during the later chunks-calls-to-list-neuron. 
    let sns_governance_service = with(&SP_DATA, |d| d.sns_governance_service());
    
    // start from the latest reward-event if this is the first time doing this.
    
    // first go through list_neurons chunks and collect every neuron_reward_e8s of each reward-event for the processing, index by neuron-owner-principal
    // then apply to shareholders list in single message execution.
    // because we need to make sure we are able to get every neuron in every chunk before we apply the neuron_rewards.
    
    // start from the second-to-earliest reward-event available, do not use the earliest reward-event because it can be gone in between the chunks-calls.
    // if for some reason it misses a few reward events, just skip them and start from the second-to-earliest-reward-event-available.
    
    /*Principal = neuron-owner-id*/
    /*u64 = sum of the neuron_reward_e8s for all neurons and for all sns-reward-events within this cts-payout-event*/
    let mut neuron_owners_neuron_rewards: HashMap<Principal, u64> = HashMap::new(); 
    let mut start_page_at: Option<NeuronId> = None; // the last neuron-id gotten so far.
    // on the first chunk, choose these reward events to count for every other chunk.
    let mut count_sns_reward_events_timestamps: Vec<u64> = Vec::new();
    loop {
        match sns_governance_service.list_neurons(
            ListNeurons{
                of_principal: None,
                limit: MAX_LIMIT_LIST_NEURONS,
                start_page_at,
            } 
        ).await {
            Ok((sponse,)) => {
                
                if sponse.neurons.len() == 0 {
                    break;
                }
                
                // must be Some now.
                start_page_at = Some(match sponse.neurons.last().unwrap().id.clone() { // unwrap because we made sure len != 0
                    Some(neuron_id) => neuron_id,
                    None => {
                        ic_cdk::print("strange, a neuron is without an id.");
                        return;
                    }  
                });
                
                if count_sns_reward_events_timestamps.len() == 0 {
                    // choose sns-reward-events to count
                    // return if there are no new reward-events
                    if let Some(first_neuron) = sponse.neurons.first() {
                        let latest_sns_reward_event_done_parsing: u64 = with(&SP_DATA, |sp_data| {
                            sp_data.latest_sns_reward_event_done_parsing
                        });
                        // remove earliest one so that the reward-events that we are counting don't slip away in between the list_neurons calls.
                        let mut first_neuron_reward_map: BTreeMap<u64, u64> = first_neuron.reward_event_end_timestamp_seconds_to_neuron_reward_e8s.clone(); 
                        if first_neuron_reward_map.len() == 0 {                                     
                            ic_cdk::print("zero reward-events on the neuron");
                            return;
                        }
                        first_neuron_reward_map.pop_first();
                        
                        // add not-yet-parsed sns-reward-events to the count_sns_reward_events_timestamps list.
                        for (reward_event_end_timestamp_seconds, _) in first_neuron_reward_map.into_iter() {
                            if reward_event_end_timestamp_seconds <= latest_sns_reward_event_done_parsing {
                                continue;
                            } else {
                                count_sns_reward_events_timestamps.push(reward_event_end_timestamp_seconds);
                            }
                        }
                    }	 
                    if count_sns_reward_events_timestamps.len() == 0 {
                        ic_cdk::print("No sns-reward-events to parse");
                        return;
                    }   
                }
                
                for neuron in sponse.neurons.iter() {
                    let neuron_owner: Principal = match neuron.permissions.iter().find(
                        |neuron_permission| { 
                            check_neuron_permissions(&neuron_permission.permission_type, &NEURON_OWNER_NEURON_PERMISSIONS[..])
                        }
                    ) {
                        Some(neuron_permission) => {
                            // sns-governance shows this as an option. 
                            match neuron_permission.principal {
                                Some(p) => p,
                                None => {
                                    ic_cdk::print("Neuron owner permission found but principal field is None.");
                                    continue;
                                }
                            }
                        }
                        None => {
                            ic_cdk::print("Neuron found with zero owners. Owner means a principal has every neuron-permission.");
                            continue;
                        }
                    };
                    
                    for count_sns_reward_event in count_sns_reward_events_timestamps.iter() {
                        match neuron.reward_event_end_timestamp_seconds_to_neuron_reward_e8s.get(&count_sns_reward_event) {
                            Some(neuron_reward_e8s) => {
                                *(neuron_owners_neuron_rewards.entry(neuron_owner).or_default()) += neuron_reward_e8s;
                            }
                            None => {
                                ic_cdk::print("Could not find reward-event on the neuron.");
                                // might be a new neuron or a neuron that didn't vote.
                                continue;
                            } 
                        }
                    }
                }                
            }
            Err(call_error) => {
                ic_cdk::print(&format!("list_neurons call error: {:?}", call_error));
                return;
            }        
        }
    }
    
    with(&NEURON_OWNERS_AS_CTS_USERS, |neuron_owners_as_cts_users| { 
        with_mut(&SHAREHOLDERS, |shareholders| {
            with_mut(&SP_DATA, |sp_data| {    
                for (neuron_owner, neuron_rewards) in neuron_owners_neuron_rewards.into_iter() {
                    if let Some(cts_user) = neuron_owners_as_cts_users.get(&neuron_owner) {
                        if let Some(mut shareholder) = shareholders.get(&cts_user) { // there should always be Some here 
                            shareholder.built_up_neuron_rewards += neuron_rewards;
                            shareholders.insert(cts_user, shareholder);
                            sp_data.sum_built_up_neuron_rewards += neuron_rewards;
                        }
                    }
                }
            });
        });
    });

    
    with_mut(&SP_DATA, |sp_data| {
        sp_data.latest_sns_reward_event_done_parsing = count_sns_reward_events_timestamps.iter().max().unwrap().clone(); // unwrap bc we made sure before that len() != 0 
    });
    
    // done parsing sns-reward-events.
    // ------- done -------

    
    // ------- start do cts-reward-event -------
        
    // call cm_main to get list of tcs and their ledgers.
    
    let tcs_and_ledgers: Vec<TradeContractIdAndLedgerId> = match call::<(), (Vec<(TradeContractIdAndLedgerId, TradeContractData)>,)>(
        with_mut(&SP_DATA, |sp_data| { sp_data.cm_main }),
        "view_icrc1_token_trade_contracts",
        ()
    ).await {
        Ok((v,)) => {
            v.into_iter().map(|t| t.0).collect()
        }
        Err(call_error) => {
            ic_cdk::print(&format!("view_icrc1_token_trade_contracts call error: {:?}", call_error));
            return;
        }
    };
    
    if tcs_and_ledgers.len() >= MAX_NUMBER_OF_TCS_ABLE_TO_BE_HANDLED {
        ic_cdk::print("cannot do cts-reward-event, cannot start collecting trade fees from tcs, because tcs_and_ledgers.len() >= MAX_NUMBER_OF_TCS_ABLE_TO_BE_HANDLED");
        return;
    }
    
    
    // loop through tcs, call each one to fill up with the trading-fees-collected if there is enough that it's worth it for the tc to send.
    
    for (i, tc) in tcs_and_ledgers.iter().map(|tc_and_ledger| tc_and_ledger.trade_contract_canister_id).enumerate() {
        match call::<(), (ShareholderPayoutsCollectTradeFeesSponse,)>(
            tc,
            "shareholder_payouts_collect_trade_fees",
            ()
        ).await {
            Ok((s,)) => {
                // add these trading-fees-collected to the sp_data.current_built_up_cts_rewards
                with_mut(&SP_DATA, |sp_data| {
                    sp_data.current_built_up_cts_rewards_cycles = sp_data.current_built_up_cts_rewards_cycles.saturating_add(s.cb_cycles_sent);
                    // if there are new tcs, push new values onto the current_built_up_cts_rewards.
                    if sp_data.current_built_up_cts_rewards_tokens.len() < i + 1 {
                        sp_data.current_built_up_cts_rewards_tokens.push(s.tokens_sent as u64); // in the sequence of the cm_main's list.
                    } else {
                        sp_data.current_built_up_cts_rewards_tokens[i] = sp_data.current_built_up_cts_rewards_tokens[i].saturating_add(s.tokens_sent as u64);
                    }
                });            
            }
            Err(call_error) => {
                ic_cdk::print(&format!("shareholder_payouts_collect_trade_fees call error on tc: {}: {:?}", tc, call_error));
                continue;
            }
        }
    }
    
    // loop through the current_built_up_cts_rewards, if there is enough of a tcs-built-up-rewards, do a payout-event for that tc.
    
    // check if there is at least one tc that we can do a cts-reward-event for. 
    // if there is at least one tc that we can do a cts-reward-event for, then we'll do a cts-reward-event and payout shareholders and re-set the shareholders' built_up_neuron_reward_e8s to 0.
    // if there is zero tcs that we can do a cts-reward-event for, then we won't do a cts-reward-event.
    
    with_mut(&SP_DATA, |sp_data| {
        with_mut(&SHAREHOLDERS, |shareholders| {
            
            let mut at_least_one_tc_payout: bool = false;
            
            let minimum_built_up_tokens_or_cycles_for_a_payout: u64 = (shareholders.len() * 100) as u64; 
            
            if sp_data.current_built_up_cts_rewards_cycles >= minimum_built_up_tokens_or_cycles_for_a_payout as u128 {
                at_least_one_tc_payout = true;
            } else {
                for built_up_tokens in sp_data.current_built_up_cts_rewards_tokens.iter() {
                    if *built_up_tokens >= minimum_built_up_tokens_or_cycles_for_a_payout {
                        at_least_one_tc_payout = true;
                    }
                } 
            }
            if at_least_one_tc_payout == true 
            && sp_data.sum_built_up_neuron_rewards > 0 { // portant
                // do cts-reward-event
                
                // since the sum of the payouts for all neurons for a specific token can be less than the amount available due to division and stuff.
                // so we count here the quantity of the payouts.
                let mut cycles_payouts_sum: Cycles = 0;
                let mut tokens_payouts_sums: Vec<u64> = vec![];
                
                // read the shareholders once, and set the shareholders once
                // shareholders.iter holds an immutable reference so we won't be able to insert back into the map if we user iter. 
                // so what we do is we collect the keys for the map first, then loop through the keys where we can get and set.                
                
                let shareholders_set: BTreeSet<Principal> = with(&NEURON_OWNERS_AS_CTS_USERS, |d| { d.iter().map(|(_k,v)| v).collect::<BTreeSet<Principal>>() }); // there is no .keys() method on the stablebtree-map. easiest way to get list of the keys of the SHAREHOLDERS map. 
                
                for shareholder in shareholders_set {  
                    let mut shareholder_data: Shareholder = match shareholders.get(&shareholder) { Some(d) => d, None => continue, }; // there should always be some since the cts-user is on the neuron_owners_as_cts_users-map.
                    
                    if shareholder_data.built_up_neuron_rewards == 0 {
                        continue;
                    }
                    
                    let shareholder_reward_portion_one_out_of: u64 = sp_data.sum_built_up_neuron_rewards / shareholder_data.built_up_neuron_rewards;
                    
                    // only if we are doing a payout for the cycles
                    if sp_data.current_built_up_cts_rewards_cycles >= minimum_built_up_tokens_or_cycles_for_a_payout as Cycles {
                        let shareholder_payout = sp_data.current_built_up_cts_rewards_cycles / (shareholder_reward_portion_one_out_of as u128);
                        shareholder_data.cycles_maturity = shareholder_data.cycles_maturity.saturating_add(shareholder_payout);
                        cycles_payouts_sum = cycles_payouts_sum.saturating_add(shareholder_payout);
                    }
                    
                    for (i, built_up_tokens) in sp_data.current_built_up_cts_rewards_tokens.iter().enumerate() {
                        // only if we are doing a payout for this token
                        if *built_up_tokens >= minimum_built_up_tokens_or_cycles_for_a_payout {
                            let shareholder_payout = built_up_tokens / shareholder_reward_portion_one_out_of;
                            for list in [&mut shareholder_data.token_maturities, &mut tokens_payouts_sums] {
                                if list.len() < i + 1 {
                                    list.push(shareholder_payout);
                                } else {
                                    list[i] = list[i].saturating_add(shareholder_payout);
                                }
                            }
                        }
                    }
                    
                    shareholder_data.built_up_neuron_rewards = 0; // reset shareholder built-up-neuron-rewards
                    shareholders.insert(shareholder, shareholder_data);        
                }
                
                // reset global sum_built_up_neuron_rewards
                sp_data.sum_built_up_neuron_rewards = 0;
                // reset the sp_data.current_built_up_cts_rewards_ to sp_data.current_built_up_cts_rewards_ - _payouts_sum
                sp_data.current_built_up_cts_rewards_cycles = sp_data.current_built_up_cts_rewards_cycles.saturating_sub(cycles_payouts_sum); 
                for (built_up_tokens, token_payout) in sp_data.current_built_up_cts_rewards_tokens.iter_mut().zip(tokens_payouts_sums.into_iter()) {
                    *built_up_tokens = built_up_tokens.saturating_sub(token_payout); 
                }
                
                ic_cdk::print("CTS-SHAREHOLDER-PAYOUT!");
                return; // will return anyway since this is the end of the function but whatever.                
            } else {
                ic_cdk::print("Not doing a cts-reward-event, sum_built_up_neuron_rewards == 0.");
                return; // will return anyway since this is the end of the function but whatever.
            }
        });
    });    
}
