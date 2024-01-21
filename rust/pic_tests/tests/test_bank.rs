use pocket_ic::{*, common::rest::RawEffectivePrincipal};
use candid::Principal;
use cts_lib::{
    consts::{TRILLION},
    tools::{principal_token_subaccount, tokens_transform_cycles},
    types::{
        CallError,
        CallCanisterQuest,
        bank::{*, log_types::*},
    },
    icrc::BlockId,
};
use icrc_ledger_types::icrc1::{account::Account, transfer::{TransferArg, TransferError}};
use more_asserts::*;
use pic_tools::{*, bank::*};


#[test]
fn test_mint_cycles() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    
    let bank_cycles_balance_before = pic.cycle_balance(bank);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), 0);
    
    let burn_icp: u128 = 500000000; 
    let mint_cycles_quest = MintCyclesQuest{ 
        burn_icp,
        burn_icp_transfer_fee: ICP_LEDGER_TRANSFER_FEE, 
        to: Account{owner: user, subaccount: None},
        fee: None,
        memo: None,    
        created_at_time: None,
    };
    
    let mint_cycles_result = call_candid_as::<_, (MintCyclesResult,)>(&pic, bank, RawEffectivePrincipal::None, user, "mint_cycles", (mint_cycles_quest.clone(),)).unwrap().0;
    mint_cycles_result.unwrap_err();            
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), 0);
    
    mint_icp(&pic, &Account{owner: bank, subaccount: Some(principal_token_subaccount(&user))}, burn_icp + ICP_LEDGER_TRANSFER_FEE);
    
    let mint_cycles_result = call_candid_as::<_, (MintCyclesResult,)>(&pic, bank, RawEffectivePrincipal::None, user, "mint_cycles", (mint_cycles_quest,)).unwrap().0;
    let mint_cycles_mount = mint_cycles_result.unwrap().mint_cycles;
    
    assert_eq!(mint_cycles_mount, tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), mint_cycles_mount);
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before + mint_cycles_mount + BANK_TRANSFER_FEE - 100_000_000);
    assert_eq!(icrc1_balance(&pic, ICP_LEDGER, &Account{owner: bank, subaccount: Some(principal_token_subaccount(&user))}), 0);
    
    let get_logs_backwards_sponse = get_logs_backwards(&pic, bank, &Account{owner: user, subaccount: None}, None::<u128>);
    let user_logs = get_logs_backwards_sponse.logs;
    assert_eq!(user_logs.len(), 1);
    assert_eq!(user_logs[0].0, 0);
    assert_eq!(
        user_logs[0].1,
        Log{
            ts: pic_get_time_nanos(&pic) as u64,
            fee: Some(BANK_TRANSFER_FEE),
            tx: LogTX{
                op: Operation::Mint{ to: (user, None), kind: MintKind::CMC{ caller: user, icp_block_height: 1 } },
                fee: None,
                amt: mint_cycles_mount,
                memo: None,
                ts: None,
            }
        }
    );
}

#[test]
fn test_mint_for_subaccount() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let subaccount = [5u8; 32];
    let mint_for_countid = Account{owner: user, subaccount: Some(subaccount)};
    assert_eq!(icrc1_balance(&pic, bank, &mint_for_countid), 0);
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &mint_for_countid, burn_icp);    
    assert_eq!(icrc1_balance(&pic, bank, &mint_for_countid), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount: None}), 0);
    assert_eq!(
        get_logs_backwards(&pic, bank, &Account{owner: user, subaccount: Some(subaccount)}, None).logs[0].1,
        Log{
            ts: pic_get_time_nanos(&pic) as u64,
            fee: Some(BANK_TRANSFER_FEE),
            tx: LogTX{
                op: Operation::Mint{ to: (user, Some(subaccount)), kind: MintKind::CMC{ caller: user, icp_block_height: 1 } },
                fee: None,
                amt: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE,
                memo: None,
                ts: None,
            }
        }
    );
}

#[test]
fn test_transfer() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let user2 = Principal::self_authenticating(&(900 as u64).to_be_bytes());
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);
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
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE - transfer_cycles_mount - BANK_TRANSFER_FEE);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user2, subaccount: None}), transfer_cycles_mount);
    for u in [user, user2].into_iter() {
        let u_logs = get_logs_backwards(&pic, bank, &Account{owner: u, subaccount: None}, None).logs;
        let (b, log) = &u_logs[u_logs.len() - 1];
        assert_eq!(b, &1);
        assert_eq!(
            log,
            &Log{
                ts: pic_get_time_nanos(&pic) as u64,
                fee: None,
                tx: LogTX{
                    op: Operation::Xfer{ from: (user, None), to: (user2, None) },
                    fee: Some(BANK_TRANSFER_FEE),
                    amt: transfer_cycles_mount,
                    memo: None,
                    ts: None,
                }
            }
        );
    }
}

#[test]
fn test_transfer_fails_when_wrong_fee_is_set() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
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
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user2, subaccount: None}), 0);    
}

#[test]
fn test_transfer_fails_when_insufficient_funds() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let user2 = Principal::self_authenticating(&(900 as u64).to_be_bytes());
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    let transfer_cycles_mount = tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE*2 + 1;
    let transfer_result = icrc1_transfer(&pic, bank, user, TransferArg{
        from_subaccount: None,
        to: Account{owner: user2, subaccount: None},
        fee: Some((BANK_TRANSFER_FEE).into()),
        created_at_time: None,
        memo: None,
        amount: transfer_cycles_mount.into(),
    });
    assert_eq!(transfer_result, Err(TransferError::InsufficientFunds{balance: (tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE).into()}));    
    transfer_result.unwrap_err();
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user, subaccount: None}), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner: user2, subaccount: None}), 0);    
}

#[test]
fn test_cycles_in() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    let canister_caller = set_up_canister_caller(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let cycles = 444*TRILLION;
    let subaccount = [5u8; 32];
    let for_account = Account{owner: user, subaccount: Some(subaccount)};
    let bank_cycles_balance_before = pic.cycle_balance(bank);    
    for i in 0..2 {
        let r = call_candid::<_, (Result<Vec<u8>, CallError>,)>(&pic, canister_caller, RawEffectivePrincipal::None, "call_canister", (CallCanisterQuest{
            callee: bank,
            method_name: "cycles_in".to_string(),
            arg_raw: candid::encode_one(CyclesInQuest{
                cycles,
                fee: Some(BANK_TRANSFER_FEE),
                to: for_account,
                memo: None,
                created_at_time: None,
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
            assert_eq!(
                get_logs_backwards(&pic, bank, &for_account, None).logs[0].1,
                Log{
                    ts: pic_get_time_nanos(&pic) as u64,
                    fee: None,
                    tx: LogTX{
                        op: Operation::Mint{ to: (user, Some(subaccount)), kind: MintKind::CyclesIn{ from_canister: canister_caller } },
                        fee: Some(BANK_TRANSFER_FEE),
                        amt: cycles,
                        memo: None,
                        ts: None,
                    }
                }
            );
        }
    }
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before + cycles - 100_000_000);    
}

#[test]
fn test_cycles_out() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let subaccount = [7u8; 32];
    let receiving_canister = pic.create_canister();
    let receiving_canister_cycles_balance_before = pic.cycle_balance(receiving_canister);
    let bank_cycles_balance_before = pic.cycle_balance(bank);    
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: Some(subaccount)}, burn_icp);    
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:Some(subaccount)}), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before + tokens_transform_cycles(burn_icp, CMC_RATE) - 100_000_000);
    let bank_cycles_balance_before_cycles_out = pic.cycle_balance(bank);
    let block = call_candid_as::<_, (Result<BlockId, CyclesOutError>,)>(&pic, bank, RawEffectivePrincipal::None, user, "cycles_out", (CyclesOutQuest{
        cycles: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE*2,
        fee: Some(BANK_TRANSFER_FEE),
        from_subaccount: Some(subaccount),
        for_canister: receiving_canister,
        memo: None,
        created_at_time: None,
    },)).unwrap().0.unwrap();
    assert_eq!(block, 1);
    assert_le!(pic.cycle_balance(bank), bank_cycles_balance_before_cycles_out - (tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE*2));
    assert_ge!(pic.cycle_balance(receiving_canister), receiving_canister_cycles_balance_before + (tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE*2) - 100_000_000);        
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:Some(subaccount)}), 0);    
    assert_eq!(
        get_logs_backwards(&pic, bank, &Account{owner: user, subaccount: Some(subaccount)}, None).logs[1].1,
        Log{
            ts: pic_get_time_nanos(&pic) as u64,
            fee: None,
            tx: LogTX{
                op: Operation::Burn{ from: (user, Some(subaccount)), for_canister: receiving_canister },
                fee: Some(BANK_TRANSFER_FEE),
                amt: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE*2,
                memo: None,
                ts: None,
            }
        }
    );
}

#[test]
fn test_cycles_out_fails_when_not_enough_balance() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let burn_icp = 500000000;
    let receiving_canister = pic.create_canister();
    let receiving_canister_cycles_balance_before = pic.cycle_balance(receiving_canister);
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    let bank_cycles_balance_before_cycles_out = pic.cycle_balance(bank);
    let cycles_out_error = call_candid_as::<_, (Result<BlockId, CyclesOutError>,)>(&pic, bank, RawEffectivePrincipal::None, user, "cycles_out", (CyclesOutQuest{
        cycles: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE*2 + 1,
        fee: Some(BANK_TRANSFER_FEE),
        from_subaccount: None,
        for_canister: receiving_canister,
        memo: None,
        created_at_time: None,
    },)).unwrap().0.unwrap_err();
    assert_eq!(cycles_out_error, CyclesOutError::InsufficientFunds{balance: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE});
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before_cycles_out - 100_000_000);
    assert_ge!(pic.cycle_balance(receiving_canister), receiving_canister_cycles_balance_before);        
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:None}), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);    
}

#[test]
fn test_cycles_out_fails_when_invalid_for_canister() {
    let pic = set_up();
    let bank = set_up_bank(&pic);
    let user = Principal::self_authenticating(&(800 as u64).to_be_bytes());
    let burn_icp = 500000000;
    mint_cycles(&pic, bank, &Account{owner: user, subaccount: None}, burn_icp);    
    let bank_cycles_balance_before_cycles_out = pic.cycle_balance(bank);
    let cycles_out_error = call_candid_as::<_, (Result<BlockId, CyclesOutError>,)>(&pic, bank, RawEffectivePrincipal::None, user, "cycles_out", (CyclesOutQuest{
        cycles: tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE*2,
        fee: Some(BANK_TRANSFER_FEE),
        from_subaccount: None,
        for_canister: Principal::management_canister(),
        memo: None,
        created_at_time: None,
    },)).unwrap().0.unwrap_err();
    if let CyclesOutError::DepositCyclesCallError(_) = cycles_out_error {} else { panic!("must be CyclesOutError::DepositCyclesCallError") }
    assert_ge!(pic.cycle_balance(bank), bank_cycles_balance_before_cycles_out - 100_000_000);
    assert_eq!(icrc1_balance(&pic, bank, &Account{owner:user, subaccount:None}), tokens_transform_cycles(burn_icp, CMC_RATE) - BANK_TRANSFER_FEE);    
}









