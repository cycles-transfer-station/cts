use pic_tools::{*};
use cts_lib::{
    types::{
        fueler::{FUEL_TOPUP_TRIGGER_THRESHOLD, FUEL_TOPUP_TO_MINIMUM_BALANCE, RHYTHM},
    },
    tools::{
        principal_token_subaccount
    },
    consts::{
        TRILLION,
    },
};
use icrc_ledger_types::icrc1::account::Account;


#[test]
fn fueler_test_1() {
    let pic = set_up();
    let tc = set_up_tc(&pic);
    
    let canisters = [SNS_ROOT, SNS_GOVERNANCE, SNS_LEDGER, SNS_LEDGER_INDEX, SNS_SWAP, CTS, BANK, CM_MAIN, FUELER, tc];

    for canister in canisters.into_iter() {
        // pre-condition
        assert!(pic.cycle_balance(canister) < FUEL_TOPUP_TRIGGER_THRESHOLD);
    }
    
    let icp_start_balance: u128 = 10_000_00_000_000;
    
    mint_icp(&pic, &Account{owner: BANK, subaccount: Some(principal_token_subaccount(&FUELER))}, icp_start_balance);
    
    pic.advance_time(RHYTHM);
    
    for i in 0..100 {
        if i % 5 == 0 {
            pic.advance_time(std::time::Duration::from_secs(60 * 5));
        }
        pic.tick();
    }
    
    for canister in canisters.into_iter() {
        println!("testing {} balance after fuel.", canister);
        // post-condition
        assert!(pic.cycle_balance(canister) > FUEL_TOPUP_TO_MINIMUM_BALANCE - 1*TRILLION);
    }    
}

