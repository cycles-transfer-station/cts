
use pocket_ic::{*, common::rest::RawEffectivePrincipal};
use candid::{Nat, Principal, CandidType, Deserialize};
use std::collections::{HashSet, HashMap};
use cts_lib::{
    consts::{TRILLION, MANAGEMENT_CANISTER_ID},
    tools::{principal_token_subaccount, cycles_transform_tokens, tokens_transform_cycles},
    types::{
        CanisterCode, CallError, cycles_bank::{self as cb, UserCBMetrics}, 
        cycles_market::{cm_main::*, tc as cm_tc} ,
        cts::*,
        CallCanisterQuest,
    },
    management_canister::{
        *,
        ManagementCanisterCanisterStatusRecord,
        CanisterIdRecord
    }
};
use icrc_ledger_types::icrc1::{account::Account, transfer::{TransferArg, TransferError, BlockIndex}};
use crate::*;
use ic_cdk::api::management_canister::main::{CanisterInfoRequest,CanisterInfoResponse,CanisterChange,CanisterChangeOrigin,CanisterChangeDetails,CanisterInstallMode,CodeDeploymentRecord,FromCanisterRecord,ControllersChangeRecord,CreationRecord};
use more_asserts::*;



#[test]
fn test_mint_cycles() {
    let (pic, bank) = set_up();
    
    let bank_cycles_balance_before = pic.cycle_balance(bank);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), 0);
    
    let burn_icp: u128 = 500000000; 
    let mint_cycles_quest = MintCyclesQuest{ 
        burn_icp,
        burn_icp_transfer_fee: LEDGER_TRANSFER_FEE, 
        for_account: Account{owner: user, subaccount: None},
    };
    
    let mint_cycles_result = call_candid_as::<_, (MintCyclesResult,)>(&pic, bank, RawEffectivePrincipal::None, user, "mint_cycles", (mint_cycles_quest.clone(),)).unwrap().0;
    mint_cycles_result.unwrap_err();            
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), 0);
    
    mint_icp(&pic, &Account{owner: bank, subaccount: Some(principal_token_subaccount(&user))}, burn_icp + LEDGER_TRANSFER_FEE);
    
    let mint_cycles_result = call_candid_as::<_, (MintCyclesResult,)>(&pic, bank, RawEffectivePrincipal::None, user, "mint_cycles", (mint_cycles_quest,)).unwrap().0;
    let mint_cycles = mint_cycles_result.unwrap().mint_cycles;
    
    assert_eq!(mint_cycles, tokens_transform_cycles(burn_icp, CMC_RATE));
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), mint_cycles);
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before + mint_cycles - 100_000_000);
    assert_eq!(icrc1_balance(&pic, ICP_LEDGER, &Account{owner: bank, subaccount: Some(principal_token_subaccount(&user))}), 0);
}

#[test]
fn test_mint_for_subaccount() {
    let (pic, bank) = set_up();
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let subaccount = [5u8; 32];
    let mint_for_countid = Account{owner: user, subaccount: Some(subaccount)};
    assert_eq!(icrc1_balance(&pic, bank, &mint_for_countid), 0);
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &mint_for_countid, burn_icp);    
    assert_eq!(icrc1_balance(&pic, bank, &mint_for_countid), tokens_transform_cycles(burn_icp, CMC_RATE));
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), 0);
}

#[test]
fn test_transfer() {
    let (pic, bank) = set_up();
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let user2 = Principal::self_authenticating(&(900 as u64).to_be_bytes());
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE));
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user2, subaccount: None}), 0);
    let transfer_cycles_mount = 5*TRILLION;
    icrc1_transfer(&pic, bank, user, TransferArg{
        from_subaccount: None,
        to: Account{owner: user2, subaccount: None},
        fee: Some(BANK_TRANSFER_FEE.into()),
        created_at_time: None,
        memo: None,
        amount: transfer_cycles_mount.into(),
    }).unwrap();
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE) - transfer_cycles_mount - BANK_TRANSFER_FEE);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user2, subaccount: None}), transfer_cycles_mount);
}

#[test]
fn test_transfer_fails_when_wrong_fee_is_set() {
    let (pic, bank) = set_up();
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let user2 = Principal::self_authenticating(&(900 as u64).to_be_bytes());
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    let transfer_cycles_mount = 5*TRILLION;
    let transfer_result = icrc1_transfer(&pic, bank, user, TransferArg{
        from_subaccount: None,
        to: Account{owner: user2, subaccount: None},
        fee: Some((BANK_TRANSFER_FEE - 1).into()),
        created_at_time: None,
        memo: None,
        amount: transfer_cycles_mount.into(),
    });
    assert_eq!(transfer_result, Err(TransferError::BadFee{expected_fee: BANK_TRANSFER_FEE.into()}));
    transfer_result.unwrap_err();
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE));
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user2, subaccount: None}), 0);    
}

#[test]
fn test_transfer_fails_when_insufficient_funds() {
    let (pic, bank) = set_up();
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let user2 = Principal::self_authenticating(&(900 as u64).to_be_bytes());
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    let transfer_cycles_mount = tokens_transform_cycles(burn_icp, CMC_RATE) + 1;
    let transfer_result = icrc1_transfer(&pic, bank, user, TransferArg{
        from_subaccount: None,
        to: Account{owner: user2, subaccount: None},
        fee: Some((BANK_TRANSFER_FEE).into()),
        created_at_time: None,
        memo: None,
        amount: transfer_cycles_mount.into(),
    });
    assert_eq!(transfer_result, Err(TransferError::InsufficientFunds{balance: tokens_transform_cycles(burn_icp, CMC_RATE).into()}));    
    transfer_result.unwrap_err();
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE));
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user2, subaccount: None}), 0);    
}

#[test]
fn test_cycles_in() {
    let (pic, bank) = set_up();    
    let canister_caller = set_up_canister_caller(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let cycles = 444*TRILLION;
    let for_account = Account{owner: user, subaccount: Some([5u8; 32])};
    let bank_cycles_balance_before = pic.cycle_balance(bank);    
    for i in 0..2 {
        let r = call_candid::<_, (Result<Vec<u8>, CallError>,)>(&pic, canister_caller, RawEffectivePrincipal::None, "call_canister", (CallCanisterQuest{
            callee: bank,
            method_name: "cycles_in".to_string(),
            arg_raw: candid::encode_one(CyclesInQuest{
                cycles,
                fee: Some(BANK_TRANSFER_FEE),
                for_account,
            }).unwrap(),
            cycles: if i == 0 { 
                cycles + BANK_TRANSFER_FEE - 1// wrong amount of cycles in the call
            } else {
                cycles + BANK_TRANSFER_FEE  
            }
        },)).unwrap().0;
        let cycles_in_result = candid::decode_one::<Result<BlockId, CyclesInError>>(&r.unwrap()).unwrap();
        if i == 0 { 
            let cycles_in_error = cycles_in_result.unwrap_err();
            assert_eq!(cycles_in_error, CyclesInError::MsgCyclesTooLow);
            assert_eq!(icrc1_balance(&pic, bank, &for_account), 0);
        } else {
            let block = cycles_in_result.unwrap();
            assert_eq!(block, 0);
            assert_eq!(icrc1_balance(&pic, bank, &for_account), cycles);
        }
    }
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before + cycles - 100_000_000);    
}

#[test]
fn test_cycles_out() {
    let (pic, bank) = set_up();    
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let subaccount = [7u8; 32];
    let receiving_canister = pic.create_canister();
    let receiving_canister_cycles_balance_before = pic.cycle_balance(receiving_canister);
    let bank_cycles_balance_before = pic.cycle_balance(bank);    
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: Some(subaccount)}, burn_icp);    
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:Some(subaccount)}), tokens_transform_cycles(burn_icp, CMC_RATE));
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before + tokens_transform_cycles(burn_icp, CMC_RATE) - 100_000_000);
    let bank_cycles_balance_after_mint = pic.cycle_balance(bank);
    let block = call_candid_as::<_, (Result<BlockId, CyclesOutError>,)>(&pic, bank, RawEffectivePrincipal::None, user, "cycles_out", (CyclesOutQuest{
        cycles: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE,
        fee: Some(BANK_TRANSFER_FEE),
        from_subaccount: Some(subaccount),
        for_canister: receiving_canister,
    },)).unwrap().0.unwrap();
    assert_eq!(block, 0);
    assert_le!(pic.cycle_balance(bank), bank_cycles_balance_after_mint - (tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE));
    assert_ge!(pic.cycle_balance(receiving_canister), receiving_canister_cycles_balance_before + (tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE) - 100_000_000);        
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:Some(subaccount)}), 0);    
}

#[test]
fn test_cycles_out_fails_when_not_enough_balance() {
    let (pic, bank) = set_up();    
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let burn_icp = 500000000;
    let receiving_canister = pic.create_canister();
    let receiving_canister_cycles_balance_before = pic.cycle_balance(receiving_canister);
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    let bank_cycles_balance_after_mint = pic.cycle_balance(bank);
    let cycles_out_error = call_candid_as::<_, (Result<BlockId, CyclesOutError>,)>(&pic, bank, RawEffectivePrincipal::None, user, "cycles_out", (CyclesOutQuest{
        cycles: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE + 1,
        fee: Some(BANK_TRANSFER_FEE),
        from_subaccount: None,
        for_canister: receiving_canister,
    },)).unwrap().0.unwrap_err();
    assert_eq!(cycles_out_error, CyclesOutError::InsufficientFunds{balance: tokens_transform_cycles(burn_icp, CMC_RATE)});
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_after_mint - 100_000_000);
    assert_ge!(pic.cycle_balance(receiving_canister), receiving_canister_cycles_balance_before);        
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:None}), tokens_transform_cycles(burn_icp, CMC_RATE));    
}

#[test]
fn test_cycles_out_fails_when_invalid_for_canister() {
    let (pic, bank) = set_up();    
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    let bank_cycles_balance_after_mint = pic.cycle_balance(bank);
    let cycles_out_error = call_candid_as::<_, (Result<BlockId, CyclesOutError>,)>(&pic, bank, RawEffectivePrincipal::None, user, "cycles_out", (CyclesOutQuest{
        cycles: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE,
        fee: Some(BANK_TRANSFER_FEE),
        from_subaccount: None,
        for_canister: Principal::management_canister(),
    },)).unwrap().0.unwrap_err();
    if let CyclesOutError::DepositCyclesCallError(_) = cycles_out_error {} else { panic!("must be CyclesOutError::DepositCyclesCallError") }
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_after_mint - 100_000_000);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:None}), tokens_transform_cycles(burn_icp, CMC_RATE));    
}






// ----- TOOLS ------

const LEDGER_TRANSFER_FEE: u128 = 10_000;
const CMC_RATE: u128 = 55555;
const ICP_MINTER: Principal = Principal::from_slice(&[1,1,1,1,1]);
const CMC: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,4,1,1]);
const NNS_GOVERNANCE: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,1,1,1]);
const ICP_LEDGER: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,2,1,1]);
const CTS_CONTROLLER: Principal = Principal::from_slice(&[0,1,2,3,4,5,6,7,8,9]);
const WASMS_DIR: &str = "../../target/wasm32-unknown-unknown/debug/";


fn icrc1_transfer(pic: &PocketIc, ledger: Principal, owner: Principal, q: TransferArg) -> Result<BlockIndex, TransferError> {
    call_candid_as::<_, (Result<BlockIndex, TransferError>,)>(pic, ledger, RawEffectivePrincipal::None, owner, "icrc1_transfer", (q,)).unwrap().0
}

fn mint_cycles(pic: &PocketIc, bank: Principal, countid: &Account, burn_icp: u128) -> Cycles {
    let mint_cycles_quest = MintCyclesQuest{ 
        burn_icp,
        burn_icp_transfer_fee: LEDGER_TRANSFER_FEE, 
        for_account: countid.clone(),
    };
    mint_icp(&pic, &Account{owner: bank, subaccount: Some(principal_token_subaccount(&countid.owner))}, burn_icp + LEDGER_TRANSFER_FEE);
    call_candid_as::<_, (MintCyclesResult,)>(&pic, bank, RawEffectivePrincipal::None, countid.owner, "mint_cycles", (mint_cycles_quest,)).unwrap().0
    .unwrap().mint_cycles
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

fn mint_icp(pic: &PocketIc, to: &Account, amount: u128) {
    let (mint_icp_r,): (Result<Nat, TransferError>,) = call_candid_as(
        pic,
        ICP_LEDGER,
        RawEffectivePrincipal::None,
        ICP_MINTER,            
        "icrc1_transfer",
        (TransferArg{
            from_subaccount: None,
            to: to.clone(),
            fee: None,
            created_at_time: None,
            memo: None,
            amount: amount.into(),
        },)
    ).unwrap();
    mint_icp_r.unwrap();
}


fn set_up() -> (PocketIc, Principal/*bank*/) {
    let pic = PocketIcBuilder::new()
        .with_nns_subnet()
        .with_fiduciary_subnet()
        .build();
    let _nns_subnet = pic.topology().get_nns().unwrap();
    let fid_subnet = pic.topology().get_fiduciary().unwrap();
    
    let icp_minter = ICP_MINTER;
    let icp_ledger_wasm = std::fs::read("../../pic_tests/ledger-canister-o-98eb213581b239c3829eee7076bea74acad9937b.wasm.gz").unwrap();
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
    let cmc_wasm = std::fs::read("../../pic_tests/cmc-o-14e0b0adf6632a6225cb1b0a22d4bafce75eb81e.wasm.gz").unwrap();
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
    
    // bank
    let bank: Principal = pic.create_canister_on_subnet(Some(CTS_CONTROLLER), None, fid_subnet);
    let bank_wasm: Vec<u8> = std::fs::read(WASMS_DIR.to_owned() + "bank.wasm").unwrap();
    println!("bank: {bank}");    
    pic.add_cycles(bank, 1_000 * TRILLION);
    pic.install_canister(
        bank, 
        bank_wasm, 
        candid::encode_args(()).unwrap(),
        Some(CTS_CONTROLLER),
    );
    
    (pic, bank)
}

fn set_up_canister_caller(pic: &PocketIc) -> Principal {
    let canister_caller: Principal = pic.create_canister();
    let canister_caller_wasm: Vec<u8> = std::fs::read(WASMS_DIR.to_owned() + "canister_caller.wasm").unwrap();
    println!("canister_caller: {canister_caller}");    
    pic.add_cycles(canister_caller, 1_000_000_000 * TRILLION);
    pic.install_canister(
        canister_caller, 
        canister_caller_wasm, 
        candid::encode_args(()).unwrap(),
        None,
    );
    canister_caller
}




