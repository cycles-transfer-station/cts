use sha2::Digest;
use crate::{
    ic_ledger_types::{
        IcpIdSub,
        IcpId,
    },
    consts::{
        NANOS_IN_A_SECOND, MAINNET_SNS_GOVERNANCE,
    },
    types::{
        Cycles,
        CallError,
        cm::tc::CyclesPerToken,
    },
    icrc::{Tokens},
};
use ic_cdk::{
    trap,
    api::{is_controller, call::RejectionCode}
};
use candid::Principal;
use std::thread::LocalKey;
use std::cell::Cell;
use ic_stable_structures::Memory;


pub use ic_cdk::api::time as time_nanos_u64;
pub fn time_nanos() -> u128 { time_nanos_u64() as u128 }
pub fn time_seconds() -> u128 { time_nanos() / NANOS_IN_A_SECOND as u128 }


mod structural_hash;
pub use structural_hash::structural_hash;



pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher: sha2::Sha256 = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}



pub mod localkey {
    pub mod refcell {
        use std::{
            cell::RefCell,
            thread::LocalKey,
        };

        pub fn with<T: 'static, R, F>(s: &'static LocalKey<RefCell<T>>, f: F) -> R
        where 
            F: FnOnce(&T) -> R 
        {
            s.with(|b| {
                f(&*b.borrow())
            })
        }
        
        pub fn with_mut<T: 'static, R, F>(s: &'static LocalKey<RefCell<T>>, f: F) -> R
        where 
            F: FnOnce(&mut T) -> R 
        {
            s.with(|b| {
                f(&mut *b.borrow_mut())
            })
        }
        /*
        pub unsafe fn get<T: 'static>(s: &'static LocalKey<RefCell<T>>) -> &T {
            let pointer: *const T = with(s, |i| { i as *const T });
            &*pointer
        }
        
        pub unsafe fn get_mut<T: 'static>(s: &'static LocalKey<RefCell<T>>) -> &mut T {
            let pointer: *mut T = with_mut(s, |i| { i as *mut T });
            &mut *pointer
        }
        */
    }
    pub mod cell {
        use std::{
            cell::Cell,
            thread::LocalKey
        };
        pub fn get<T: 'static + Copy>(s: &'static LocalKey<Cell<T>>) -> T {
            s.with(|c| { c.get() })
        }
        pub fn set<T: 'static + Copy>(s: &'static LocalKey<Cell<T>>, v: T) {
            s.with(|c| { c.set(v); });
        }
        
    }
}







pub fn principal_as_thirty_bytes(p: &Principal) -> [u8; 30] {
    let mut bytes: [u8; 30] = [0; 30];
    let p_bytes: &[u8] = p.as_slice();
    bytes[0] = p_bytes.len() as u8; 
    bytes[1 .. p_bytes.len() + 1].copy_from_slice(p_bytes); 
    bytes
}

pub fn thirty_bytes_as_principal(bytes: &[u8; 30]) -> Principal {
    Principal::from_slice(&bytes[1..1 + bytes[0] as usize])
} 


#[test]
fn thirty_bytes_principal() {
    let test_principal: Principal = Principal::from_slice(&[0,1,2,3,4,5,6,7,8,9]);
    assert_eq!(test_principal, thirty_bytes_as_principal(&principal_as_thirty_bytes(&test_principal)));
}




pub fn principal_icp_subaccount(principal: &Principal) -> IcpIdSub {
    let mut sub_bytes = [0u8; 32];
    sub_bytes[..30].copy_from_slice(&principal_as_thirty_bytes(principal));
    IcpIdSub(sub_bytes)
}

pub fn principal_token_subaccount(principal: &Principal) -> [u8; 32] {
    let mut sub_bytes = [0u8; 32];
    sub_bytes[..30].copy_from_slice(&principal_as_thirty_bytes(principal));
    sub_bytes
}


pub fn user_icp_id(cts_id: &Principal, user_id: &Principal) -> IcpId {
    IcpId::new(cts_id, &principal_icp_subaccount(user_id))
}




pub fn tokens_transform_cycles(tokens: Tokens, cycles_per_token: Cycles) -> Cycles {
    tokens * cycles_per_token
}

pub fn cycles_transform_tokens(cycles: Cycles, cycles_per_token: Cycles) -> Tokens {
    if cycles_per_token == 0 {
        return 0;
    }
    cycles / cycles_per_token
}




#[test]
fn test_tokens_cycles_transform() {
    let cycles_per_token: Cycles = 500_000_000;
    let tokens: Tokens = 10;
    assert_eq!(tokens.clone(), cycles_transform_tokens(tokens_transform_cycles(tokens.clone(), cycles_per_token), cycles_per_token));

    println!("{:?}", tokens_transform_cycles(tokens.clone(), cycles_per_token));
    println!("{:?}", cycles_transform_tokens(tokens_transform_cycles(tokens.clone(), cycles_per_token), cycles_per_token));

}




// round-robin on the cycles-transferrer-canisters
pub fn round_robin<T: Copy>(ctcs: &Vec<T>, round_robin_counter: &'static LocalKey<Cell<usize>>) -> Option<T> {
    match ctcs.len() {
        0 => None,
        1 => Some(ctcs[0]),
        l => {
            round_robin_counter.with(|ctcs_rrc| { 
                let c_i: usize = ctcs_rrc.get();                    
                if c_i <= l-1 {
                    let ctc: T = ctcs[c_i];
                    if c_i == l-1 { ctcs_rrc.set(0); } else { ctcs_rrc.set(c_i + 1); }
                    Some(ctc)
                } else {
                    ctcs_rrc.set(1); // we check before that the len of the ctcs is at least 2 in the first match                         
                    Some(ctcs[0])
                } 
            })
        }
    }
}











pub fn caller_is_controller_guard(caller: &Principal) {
    if is_controller(caller) == false {
        trap("Caller must be a controller for this method.");
    }
}




pub fn call_error_as_u32_and_string(t: (RejectionCode, String)) -> CallError {
    (t.0 as u32, t.1)
}



pub fn stable_read_into_vec<M: Memory>(memory: &M, start: u64, len: usize) -> Vec<u8> {
    let mut v: Vec<u8> = vec![0; len];
    memory.read(start, &mut v);    
    v
}


pub mod upgrade_canisters {
    
    use std::collections::HashSet;
    use candid::{CandidType, Deserialize, Principal};
    use outsiders::management_canister::{Service as ManagementCanisterService, ListCanisterSnapshotsArgs, TakeCanisterSnapshotArgs, Snapshot, SnapshotId};
    use ic_cdk::api::management_canister::main::{start_canister, stop_canister, CanisterIdRecord};
    use crate::{
        types::{CallError, CanisterCode},
        tools::call_error_as_u32_and_string,
        management_canister::{InstallCodeQuest, InstallCodeMode, install_code},    
    };    
    
    
    #[derive(CandidType, Deserialize)]
    pub struct ControllerUpgradeCSQuest {
        pub specific_cs: Option<HashSet<Principal>>, 
        pub new_canister_code: Option<CanisterCode>, 
        #[serde(with = "serde_bytes")]
        pub post_upgrade_quest: Vec<u8>,
        pub take_canisters_snapshots: bool
    }
    
    // options are for the steps, none means didn't call.
    #[derive(CandidType, Deserialize, Default, Debug, PartialEq, Eq, Clone)]
    pub struct UpgradeOutcome {
        pub stop_canister_result: Option<Result<(), CallError>>,
        pub take_canister_snapshot_result: Option<Result<Snapshot, CallError>>, // this can also contain an error for the list_canister_snapshots call which is done before any other calls if take_canister_snapshot = true
        pub install_code_result: Option<Result<(), CallError>>,    
        pub start_canister_result: Option<Result<(), CallError>>,
    }
        
    pub async fn upgrade_canisters(cs: Vec<Principal>, canister_code: &CanisterCode, post_upgrade_quest: &[u8], take_canisters_snapshots: bool) -> Vec<(Principal, UpgradeOutcome)> {    
        futures::future::join_all(cs.into_iter().map(|c| upgrade_canister_(c, canister_code, post_upgrade_quest, take_canisters_snapshots))).await // // use async fn upgrade_canister_, (not async block)
    }
    
    async fn upgrade_canister_(c: Principal, canister_code: &CanisterCode, post_upgrade_quest: &[u8], take_canister_snapshot: bool) -> (Principal, UpgradeOutcome) {
                
        let mut upgrade_outcome = UpgradeOutcome::default();
                
        let mc_service = ManagementCanisterService(Principal::management_canister());
        
        let mut possible_earliest_snapshot_id: Option<SnapshotId> = None;
        if take_canister_snapshot == true {
            match mc_service.list_canister_snapshots(ListCanisterSnapshotsArgs{ canister_id: c }).await.map(|t|t.0) {   
                Ok(vec_snapshots) => {
                    if vec_snapshots.len() != 0 {
                        possible_earliest_snapshot_id = Some(vec_snapshots.into_iter().next().unwrap().id);
                    }
                }
                Err(call_error) => {
                    upgrade_outcome.take_canister_snapshot_result = Some(Err((0, format!("Error when calling list_canister_snapshots for canister_id: {}:\n{:?}", c, call_error))));
                    return (c, upgrade_outcome)
                }
            }
        }
        
        upgrade_outcome.stop_canister_result = Some(stop_canister(CanisterIdRecord{canister_id: c}).await.map_err(call_error_as_u32_and_string));
        if upgrade_outcome.stop_canister_result.as_ref().unwrap().is_err() {
            return (c, upgrade_outcome);
        } 
        
        if take_canister_snapshot == true {
            upgrade_outcome.take_canister_snapshot_result = Some(mc_service.take_canister_snapshot(
                TakeCanisterSnapshotArgs{
                    canister_id: c,
                    replace_snapshot: possible_earliest_snapshot_id,
                }
            ).await.map(|t|t.0).map_err(call_error_as_u32_and_string));
        }
        
        if upgrade_outcome.take_canister_snapshot_result.is_none() 
        || upgrade_outcome.take_canister_snapshot_result.as_ref().unwrap().is_ok() {
            
            let a = InstallCodeQuest {
                mode: InstallCodeMode::upgrade,
                canister_id: c,
                wasm_module: canister_code.module(),
                arg: post_upgrade_quest,
            };
            upgrade_outcome.install_code_result = Some(install_code(a).await);
        
        }
        
        upgrade_outcome.start_canister_result = Some(start_canister(CanisterIdRecord{canister_id: c}).await.map_err(call_error_as_u32_and_string));
                
        return (c, upgrade_outcome);
    }
    
}



pub fn sns_validation_string<T: core::fmt::Debug>(q: T) -> String {
    format!("{:#?}", q)
}



pub fn caller_is_sns_governance_guard() {
    let must_be_principal: Principal = {
        use crate::consts::livetest::*;
        if [LIVETEST_CTS, LIVETEST_CM_MAIN].contains(&ic_cdk::api::id()) {
            LIVETEST_CONTROLLER
        } else {
            MAINNET_SNS_GOVERNANCE
        }
    };
    if ic_cdk::caller() != must_be_principal {
        trap("Caller must be the CTS SNS governance canister.");
    }
}



pub mod canister_status {
    use outsiders::management_canister::{Service as ManagementCanisterService, CanisterStatusArgs, CanisterStatusResult}; 
    use crate::{tools::call_error_as_u32_and_string, types::CallError};
    use candid::Principal;
    
    pub type ViewCanistersStatusSponse = (Vec<(Principal/*tc*/, CanisterStatusResult)>, Vec<(Principal, CallError)>);
    
    pub async fn view_canisters_status(cs: Vec<Principal>) -> ViewCanistersStatusSponse { 
        
        let management_canister_service = ManagementCanisterService(Principal::management_canister());
        
        let mut futures: Vec<_/*future*/> = Vec::new();
        
        for c in cs.iter().copied() {
            futures.push(
                management_canister_service.canister_status(CanisterStatusArgs{canister_id: c})
            );
        }
        
        let rs = futures::future::join_all(futures).await;
        
        let mut successes: Vec<(Principal, CanisterStatusResult)> = vec![];
        let mut errors: Vec<(Principal, CallError)> = vec![];
        
        for (c, r) in cs.into_iter().zip(rs.into_iter()) {
            match r {
                Ok((s,)) => {
                    successes.push((c, s));
                }
                Err(call_error) => {
                    errors.push((c, call_error_as_u32_and_string(call_error)));
                }
            }
        }
        
        (successes, errors)
    }
}


pub fn cycles_per_token_rate_as_f64(rate: CyclesPerToken, token_decimal_places: u8) -> f64 {
    let cycles_per_whole_token = rate * 10_u128.checked_pow(token_decimal_places as u32).unwrap();
    let s = cycles_as_tcycles_str(cycles_per_whole_token);
    use std::str::FromStr;
    f64::from_str(&s).unwrap()
}

fn cycles_as_tcycles_str(cycles: Cycles) -> String {
    token_quantums_as_token_str(cycles, 12)
}

fn token_quantums_as_token_str(quantums: u128, decimal_places: usize) -> String {
    let mut s = format!("{}", quantums);
    while s.len() < decimal_places + 1 {
        s.insert_str(0, "0");
    }
    let split_i = s.len() - decimal_places;
    s.insert_str(split_i, ".");
    while s.ends_with("0") || s.ends_with(".") {
        let mut brk = false;
        if s.ends_with(".") { brk = true; }
        s.pop().unwrap();
        if brk { break; }
    }
    s
}


#[test]
fn test_cycles_per_token_rate_as_f64() {
    assert_eq!(cycles_per_token_rate_as_f64(75489, 8), 7.5489);
    assert_eq!(cycles_per_token_rate_as_f64(869, 8), 0.0869);
    assert_eq!(cycles_per_token_rate_as_f64(7897, 8), 0.7897);
    assert_eq!(cycles_per_token_rate_as_f64(869, 8), 0.0869);
    assert_eq!(cycles_per_token_rate_as_f64(7548789629, 8), 754878.9629);
    assert_eq!(cycles_per_token_rate_as_f64(7548789629, 12), 7548789629.0);
    assert_eq!(cycles_per_token_rate_as_f64(7548789629, 3), 7.548789629);   
    assert_eq!(cycles_per_token_rate_as_f64(754000000, 8), 75400.0);   
    assert_eq!(cycles_per_token_rate_as_f64(7540021, 8), 754.0021);   
    assert_eq!(cycles_per_token_rate_as_f64(74567, 8), 7.4567);
}