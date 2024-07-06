use ic_cdk::{
    init,
    pre_upgrade,
    post_upgrade,
	update, 
    query,
};
use ic_stable_structures::StableBTreeMap;

use std::time::Duration;



#[cfg(test)]
mod tests;


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
    current_built_up_cts_rewards: Vec<u64>, // in the same sequence as the cm_main. each value will be reset after a payout-event of that cm_tc.
}

pub struct Shareholder {
    // can only be registered by the neuron owner putting this principal as a hotkey on one of his/her neurons, 
    // so then this canister will check and register this field for the neuron-owner. The payouts go to the cts-website-ii-principal  
    neuron_owner_ids: Vec<Principal>, // max MAX_NEURON_OWNER_IDS_PER_SHAREHOLDER
	balances: Vec<u64>, // a list of the cts-maturity currently held by this neuron for each cm_tc in the same sequence as the cm_tcs are held in the cm_main.trade_contracts. 
	built_up_neuron_reward_e8s: u64 = // built-up neuron_reward_e8s sum of each neuron_reward_e8s for every sns-governance-reward-event within this cts-payout-event.
}



const SP_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);
const SHAREHOLDERS_MEMORY_ID: MemoryId = MemoryId::new(1);
const NEURON_OWNERS_AS_CTS_USERS_MEMORY_ID: MemoryId = MemoryId::new(2);


pub const MAX_NEURON_OWNER_IDS_PER_SHAREHOLDER: usize = 5;

pub const MAX_REGISTER_NEURON_OWNER_LOCKS: usize = 200;

thread_local! {
    static SP_DATA: RefCell<SPData> = RefCell::new(SPData::new());
    
    // stable-structures
    // cts-user-principal and data.
    static SHAREHOLDERS: RefCell<StableBTreeMap<Principal, Shareholder, VirtualMemory<DefaultMemoryImpl>>> = RefCell::new(StableBTreeMap::init(canister_tools::get_virtual_memory(SHAREHOLDERS_MEMORY_ID)));
    // neuron-owner-ids as cts-user-ids
    static NEURON_OWNERS_AS_CTS_USERS: RefCell<StableBTreeMap<Principal, Principal, VirtualMemory<DefaultMemoryImpl>>> = RefCell::new(StableBTreeMap::init(canister_tools::get_virtual_memory(NEURON_OWNERS_AS_CTS_USERS_MEMORY_ID)));
}


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



pub struct RegisterNeuronOwnerQuest {
    neuron_owner_id: Principal,
}

enum RegisterNeuronOwnerError {
    // neurons are not found with a hotkey set as this cts-user
    NeuronsNotFound,
	// a neuron-owner must only be registered to one cts user.
    NeuronOwnerAlreadyRegistered,
    
    CallerIsInADifferentCall,
    NeuronOwnerIdIsInADifferentCall,
    
    MaxNumberOfOngoingCalls, // canister is busy, locks are full
	
	ListNeuronsCallError(CallError),
	
}

type RegisterNeuronOwnerResult = Result<(), RegisterNeuronOwnerError>;

// lock the caller and neuron-owner-id on this method, add both principals into the lock-hashset.

#[update]
pub async fn register_neuron_owner(q: RegisterNeuronOwnerQuest) -> RegisterNeuronOwnerResult {
 	
 	let cts_user: Principal = ic_cdk::api::caller();
 	
 	with(&NEUORN_OWNERS_AS_CTS_USERS, |d| {
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
 	
	with(&SP_DATA, |d| {
	    // check lock
	    if d.register_neuron_owner_locks.contains(&cts_user) {
	        return Err(RegisterNeuronOwnerError::CallerIsInADifferentCall);
	    }
		if d.register_neuron_owner_locks.contains(&q.neuron_owner_id) {
	        return Err(RegisterNeuronOwnerError::NeuronOwnerIdIsInADifferentCall);
	    }
	    if d.register_neuron_owner_locks.len() >= MAX_REGISTER_NEURON_OWNER_LOCKS {
	        return Err(RegisterNeuronOwnerError::MaxNumberOfOngoingCalls),
	    }	
		// lock	    
	    d.register_neuron_owner_locks.insert(cts_user);
	    d.register_neuron_owner_locks.insert(q.neuron_owner_id);
	    Ok(())
	})?;
	
	// make sure to unlock cts_user and q.neuron_owner_id on errors after here
	let unlock = |sp_data: &mut SPData| {
	    sp_data.register_neuron_owner_locks.remove(cts_user);
	    sp_data.register_neuron_owner_locks.remove(q.neuron_owner_id);
	};
	
	use outsiders::sns_governance::{Service as SNSGovernanceService, ListNeurons, ListNeuronsResponse};
	let sns_governance_service = SNSGovernanceService(with(&SP_DATA, |sp_data| { sp_data.sns_governance_canister_id }));
	
	match sns_governance_service.list_neurons(
	    ListNeurons{
	        of_principal: Some(q.neuron_owner_id),
	        limit: 500, // how many in one shot?
	    	start_page_at: None,
	    }
	).await {
	    Ok(list_neurons_response) => {
	        for neuron in list_neurons_response.neurons.iter() {
	            for neuron_permission in neuron.permissions.iter() {
	                if let Some(neuron_permission_principal) = neuron_permission.principal {
	                    if neuron_permission_principal == cts_user {
	                        // good.
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
                                                balances: Vec::new(), 
                                                built_up_neuron_reward_e8s: Vec::new();
	                                       	}
	                                    );
	                                }
	                                Some(shareholder) => {
	                                    shareholder.neuron_owner_ids.push(q.neuron_owner_id);
	                                } 
	                            }
	                        });
	                        
	                        return Ok(());
	                    }
	                }
	           	}
	        }
	        
	        // if no neuron is found with a hotkey as the cts_user, return err
	        with_mut(&SP_DATA, |d| unlock(d));
	        return return Err(RegisterNeuronOwnerError::NeuronsNotFound);
	    }
	    Err(call_error) => {
	        with_mut(&SP_DATA, |d| unlock(d));
	        return return Err(RegisterNeuronOwnerError::ListNeuronsCallError(call_error_as_u32_and_string(call_error)));
	    }
	}
}




fn set_timer() {
    ic_cdk_timers::set_timer_interval(
        Duration::from_days(1),
        ic_cdk::spawn(timer());
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
    
    // call cm_main to get list of tcs and their ledgers.
    
    // loop through tcs, call each one to fill up with the trading-fees-collected if there is enough that it's worth it to send.
    // add these trading-fees-collected to the sp_data.current_built_up_cts_rewards
    // if there are new tcs, push new values onto the current_built_up_cts_rewards.
    
    // loop through the current_built_up_cts_rewards, if there is enough of a tcs-built-up-rewards, do a payout-event for that tc.
        
}