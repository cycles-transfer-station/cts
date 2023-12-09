use pocket_ic::{*, common::rest::RawEffectivePrincipal};
use candid::{Nat, Principal, CandidType, Deserialize};
use std::collections::{HashSet, HashMap};
use cts_lib::{
    consts::TRILLION,
    tools::principal_token_subaccount,
    types::{CanisterCode, CallError}
};
use icrc_ledger_types::icrc1::{account::Account, transfer::{TransferArg, TransferError}};
use crate::*;

const LEDGER_TRANSFER_FEE: u128 = 10_000;
const CMC_RATE: u128 = 55555;
const ICP_MINTER: Principal = Principal::from_slice(&[1,1,1,1,1]);
const CMC: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,4,1,1]);
const NNS_GOVERNANCE: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,1,1,1]);
const ICP_LEDGER: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,2,1,1]);
const CTS_CONTROLLER: Principal = Principal::from_slice(&[0,1,2,3,4,5,6,7,8,9]);



#[test]
fn purchase_cycles_bank_test() {
    let (pic, cts, cm_main) = cts_setup();
    let mut users_and_cbs: Vec<(Principal, Principal)> = Vec::new();
    for i in 0..(CB_CACHE_SIZE * 3) {
        let user = Principal::from_slice(&(i as u64).to_be_bytes());
        // test purchase_cycles_bank
        let cb: Principal = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
        println!("cb: {}", cb);
        users_and_cbs.push((user, cb));
        assert!(
            pic.cycle_balance(cb)
            >=
            NEW_CYCLES_BANK_CREATION_CYCLES - NETWORK_CANISTER_CREATION_FEE_CYCLES - 5_000_000_000/*for the install and managment canister calls*/,
        );
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner:cts,subaccount: None}),
            ((MEMBERSHIP_COST_CYCLES / CMC_RATE) - (NEW_CYCLES_BANK_CREATION_CYCLES / CMC_RATE)) * (i+1) as u128
        );
        assert_eq!(cb_cycles_balance(&pic, cb, user), 0);        
        pic.advance_time(core::time::Duration::from_nanos(MINIMUM_CB_AUTH_DURATION_NANOS * 2));
    }
    
    for (user,cb) in users_and_cbs.iter() {
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
        
        // check metrics
    }
}

#[test]
fn test_cb_transfer_icrc1() {
    let (pic, cts, cm_main) = cts_setup();
    let user = Principal::from_slice(&(0123456789 as u64).to_be_bytes());
    let cb: Principal = mint_icp_and_purchase_cycles_bank(&pic, user, cts);
    
    let cb_canister_cycles_balance = pic.cycle_balance(cb);
    println!("cb_canister_cycles_balance: {cb_canister_cycles_balance}");
    mint_icp(&pic, Account{owner: cb,subaccount: None}, 500_000_000_000);
        
    for ii in 0..100 {
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
    let cts_wasm: Vec<u8> = std::fs::read("../../target/wasm32-unknown-unknown/release/cts.wasm").unwrap();
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
        (CanisterCode::new(std::fs::read("../../target/wasm32-unknown-unknown/release/cycles_bank.wasm").unwrap()),)
    ).unwrap();
    let _: () = call_candid_as(
        &pic,
        cts,
        RawEffectivePrincipal::None,
        cts_controller,
        "controller_put_umc_code",
        (CanisterCode::new(std::fs::read("../../target/wasm32-unknown-unknown/release/cbs_map.wasm").unwrap()),)
    ).unwrap();
    
    (pic, cts, cm_main)
}
