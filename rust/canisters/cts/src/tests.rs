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

#[test]
fn t() {
    let pic = PocketIcBuilder::new()
        .with_nns_subnet()
        .with_fiduciary_subnet()
        .build();
    let _nns_subnet = pic.topology().get_nns().unwrap();
    let fid_subnet = pic.topology().get_fiduciary().unwrap();
    
    let icp_minter = Principal::from_slice(&[1,1,1,1,1]);
    let icp_ledger_wasm = std::fs::read("ledger-canister-o-98eb213581b239c3829eee7076bea74acad9937b.wasm.gz").unwrap();
    let icp_ledger = pic.create_canister_with_id(None, None, Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap()).unwrap();
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
     
    let nns_governance = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
     
    let cmc_wasm = std::fs::read("cmc-o-14e0b0adf6632a6225cb1b0a22d4bafce75eb81e.wasm.gz").unwrap();
    let cmc = pic.create_canister_with_id(None, None, Principal::from_text("rkp4c-7iaaa-aaaaa-aaaca-cai").unwrap()).unwrap();
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
       
    let cmc_rate: u128 = 55555;
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
    // ---
    
    let cts_controller = Principal::from_slice(&[0,1,2,3,4,5,6,7,8,9]);
    
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
    
    
    // --- setup complete ---
    let mut users_and_cbs: Vec<(Principal, Principal)> = Vec::new();
    for i in 0..10 {
        let user = Principal::from_slice(&(i as u64).to_be_bytes());
        let (mint_icp_r,): (Result<Nat, TransferError>,) = call_candid_as(
            &pic,
            icp_ledger,
            RawEffectivePrincipal::None,
            icp_minter,            
            "icrc1_transfer",
            (TransferArg{
                from_subaccount: None,
                to: Account{
                    owner: cts,
                    subaccount: Some(principal_token_subaccount(&user))
                },
                fee: None,
                created_at_time: None,
                memo: None,
                amount: (MEMBERSHIP_COST_CYCLES / cmc_rate + LEDGER_TRANSFER_FEE*2).into(),
            },)
        ).unwrap();
        mint_icp_r.unwrap();
        
        let (purchase_cb_result,): (Result<PurchaseCyclesBankSuccess, PurchaseCyclesBankError>,) = call_candid_as(
            &pic,
            cts,
            RawEffectivePrincipal::None,
            user,
            "purchase_cycles_bank",
            (PurchaseCyclesBankQuest{},)
        ).unwrap();
        let cb = purchase_cb_result.unwrap().cycles_bank_canister_id;
        println!("cb: {}", cb);
        users_and_cbs.push((user, cb));
    }
    
    let (user1, cb1) = users_and_cbs[0];
    let cb_canister_cycles_balance = pic.cycle_balance(cb1);
    println!("cb_canister_cycles_balance: {cb_canister_cycles_balance}");
        
        let (mint_icp_r,): (Result<Nat, TransferError>,) = call_candid_as(
            &pic,
            icp_ledger,
            RawEffectivePrincipal::None,
            icp_minter,            
            "icrc1_transfer",
            (TransferArg{
                from_subaccount: None,
                to: Account{
                    owner: cb1,
                    subaccount: None
                },
                fee: None,
                created_at_time: None,
                memo: None,
                amount: 500_000_000_000u128.into(),
            },)
        ).unwrap();
        mint_icp_r.unwrap();
        
    for ii in 0..100 {
        let (tr,): (Result<Vec<u8>, CallError>,) = call_candid_as(
            &pic,
            cb1,
            RawEffectivePrincipal::None,
            user1,
            "transfer_icrc1",
            (icp_ledger, encode_one(
                TransferArg{
                    from_subaccount: None,
                    to: Account{
                        owner: user1,
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
    
    let cb_canister_cycles_balance = pic.cycle_balance(cb1);
    println!("cb_canister_cycles_balance: {cb_canister_cycles_balance}");
    
    
    
    
    
}