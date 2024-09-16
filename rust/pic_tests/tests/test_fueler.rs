use pic_tools::{*, bank::{self, mint_cycles}};
use cts_lib::{
    types::{
        fueler::{FUEL_TOPUP_TRIGGER_THRESHOLD, FUEL_TOPUP_TO_MINIMUM_BALANCE, RHYTHM},
        cm::cm_main::NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES,
        bank::BANK_TRANSFER_FEE
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
    
    let canisters = [SNS_ROOT, SNS_GOVERNANCE, SNS_LEDGER, SNS_LEDGER_INDEX, SNS_SWAP, CTS, BANK, CM_MAIN, FUELER, TOP_LEVEL_UPGRADER, tc];

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


#[test]
fn fueler_test_2() {
    let pic = set_up();
    let tc = set_up_tc(&pic);

    let canisters = [SNS_ROOT, SNS_GOVERNANCE, SNS_LEDGER, SNS_LEDGER_INDEX, SNS_SWAP, CTS, BANK, CM_MAIN, FUELER, TOP_LEVEL_UPGRADER, tc];

    for canister in canisters.into_iter() {
        // pre-condition
        assert!(pic.cycle_balance(canister) < FUEL_TOPUP_TRIGGER_THRESHOLD);
    }

    let icp_start_balance: u128 = 10_000_00_000_000;
    mint_icp(&pic, &Account{owner: BANK, subaccount: Some(principal_token_subaccount(&FUELER))}, icp_start_balance);
    let cycles_start_balance: u128 = mint_cycles(&pic, &Account{owner: FUELER, subaccount: None}, (40*TRILLION) / CMC_RATE);

    pic.advance_time(RHYTHM);

    for i in 0..100 {
        if i % 5 == 0 {
            pic.advance_time(std::time::Duration::from_secs(60 * 5));
        }
        pic.tick();
    }

    for canister in canisters.iter() {
        println!("testing {} balance after fuel.", canister);
        // post-condition
        assert!(pic.cycle_balance(canister.clone()) > FUEL_TOPUP_TO_MINIMUM_BALANCE - 1*TRILLION);
    }

    let need_mint_cycles =
        (canisters.len() - 1) as u128 /* minus 1 for the tc which starts with a different mount of cycles*/
        * (FUEL_TOPUP_TO_MINIMUM_BALANCE - FUEL_TOPUP_TRIGGER_THRESHOLD) // bc these canisters start with START_WITH_FUEL = FUEL_TOPUP_TRIGGER_THRESHOLD-1 cycles balance.
        + (FUEL_TOPUP_TO_MINIMUM_BALANCE - NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES) // for the tc
        + (canisters.len() as u128 * BANK_TRANSFER_FEE) // cycles-out transfer fees
        + BANK_TRANSFER_FEE // for the fueler's mint-cycles-fee
        - cycles_start_balance;

    let need_burn_icp = need_mint_cycles / CMC_RATE + 1;

    let range_radius_cycles = 1*TRILLION; // for the cycles that get burned just by the canister existing so far.

    let fueler_mint_cycles_log = &bank::get_logs_backwards(&pic, BANK, &Account{owner: FUELER, subaccount: None}, None).logs[1].1;
    assert_eq!(
        fueler_mint_cycles_log.tx.op,
        cts_lib::types::bank::new_log_types::Operation::Mint{
            to: cts_lib::icrc::IcrcId{owner: FUELER, subaccount: None},
            kind: cts_lib::types::bank::new_log_types::MintKind::CMC{caller: FUELER, icp_block_height: 5}
        },
    );

    println!("fueler_mint_cycles_log.tx.amt: {}", fueler_mint_cycles_log.tx.amt);
    println!("need_mint_cycles: {}", need_mint_cycles);
    assert!(
        range_radius(need_mint_cycles, range_radius_cycles)
        .contains(&fueler_mint_cycles_log.tx.amt)
    );

    let calculate_icp_finish_balance = icp_start_balance - need_burn_icp - ICP_LEDGER_TRANSFER_FEE;
    assert!(
        range_radius(calculate_icp_finish_balance, range_radius_cycles / CMC_RATE)
        .contains(
            &icrc1_balance(&pic, ICP_LEDGER, &Account{owner: BANK, subaccount: Some(principal_token_subaccount(&FUELER))})
        )
    );

}




#[test]
fn fueler_test_3() {
    let pic = set_up();
    let tc = set_up_tc(&pic);

    let canisters = [SNS_ROOT, SNS_GOVERNANCE, SNS_LEDGER, SNS_LEDGER_INDEX, SNS_SWAP, CTS, BANK, CM_MAIN, FUELER, TOP_LEVEL_UPGRADER, tc];
    
    for canister in canisters.into_iter() {
        pic.add_cycles(canister, FUEL_TOPUP_TRIGGER_THRESHOLD + 2*TRILLION - (pic.cycle_balance(canister)));
        // pre-condition
        assert!(
            pic.cycle_balance(canister) > FUEL_TOPUP_TRIGGER_THRESHOLD + 1*TRILLION 
            && pic.cycle_balance(canister) <= FUEL_TOPUP_TRIGGER_THRESHOLD + 2*TRILLION
        );
    }
        
    pic.advance_time(RHYTHM);
    for i in 0..100 {
        if i % 5 == 0 {
            pic.advance_time(std::time::Duration::from_secs(60 * 5));
        }
        pic.tick();
    }

    for canister in canisters.iter() {
        // post-condition
        assert!(
            pic.cycle_balance(canister.clone()) <= FUEL_TOPUP_TRIGGER_THRESHOLD + 2*TRILLION
        );
    }
    
}
