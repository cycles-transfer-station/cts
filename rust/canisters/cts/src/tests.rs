use pocket_ic::{*, common::rest::RawEffectivePrincipal};
use candid::{Nat, Principal, CandidType, Deserialize};
use std::collections::{HashSet, HashMap};
use cts_lib::{
    consts::{TRILLION, MANAGEMENT_CANISTER_ID},
    tools::{principal_token_subaccount, cycles_transform_tokens},
    types::{CanisterCode, CallError, cycles_bank::UserCBMetrics, CyclesTransfer, CyclesTransferMemo},
    management_canister::{
        *,
        ManagementCanisterCanisterStatusRecord,
        CanisterIdRecord
    }
};
use icrc_ledger_types::icrc1::{account::Account, transfer::{TransferArg, TransferError}};
use crate::*;
use ic_cdk::api::management_canister::main::{CanisterInfoRequest,CanisterInfoResponse,CanisterChange,CanisterChangeOrigin,CanisterChangeDetails,CanisterInstallMode,CodeDeploymentRecord,FromCanisterRecord,ControllersChangeRecord,CreationRecord};
use more_asserts::*;


const LEDGER_TRANSFER_FEE: u128 = 10_000;
const CMC_RATE: u128 = 55555;
const ICP_MINTER: Principal = Principal::from_slice(&[1,1,1,1,1]);
const CMC: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,4,1,1]);
const NNS_GOVERNANCE: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,1,1,1]);
const ICP_LEDGER: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,2,1,1]);
const CTS_CONTROLLER: Principal = Principal::from_slice(&[0,1,2,3,4,5,6,7,8,9]);



#[test]
fn test_purchase_cycles_bank() {
    let (pic, cts, _cm_main) = cts_setup();
    let mut users_and_cbs: Vec<(Principal, Principal)> = Vec::new();
    let cb_module_hash: [u8; 32] = sha256(&std::fs::read("../../target/wasm32-unknown-unknown/debug/cycles_bank.wasm").unwrap());
    for i in 0..100/*(CB_CACHE_SIZE * 3)*/ {
        println!("i: {i}");
        pic.advance_time(core::time::Duration::from_secs(60*100));
        pic.tick();
        /*
        if i != 0 && i % 200 == 0 {
            std::thread::sleep(core::time::Duration::from_secs(20));
        }        
        */
        let user = Principal::from_slice(&(i+5000 as u64).to_be_bytes());
        // test purchase_cycles_bank
        let cb: Principal = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
        //println!("cb: {}", cb);
        users_and_cbs.push((user, cb));
        assert_ge!(
            pic.cycle_balance(cb),
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES - 10_000_000_000/*for the install and managment canister calls*/,
        );
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner:cts,subaccount: None}),
            ((MEMBERSHIP_COST_CYCLES / CMC_RATE) - (NEW_CYCLES_BANK_CREATION_CYCLES / CMC_RATE)) * (i+1) as u128
        );
        assert_eq!(cb_cycles_balance(&pic, cb, user), 0);        
        let latest_cbsm = controller_view_cbsms(&pic, cts).last().cloned().unwrap();
        // cts cbsm find_user
        let (cts_find_user_sponse,): (Option<CBSMUserData>,) = query_candid_as(
            &pic,
            latest_cbsm,
            cts,
            "find_user",
            (user,)
        ).unwrap();
        let cbsm_user_data = cts_find_user_sponse.unwrap();
        assert_eq!(
            cbsm_user_data,
            CBSMUserData {
                cycles_bank_canister_id: cb,
                first_membership_creation_timestamp_nanos: cbsm_user_data.first_membership_creation_timestamp_nanos, 
                cycles_bank_latest_known_module_hash: cb_module_hash,
                cycles_bank_lifetime_termination_timestamp_seconds: cbsm_user_data.first_membership_creation_timestamp_nanos / NANOS_IN_A_SECOND + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS,
                membership_termination_cb_uninstall_data: None
            }
        );
        // check metrics
        let (metrics,): (UserCBMetrics,) = query_candid_as(
            &pic,
            cb,
            user,
            "metrics",
            ()
        ).unwrap();
        //println!("{:?}", metrics.cts_cb_authorization);
        assert_eq!(
            metrics,
            UserCBMetrics{
                global_allocator_counter: metrics.global_allocator_counter,
                cycles_balance: 0,
                ctsfuel_balance: metrics.ctsfuel_balance,
                storage_size_mib: NEW_CYCLES_BANK_STORAGE_SIZE_MiB,
                lifetime_termination_timestamp_seconds: cbsm_user_data.first_membership_creation_timestamp_nanos / NANOS_IN_A_SECOND + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS,
                user_id: user,
                user_canister_creation_timestamp_nanos: cbsm_user_data.first_membership_creation_timestamp_nanos,
                storage_usage: 1310720,
                cycles_transfers_id_counter: 0,
                cycles_transfers_in_len: 0,
                cycles_transfers_out_len: 0,
                cm_trade_contracts: Vec::new(),   
                cts_cb_authorization: false,    
                cbsm_id: latest_cbsm,
            }
        );
        assert_ge!(metrics.ctsfuel_balance, NEW_CYCLES_BANK_CTSFUEL - 10_000_000_000);
        // check canister_status
        let (canister_status,): (ManagementCanisterCanisterStatusRecord,) = call_candid_as(
            &pic,
            MANAGEMENT_CANISTER_ID,
            RawEffectivePrincipal::CanisterId(cb.as_slice().to_vec()),
            cts,
            "canister_status",
            (CanisterIdRecord{canister_id: cb},)
        ).unwrap();
        //println!("canister_status cb controllers: {:?}", canister_status.settings.controllers);
        assert_eq!(
            canister_status,
            ManagementCanisterCanisterStatusRecord {
                status : ManagementCanisterCanisterStatusVariant::running,
                settings: ManagementCanisterCanisterSettings{
                    controllers : {let mut v = vec![cts, latest_cbsm, cb]; v.sort(); v},
                    compute_allocation : 0,
                    memory_allocation : 0, //NEW_CYCLES_BANK_NETWORK_MEMORY_ALLOCATION_MiB * MiB as u128,
                    freezing_threshold : NEW_CYCLES_BANK_FREEZING_THRESHOLD,
                },
                module_hash: Some(cb_module_hash),
                memory_size: canister_status.memory_size,
                cycles: canister_status.cycles // already checked above using pic.cycle_balance
            }
        );
        assert_le!(
            canister_status.memory_size,
            12000000, // why 12 MiB?
        );
        
        //pic.advance_time(core::time::Duration::from_nanos(MINIMUM_CB_AUTH_DURATION_NANOS * 2));
    }
    let (view_cbsms,): (Vec<Principal>,) = query_candid_as(&pic, cts, CTS_CONTROLLER, "controller_view_cbsms", ()).unwrap();
    assert_eq!(view_cbsms.len(), 1);    
    
    // do in a separate loop, cause it is testing the cb-cache and cb-auths pruning mechanism
    for (user,cb) in users_and_cbs.iter() {
        pic.advance_time(core::time::Duration::from_secs(60));
        pic.tick();
        // test find_cycles_bank
        let (find_user_cb_r,): (Result<Option<Principal>, FindCyclesBankError>,) = call_candid_as(
            &pic,
            cts,
            RawEffectivePrincipal::None,
            *user,
            "find_cycles_bank",
            ()
        ).unwrap();
        let find_user_cb: Principal = find_user_cb_r.unwrap().unwrap();     
        assert_eq!(*cb, find_user_cb);
        // test cts-cb-auth
        let (set_cb_auth_r,): (Result<(), SetCBAuthError>,) = call_candid_as(
            &pic,
            cts,
            RawEffectivePrincipal::None,
            *user,
            "set_cb_auth",
            ()
        ).unwrap();
        set_cb_auth_r.unwrap();
        let (cb_auth,): (Vec<u8>,) = query_candid_as(
            &pic,
            cts,
            *user,
            "get_cb_auth",
            (cb,)
        ).unwrap();
        local_put_ic_root_key(&pic, cb.clone());
        let _: () = call_candid_as(
            &pic,
            *cb,
            RawEffectivePrincipal::None,
            *user,
            "user_upload_cts_cb_authorization",
            (cb_auth,)
        ).unwrap();
        assert_eq!(query_candid_as::<(),(UserCBMetrics,)>(&pic,*cb,*user,"metrics",()).unwrap().0.cts_cb_authorization, true);
    }
}

#[test]
fn test_cb_transfer_icrc1() {
    let (pic, cts, _cm_main) = cts_setup();
    let user = Principal::from_slice(&(0123456789 as u64).to_be_bytes());
    let cb: Principal = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
    
    let cb_canister_cycles_balance = pic.cycle_balance(cb);
    println!("cb_canister_cycles_balance: {cb_canister_cycles_balance}");
    mint_icp(&pic, Account{owner: cb,subaccount: None}, 500_000_000_000);
        
    for ii in 0..10 {
        let (tr,): (Result<Vec<u8>, CallError>,) = call_candid_as(
            &pic,
            cb,
            RawEffectivePrincipal::None,
            user,
            "transfer_icrc1",
            (ICP_LEDGER, encode_one(
                TransferArg{
                    from_subaccount: None,
                    to: Account{
                        owner: user,
                        subaccount: None
                    },
                    fee: Some(LEDGER_TRANSFER_FEE.into()),
                    created_at_time: None,
                    memo: Some(ii.into()),
                    amount: 5.into(),
                }
            ).unwrap()),
        ).unwrap();
        candid::decode_one::<Result<Nat, TransferError>>(&tr.unwrap()).unwrap().unwrap();
    }
    
    let cb_canister_cycles_balance = pic.cycle_balance(cb);
    println!("cb_canister_cycles_balance: {cb_canister_cycles_balance}");    
   
}

#[test]
fn test_upgrade_cbs() {
    let (pic, cts, _cm_main) = cts_setup();
    let mut users_and_cbs: Vec<(Principal, Principal)> = Vec::new();
    for i in 0..10 {
        let user = Principal::from_slice(&(i+500 as u64).to_be_bytes());
        let cb = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
        users_and_cbs.push((user, cb));    
    }
    
    let cbsms = controller_view_cbsms(&pic, cts);
    let cb_module: Vec<u8> = std::fs::read("../../target/wasm32-unknown-unknown/debug/cycles_bank.wasm").unwrap();
    let cb_module_hash: [u8; 32] = sha256(&cb_module);
    let (upgrade_r,): (Result<Vec<(Principal, UpgradeOutcome)>, CallError>,) = call_candid_as(
        &pic,
        cts,
        RawEffectivePrincipal::None,
        CTS_CONTROLLER,
        "controller_upgrade_cbsm_cbs_chunk",
        (cbsms[0], ControllerUpgradeCSQuest{
            new_canister_code: Some(CanisterCode::new(cb_module)),
            specific_cs: Some(users_and_cbs.iter().map(|t| t.1).collect()),
            post_upgrade_quest: candid::encode_args(()).unwrap(),
        })
    ).unwrap();
    let uos: Vec<(Principal, UpgradeOutcome)> = upgrade_r.unwrap();
    println!("uos.len(): {}", uos.len());
    //println!("uos: {:?}", uos);
    for (cb, uo) in uos.into_iter() {
        assert_eq!(
            uo,
            UpgradeOutcome{
                stop_canister_result: Some(Ok(())),
                install_code_result: Some(Ok(())),    
                start_canister_result: Some(Ok(())),
            }
        );
        let (call_canister_r,): (Result<Vec<u8>, CallError>,) = call_candid_as(
            &pic,
            cts, // only canisters can call management-method: canister_info.
            RawEffectivePrincipal::None,
            CTS_CONTROLLER,
            "controller_call_canister",
            (ControllerCallCanisterQuest {
                callee: MANAGEMENT_CANISTER_ID,
                method_name: "canister_info".to_string(),
                arg_raw: candid::encode_one(CanisterInfoRequest{ canister_id: cb, num_requested_changes: Some(4) }).unwrap(),
                cycles: 0
            },)
        ).unwrap();
        let canister_info_sponse: CanisterInfoResponse = candid::decode_one(&call_canister_r.unwrap()).unwrap();
        //println!("{:?}", canister_info_sponse.recent_changes);
        assert_eq!(canister_info_sponse.total_num_changes, 4);
        assert_eq!(
            canister_info_sponse.recent_changes[0], 
            CanisterChange{
                timestamp_nanos: canister_info_sponse.recent_changes[0].timestamp_nanos,
                canister_version: 0,
                origin: CanisterChangeOrigin::FromCanister(FromCanisterRecord{canister_id: cts, canister_version: None}),
                details: CanisterChangeDetails::Creation(CreationRecord{controllers: vec![cts]}),    
            },
        ); 
        assert_eq!(
            canister_info_sponse.recent_changes[1], 
            CanisterChange{
                timestamp_nanos: canister_info_sponse.recent_changes[1].timestamp_nanos,
                canister_version: 1,
                origin: CanisterChangeOrigin::FromCanister(FromCanisterRecord{canister_id: cts, canister_version: None}),
                details: CanisterChangeDetails::CodeDeployment(CodeDeploymentRecord{mode: CanisterInstallMode::Install, module_hash: cb_module_hash.to_vec()}),    
            },
        ); 
        assert_eq!(
            canister_info_sponse.recent_changes[2], 
            CanisterChange{
                timestamp_nanos: canister_info_sponse.recent_changes[2].timestamp_nanos,
                canister_version: 2,
                origin: CanisterChangeOrigin::FromCanister(FromCanisterRecord{canister_id: cts, canister_version: None}),
                details: CanisterChangeDetails::ControllersChange(ControllersChangeRecord{controllers: {let mut v=vec![cts,cbsms[0],cb];v.sort();v}}),    
            },
        ); 
        assert_eq!(
            canister_info_sponse.recent_changes[3], 
            CanisterChange{
                timestamp_nanos: canister_info_sponse.recent_changes[3].timestamp_nanos,
                canister_version: 3,
                origin: CanisterChangeOrigin::FromCanister(FromCanisterRecord{canister_id: cbsms[0], canister_version: None}),
                details: CanisterChangeDetails::CodeDeployment(CodeDeploymentRecord{mode: CanisterInstallMode::Upgrade, module_hash: cb_module_hash.to_vec()}),    
            },
        );          
    }
}


#[test]
fn test_upgrade_cbsms() {
    let (pic, cts, _cm_main) = cts_setup();
    let mut users_and_cbs: Vec<(Principal, Principal)> = Vec::new();
    for i in 0..2 {
        let user = Principal::from_slice(&(i+5000 as u64).to_be_bytes());
        let cb = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
        users_and_cbs.push((user, cb));    
    }
    for (user, cb) in users_and_cbs.iter() {
        println!("user: {}, cb: {}", user, cb);    
    }
    //println!("1234 p: {}", Principal::from_slice(&[1,2,3,4]));
    let cbsms = controller_view_cbsms(&pic, cts);
    let cbsm_module: Vec<u8> = std::fs::read("../../target/wasm32-unknown-unknown/debug/cbs_map.wasm").unwrap();
    let cbsm_module_hash = sha256(&cbsm_module);
    
    let (uos,): (Vec<(Principal, UpgradeOutcome)>,) = call_candid_as(
        &pic,
        cts,
        RawEffectivePrincipal::None,
        CTS_CONTROLLER,
        "controller_upgrade_cbsms",
        (ControllerUpgradeCSQuest{
            new_canister_code: Some(CanisterCode::new(cbsm_module)),
            specific_cs: Some(cbsms.iter().copied().collect()),
            post_upgrade_quest: candid::encode_args(()).unwrap(),
        },)
    ).unwrap();
    println!("uos.len(): {}", uos.len());
    //println!("uos: {:?}", uos);
    for (cbsm, uo) in uos.into_iter() {
        println!("cbsm: {}", cbsm);
        assert_eq!(
            uo,
            UpgradeOutcome{
                stop_canister_result: Some(Ok(())),
                install_code_result: Some(Ok(())),    
                start_canister_result: Some(Ok(())),
            }
        );
        
        let (call_canister_r,): (Result<Vec<u8>, CallError>,) = call_candid_as(
            &pic,
            cts, // only canisters can call management-method: canister_info.
            RawEffectivePrincipal::None,
            CTS_CONTROLLER,
            "controller_call_canister",
            (ControllerCallCanisterQuest {
                callee: MANAGEMENT_CANISTER_ID,
                method_name: "canister_info".to_string(),
                arg_raw: candid::encode_one(CanisterInfoRequest{ canister_id: cbsm, num_requested_changes: Some(3) }).unwrap(),
                cycles: 0
            },)
        ).unwrap();
        let canister_info_sponse: CanisterInfoResponse = candid::decode_one(&call_canister_r.unwrap()).unwrap();
        println!("{:?}", canister_info_sponse.recent_changes);
        assert_eq!(canister_info_sponse.total_num_changes, 3);
        assert_eq!(
            canister_info_sponse.recent_changes[0], 
            CanisterChange{
                timestamp_nanos: canister_info_sponse.recent_changes[0].timestamp_nanos,
                canister_version: 0,
                origin: CanisterChangeOrigin::FromCanister(FromCanisterRecord{canister_id: cts, canister_version: None}),
                details: CanisterChangeDetails::Creation(CreationRecord{controllers: vec![cts]}),    
            },
        ); 
        assert_eq!(
            canister_info_sponse.recent_changes[1], 
            CanisterChange{
                timestamp_nanos: canister_info_sponse.recent_changes[1].timestamp_nanos,
                canister_version: 1,
                origin: CanisterChangeOrigin::FromCanister(FromCanisterRecord{canister_id: cts, canister_version: None}),
                details: CanisterChangeDetails::CodeDeployment(CodeDeploymentRecord{mode: CanisterInstallMode::Install, module_hash: cbsm_module_hash.to_vec()}),    
            },
        ); 
        assert_eq!(
            canister_info_sponse.recent_changes[2], 
            CanisterChange{
                timestamp_nanos: canister_info_sponse.recent_changes[2].timestamp_nanos,
                canister_version: 6,
                origin: CanisterChangeOrigin::FromCanister(FromCanisterRecord{canister_id: cts, canister_version: None}),
                details: CanisterChangeDetails::CodeDeployment(CodeDeploymentRecord{mode: CanisterInstallMode::Upgrade, module_hash: cbsm_module_hash.to_vec()}),    
            },
        ); 
    }
}


#[test]
fn test_lengthen_lifetime_icp_payment() {
    let (pic, cts, _cm_main) = cts_setup();
    let mut users_and_cbs: Vec<(Principal, Principal)> = Vec::new();
    for i in 0..3 {
        let user = Principal::from_slice(&(i+50000 as u64).to_be_bytes());
        let cb = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
        users_and_cbs.push((user, cb));
    }
    let cb_module: Vec<u8> = std::fs::read("../../target/wasm32-unknown-unknown/debug/cycles_bank.wasm").unwrap();
    let cb_module_hash: [u8; 32] = sha256(&cb_module);
    for (i, (user, cb)) in users_and_cbs.into_iter().enumerate() {
        let lengthen_years = (i as u128) + 1;
        let (metrics_before,): (UserCBMetrics,) = query_candid_as(&pic,cb,user,"metrics",()).unwrap();
        assert_eq!(
            metrics_before.lifetime_termination_timestamp_seconds,
            pic_get_time_nanos(&pic) / NANOS_IN_A_SECOND + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS
        );
        let canister_cycles_balance_before = pic.cycle_balance(cb);
        assert_ge!(
            canister_cycles_balance_before,
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES - 5_000_000_000
        );
        mint_icp(
            &pic, 
            Account{owner: cts, subaccount: Some(principal_token_subaccount(&user))}, 
            cycles_transform_tokens(MEMBERSHIP_COST_CYCLES*lengthen_years, CMC_RATE) + LEDGER_TRANSFER_FEE*2
        );
        let (r,) = call_candid_as::<_, (Result<LengthenMembershipSuccess, LengthenMembershipError>,)>(
            &pic, cts, RawEffectivePrincipal::None, user, "lengthen_membership", (LengthenMembershipQuest{ lengthen_years },)
        ).unwrap();
        let new_lifetime_termination_timestamp_seconds = r.unwrap().lifetime_termination_timestamp_seconds;
        assert_eq!(
            metrics_before.lifetime_termination_timestamp_seconds + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS*lengthen_years,
            new_lifetime_termination_timestamp_seconds, 
        );
        let (metrics_after,): (UserCBMetrics,) = query_candid_as(&pic,cb,user,"metrics",()).unwrap();
        assert_eq!(
            metrics_after.lifetime_termination_timestamp_seconds,
            new_lifetime_termination_timestamp_seconds
        );
        let canister_cycles_balance_after = pic.cycle_balance(cb);
        assert_ge!(
            canister_cycles_balance_after,
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES - 5_000_000_000 
            + cycles_transform_tokens(MEMBERSHIP_COST_CYCLES*lengthen_years, CMC_RATE) / 2 - 5_000_000_000
        );
        let (cts_find_user_sponse,): (Option<CBSMUserData>,) = query_candid_as(
            &pic, controller_view_cbsms(&pic, cts)[0], cts, "find_user", (user,)
        ).unwrap();
        let cbsm_user_data = cts_find_user_sponse.unwrap();
        assert_eq!(
            cbsm_user_data,
            CBSMUserData{
                cycles_bank_canister_id: cb,
                first_membership_creation_timestamp_nanos: pic_get_time_nanos(&pic), 
                cycles_bank_latest_known_module_hash: cb_module_hash,
                cycles_bank_lifetime_termination_timestamp_seconds: new_lifetime_termination_timestamp_seconds,
                membership_termination_cb_uninstall_data: None
            }
        );
        // check cts main and user-subaccount icp-balances
    }
}


#[test]
fn test_lengthen_lifetime_cycles_payment() {
    let (pic, cts, _cm_main) = cts_setup();
    let mut users_and_cbs: Vec<(Principal, Principal)> = Vec::new();
    for i in 0..3 {
        let user = Principal::from_slice(&(i+50000 as u64).to_be_bytes());
        let cb = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
        users_and_cbs.push((user, cb));
            
    }
    let cb_module: Vec<u8> = std::fs::read("../../target/wasm32-unknown-unknown/debug/cycles_bank.wasm").unwrap();
    let cb_module_hash: [u8; 32] = sha256(&cb_module);
    for (i, (user, cb)) in users_and_cbs.into_iter().enumerate() {
        let lengthen_years = (i as u128) + 1;
        let (metrics_before,): (UserCBMetrics,) = query_candid_as(&pic,cb,user,"metrics",()).unwrap();
        assert_eq!(
            metrics_before.lifetime_termination_timestamp_seconds,
            pic_get_time_nanos(&pic) / NANOS_IN_A_SECOND + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS
        );
        assert_ge!(
            pic.cycle_balance(cb),
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES - 5_000_000_000
        );
        // transfer some cycles onto the cycles-bank
        let (call_canister_r,): (Result<Vec<u8>, CallError>,) = call_candid_as(
            &pic,
            cts, 
            RawEffectivePrincipal::None,
            CTS_CONTROLLER,
            "controller_call_canister",
            (ControllerCallCanisterQuest {
                callee: cb,
                method_name: "cycles_transfer".to_string(),
                arg_raw: candid::encode_one(CyclesTransfer{memo: CyclesTransferMemo::Nat(5)}).unwrap(),
                cycles: MEMBERSHIP_COST_CYCLES*lengthen_years
            },)
        ).unwrap();
        let _: () = candid::decode_one(&call_canister_r.unwrap()).unwrap();
        assert_eq!(
            cb_cycles_balance(&pic, cb, user),
            MEMBERSHIP_COST_CYCLES*lengthen_years
        );
        assert_ge!(
            pic.cycle_balance(cb),
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES - 5_000_000_000
            + MEMBERSHIP_COST_CYCLES*lengthen_years
        );
        let (rr,) = call_candid_as::<_, (Result<Vec<u8>, CallError>,)>(
            &pic, cb, RawEffectivePrincipal::None, user, "user_lengthen_membership_cb_cycles_payment", (LengthenMembershipQuest{ lengthen_years }, lengthen_years*MEMBERSHIP_COST_CYCLES)
        ).unwrap();
        let r = candid::decode_one::<Result<LengthenMembershipSuccess, LengthenMembershipError>>(&rr.unwrap()).unwrap();
        let new_lifetime_termination_timestamp_seconds = r.unwrap().lifetime_termination_timestamp_seconds;
        assert_eq!(
            metrics_before.lifetime_termination_timestamp_seconds + NEW_CYCLES_BANK_LIFETIME_DURATION_SECONDS*lengthen_years,
            new_lifetime_termination_timestamp_seconds, 
        );
        let (metrics_after,): (UserCBMetrics,) = query_candid_as(&pic,cb,user,"metrics",()).unwrap();
        assert_eq!(
            metrics_after.lifetime_termination_timestamp_seconds,
            new_lifetime_termination_timestamp_seconds
        );
        assert_ge!(
            pic.cycle_balance(cb),
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES 
            + MEMBERSHIP_COST_CYCLES*lengthen_years / 2
            - 5_000_000_000
        );
        assert_le!(
            pic.cycle_balance(cb),
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES 
            + MEMBERSHIP_COST_CYCLES*lengthen_years / 2 
        );
        let (cts_find_user_sponse,): (Option<CBSMUserData>,) = query_candid_as(
            &pic, controller_view_cbsms(&pic, cts)[0], cts, "find_user", (user,)
        ).unwrap();
        let cbsm_user_data = cts_find_user_sponse.unwrap();
        assert_eq!(
            cbsm_user_data,
            CBSMUserData{
                cycles_bank_canister_id: cb,
                first_membership_creation_timestamp_nanos: pic_get_time_nanos(&pic), 
                cycles_bank_latest_known_module_hash: cb_module_hash,
                cycles_bank_lifetime_termination_timestamp_seconds: new_lifetime_termination_timestamp_seconds,
                membership_termination_cb_uninstall_data: None
            }
        );
        assert_eq!(
            cb_cycles_balance(&pic, cb, user),
            0
        );
    }
}

#[test]
fn test_burn_icp_mint_cycles() {
    
}


// --- tools ---

fn pic_get_time_nanos(pic: &PocketIc) -> u128 {
    pic.get_time().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
}

fn mint_icp(pic: &PocketIc, to: Account, amount: u128) {
    let (mint_icp_r,): (Result<Nat, TransferError>,) = call_candid_as(
        pic,
        ICP_LEDGER,
        RawEffectivePrincipal::None,
        ICP_MINTER,            
        "icrc1_transfer",
        (TransferArg{
            from_subaccount: None,
            to: to,
            fee: None,
            created_at_time: None,
            memo: None,
            amount: amount.into(),
        },)
    ).unwrap();
    mint_icp_r.unwrap();
}


fn mint_icp_and_purchase_cycles_bank(pic: &PocketIc, user: Principal, cts: Principal) -> Principal {
    mint_icp(
        &pic,
        Account{ owner: cts, subaccount: Some(principal_token_subaccount(&user)) },
        MEMBERSHIP_COST_CYCLES / CMC_RATE + LEDGER_TRANSFER_FEE*2,
    );
    let (purchase_cb_result,): (Result<PurchaseCyclesBankSuccess, PurchaseCyclesBankError>,) = call_candid_as(
        &pic,
        cts,
        RawEffectivePrincipal::None,
        user,
        "purchase_cycles_bank",
        (PurchaseCyclesBankQuest{},)
    ).unwrap();
    let cb = purchase_cb_result.unwrap().cycles_bank_canister_id;   
    cb
}    

fn cb_cycles_balance(pic: &PocketIc, cb: Principal, user: Principal) -> Cycles {
    let (cb_cycles_balance,): (Cycles,) = call_candid_as(&pic, cb, RawEffectivePrincipal::None, user, "cycles_balance", ()).unwrap(); 
    cb_cycles_balance    
}

fn icrc1_balance(pic: &PocketIc, ledger: Principal, countid: &Account) -> u128 {
    call_candid(
        pic,
        ledger,
        RawEffectivePrincipal::None,
        "icrc1_balance_of",
        (countid,),
    ).map(|t: (u128,)| t.0).unwrap()
}

fn local_put_ic_root_key(pic: &PocketIc, cb: Principal) {
    let _: () = call_candid(
        &pic,
        cb,
        RawEffectivePrincipal::None,
        "local_put_ic_root_key",
        (&pic.root_key().unwrap()[37..],)
    ).unwrap();
}

fn controller_view_cbsms(pic: &PocketIc, cts: Principal) -> Vec<Principal> {
    query_candid_as::<(), (Vec<Principal>,)>(pic, cts, CTS_CONTROLLER, "controller_view_cbsms", ()).unwrap().0
}


fn cts_setup() -> (PocketIc, Principal/*CTS*/, Principal/*CM_MAIN*/) {
    let pic = PocketIcBuilder::new()
        .with_nns_subnet()
        .with_fiduciary_subnet()
        .build();
    let _nns_subnet = pic.topology().get_nns().unwrap();
    let fid_subnet = pic.topology().get_fiduciary().unwrap();
    
    let icp_minter = ICP_MINTER;
    let icp_ledger_wasm = std::fs::read("ledger-canister-o-98eb213581b239c3829eee7076bea74acad9937b.wasm.gz").unwrap();
    let icp_ledger = pic.create_canister_with_id(None, None, ICP_LEDGER).unwrap();
    pic.add_cycles(icp_ledger, 1_000 * TRILLION);    
    pic.install_canister(
        icp_ledger, 
        icp_ledger_wasm, 
        candid::encode_one(
            icp_ledger::LedgerCanisterPayload::Init(
                icp_ledger::InitArgs{
                    minting_account: icp_ledger::AccountIdentifier::from(Account{owner: icp_minter, subaccount: None}),
                    icrc1_minting_account: Some(Account{owner: icp_minter, subaccount: None}),
                    initial_values: HashMap::new(),
                    send_whitelist: HashSet::new(),
                    transfer_fee: Some(icp_ledger::Tokens::from_e8s(LEDGER_TRANSFER_FEE as u64)),
                    token_symbol: Some("ICP".to_string()),
                    token_name: Some("Internet-Computer".to_string()),
                    feature_flags: Some(icp_ledger::FeatureFlags{ icrc2: true }),
                    max_message_size_bytes: None,
                    transaction_window: None, //Option<Duration>,
                    archive_options: None, //,Option<ArchiveOptions>,
                    maximum_number_of_accounts: None, //Option<usize>,
                    accounts_overflow_trim_quantity: None //Option<usize>,
                }   
            )
        ).unwrap(), 
        None
    );
     
    let nns_governance = NNS_GOVERNANCE;
    let cmc_wasm = std::fs::read("cmc-o-14e0b0adf6632a6225cb1b0a22d4bafce75eb81e.wasm.gz").unwrap();
    let cmc = pic.create_canister_with_id(None, None, CMC).unwrap();
    pic.add_cycles(cmc, 1_000 * TRILLION);    
    pic.install_canister(
        cmc, 
        cmc_wasm, 
        candid::encode_one(
            {
                #[derive(CandidType, Deserialize)]
                struct Ia {
                    ledger_canister_id: Option<Principal>,
                    governance_canister_id: Option<Principal>,
                    minting_account_id: Option<icp_ledger::AccountIdentifier>,
                    last_purged_notification: Option<u64>,
                }
                Ia{
                    ledger_canister_id: Some(icp_ledger),
                    governance_canister_id: Some(nns_governance),
                    minting_account_id: Some(icp_ledger::AccountIdentifier::from(Account{owner: icp_minter, subaccount: None})),   
                    last_purged_notification: Some(0),
                }
            }
        ).unwrap(), 
        None
    );
       
    let cmc_rate: u128 = CMC_RATE;
    let (r,): (Result<(), String>,) = call_candid_as(
        &pic,
        cmc,
        RawEffectivePrincipal::None,
        nns_governance,
        "set_icp_xdr_conversion_rate",
        (ic_nns_common::types::UpdateIcpXdrConversionRatePayload {
            data_source: "".to_string(),
            timestamp_seconds: u64::MAX, //pic.get_time().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 5*60,
            xdr_permyriad_per_icp: cmc_rate as u64,
            reason: None, //Option<UpdateIcpXdrConversionRatePayloadReason>,,)
        },)
    ).unwrap();
    r.unwrap();
    /*
    use cycles_minting_canister as cmc_lib;
    let r/*,):(cmc_lib::UpdateSubnetTypeResult,)*/ = pic.update_call(
        cmc,
        nns_governance,
        "update_subnet_type",
        encode_one(cmc_lib::UpdateSubnetTypeArgs::Add("fiduciary".to_string())).unwrap()
    ).unwrap();
    //r.unwrap();
    println!("{:?}", r);
    
    let r/*,):(cmc_lib::ChangeSubnetTypeAssignmentResult,)*/ = pic.update_call(
        cmc,
        nns_governance,
        "change_subnet_type_assignment",
        encode_one(cmc_lib::ChangeSubnetTypeAssignmentArgs::Add(cmc_lib::SubnetListWithType{ 
            subnets: vec![ic_base_types::SubnetId::new(fid_subnet.as_slice().try_into().unwrap())],
            subnet_type: "fiduciary".to_string()
        })).unwrap()
    ).unwrap();
    //r.unwrap();
    */
    
    let cts_controller = CTS_CONTROLLER;
    let cm_main: Principal = pic.create_canister_on_subnet(Some(cts_controller), None, fid_subnet);
    let cts_wasm: Vec<u8> = std::fs::read("../../target/wasm32-unknown-unknown/debug/cts.wasm").unwrap();
    let cts: Principal = pic.create_canister_on_subnet(Some(cts_controller), None, fid_subnet);
    println!("cts: {cts}");    
    pic.add_cycles(cts, 1_000 * TRILLION);
    pic.install_canister(
        cts, 
        cts_wasm, 
        candid::encode_one(
            CTSInit {
                cycles_market_main: cm_main,
            }
        ).unwrap(), 
        Some(cts_controller),
    );
    
    let _: () = call_candid_as(
        &pic,
        cts,
        RawEffectivePrincipal::None,
        cts_controller,
        "controller_put_cycles_bank_canister_code",
        (CanisterCode::new(std::fs::read("../../target/wasm32-unknown-unknown/debug/cycles_bank.wasm").unwrap()),)
    ).unwrap();
    let _: () = call_candid_as(
        &pic,
        cts,
        RawEffectivePrincipal::None,
        cts_controller,
        "controller_put_umc_code",
        (CanisterCode::new(std::fs::read("../../target/wasm32-unknown-unknown/debug/cbs_map.wasm").unwrap()),)
    ).unwrap();
    
    (pic, cts, cm_main)
}
