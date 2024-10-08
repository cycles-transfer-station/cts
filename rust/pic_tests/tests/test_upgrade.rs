use pic_tools::{*, bank::*, tc::*};
use pocket_ic::{PocketIc, call_candid_as};

use std::thread;
use std::sync::mpsc;
use std::process::{Command, ExitStatus};
use std::collections::BTreeMap;
use std::time::Duration;

use candid::{Principal, CandidType};
use serde_bytes::ByteBuf;

use icrc_ledger_types::icrc1::{account::Account, transfer::TransferArg};
use cts_lib::{
    types::{
        CallCanisterQuest,
        CallError,
        CanisterCode,
        top_level_upgrader::UpgradeTopLevelCanisterQuest,
        bank::{*, log_types::*, icrc3::{icrc3_value_of_a_block_log, StartAndLength}},
        cm::{icrc45, tc::{TradeCyclesQuest, TradeTokensQuest}},
    },
    consts::TRILLION,
    icrc::IcrcId,
    tools::principal_token_subaccount,
};



#[test]
#[ignore]
fn test_upgrade_1() {
    // will create the canisters at their mainnet versions, 
    // then will put data into them
    // then will upgrade the canisters to the current working copy version
    // and check that the data is good
    
    // set up starting versions 
    let start_at_version = CanisterGitVersions::current_live_mainnet_versions();
    println!("Starting at versions: {:?}", start_at_version);
    let (start_at_top_level_modules, start_at_tcs_modules) = get_canister_modules_of_the_git_versions(start_at_version);
    
    // set inits be compatible with the mainnet versions
    let start_at_top_level_inits = {
        let default_inits = TopLevelInits::default();
        TopLevelInits{
            cts: default_inits.cts,
            bank: default_inits.bank,
            cm_main: default_inits.cm_main,
            fueler: default_inits.fueler,
            top_level_upgrader: default_inits.top_level_upgrader,
        }
    };
    
    // set up
    let pic = set_up_with_modules_and_inits(start_at_top_level_modules, start_at_top_level_inits);
    let tc = set_up_tc_with_modules(&pic, start_at_tcs_modules);
    let (_ledger1, _tc1) = set_up_new_ledger_and_tc(&pic);
    let (_ledger2, _tc2) = set_up_new_ledger_and_tc(&pic);
    let (_ledger3, _tc3) = set_up_new_ledger_and_tc(&pic);
    
    // put some data    
    let p1 = Principal::from_slice(b"abcdefghijklmnopqrstuvwxyz");
    let p2 = Principal::from_slice(b"zyxwvutsrqponmlkjihgfedcba");
    
    let p1_mint_cycles = pic_tools::bank::mint_cycles(&pic, &Account{ owner: p1, subaccount: None }, 50000000000);    
    
    icrc1_transfer(&pic, BANK, p1, TransferArg{
        to: Account{ owner: p2, subaccount: None},
        amount: (1234*TRILLION).into(),
        from_subaccount: None,
        created_at_time: None,
        memo: None,
        fee: None,
    }).unwrap();
    
    let cts_canister_cycles_balance_before_cycles_out = pic.cycle_balance(CTS);
    call_candid_as_::<_, (Result<u128/*block*/, CyclesOutError>,)>(&pic, BANK, p2, "cycles_out", (CyclesOutQuest{
        for_canister: CTS,
        cycles: 123*TRILLION,
        memo: None, 
        fee: Some(BANK_TRANSFER_FEE),
        from_subaccount: None,
    },)).unwrap().0.unwrap();
    
    let canister_caller = set_up_canister_caller(&pic);
    let canister_caller_cycles_in_r_b = call_candid_::<_, (Result<Vec<u8>, CallError>,)>(&pic, canister_caller, "call_canister", (CallCanisterQuest{
        callee: BANK,
        method_name: "cycles_in".to_string(),
        arg_raw: candid::encode_one(CyclesInQuest{
            cycles: 123*TRILLION, 
            fee: Some(BANK_TRANSFER_FEE),
            to: IcrcId{owner: canister_caller, subaccount: None},
            memo: None, 
        }).unwrap(),
        cycles: 123*TRILLION + BANK_TRANSFER_FEE,
    },)).unwrap().0.unwrap();
    candid::decode_one::<Result<u128, CyclesInError>>(&canister_caller_cycles_in_r_b).unwrap().unwrap();
    
    // cm-tc some positions
    let p2_trade_cycles = 100*TRILLION;
    icrc1_transfer(&pic, BANK, p2, TransferArg{
        to: Account{ owner: tc, subaccount: Some(principal_token_subaccount(&p2))},
        amount: (p2_trade_cycles + BANK_TRANSFER_FEE).into(),
        from_subaccount: None,
        created_at_time: None,
        memo: None,
        fee: Some(BANK_TRANSFER_FEE.into()),
    }).unwrap();
    let p2_trade_cycles_quest = TradeCyclesQuest{
        cycles: p2_trade_cycles,
        cycles_per_token_rate: 74567,
        posit_transfer_ledger_fee: Some(BANK_TRANSFER_FEE),
        return_cycles_to_subaccount: None,
        payout_tokens_to_subaccount: None,
    };
    let _p2_trade_cycles_position_id = call_trade_cycles(&pic, tc, p2, &p2_trade_cycles_quest).unwrap().position_id;
    
    let p1_mint_icp = 13_00000000;
    mint_icp(&pic, &Account{owner: tc, subaccount: Some(principal_token_subaccount(&p1))}, p1_mint_icp);
    let p1_trade_tokens_quest = TradeTokensQuest{
        tokens: p1_mint_icp - ICP_LEDGER_TRANSFER_FEE,
        cycles_per_token_rate: p2_trade_cycles_quest.cycles_per_token_rate,
        posit_transfer_ledger_fee: Some(ICP_LEDGER_TRANSFER_FEE),
        return_tokens_to_subaccount: None,
        payout_cycles_to_subaccount: None,
    };
    let _p1_trade_tokens_position_id = call_trade_tokens(&pic, tc, p1, &p1_trade_tokens_quest).unwrap().position_id;

    pic.advance_time(Duration::from_secs(500));
    pic.tick();
    
    
    
    
    // backup    
    //let canisters_memories_before_upgrades: Vec<CanisterMemoriesRawData> = get_canisters_memory_ids(&[icp_tc, tc1, tc2, tc3]).iter().map(download_canister_memories).collect();   
    
    // upgrade
    upgrade_to_current_release_versions(&pic);
    
    pic.advance_time(Duration::from_secs(500));
    pic.tick();
    
    // check that the data is still there
    
    /*let canisters_memories_after_upgrades: Vec<CanisterMemoriesRawData> = get_canisters_memory_ids(&[icp_tc, tc1, tc2, tc3]).iter().map(download_canister_memories).collect();    
    assert_eq!(
        canisters_memories_before_upgrades,
        canisters_memories_after_upgrades,
    );*/
    
    assert_eq!(
        p1_mint_cycles - (1234*TRILLION) - BANK_TRANSFER_FEE
        + {
            let cycles_trade = (p1_mint_icp - ICP_LEDGER_TRANSFER_FEE)*p2_trade_cycles_quest.cycles_per_token_rate;
            let cycles_fee = cts_lib::types::cm::tc::trade_fee::calculate_trade_fee(0, cycles_trade);
            cycles_trade - cycles_fee - BANK_TRANSFER_FEE
        },
        icrc1_balance(&pic, BANK, &Account{owner: p1, subaccount: None}),
    );
    assert_eq!(
        (1234*TRILLION) - (123*TRILLION) - BANK_TRANSFER_FEE 
        - (p2_trade_cycles_quest.cycles + (BANK_TRANSFER_FEE*2)),
        icrc1_balance(&pic, BANK, &Account{owner: p2, subaccount: None}),
    );
    assert!(pic.cycle_balance(CTS) > cts_canister_cycles_balance_before_cycles_out + (123*TRILLION) - 1*TRILLION);
    assert_eq!(
        (123*TRILLION),
        icrc1_balance(&pic, BANK, &Account{owner: canister_caller, subaccount: None}),
    );
    
    let p1_logs = get_logs_backwards(&pic, BANK, &Account{owner: p1, subaccount: None}, None::<u128>).logs;  
    //assert_eq!(p1_logs.len(), 2);
    let log_0 = (
        0,
        Log{
            phash: None,
            ts: p1_logs[0].1.ts,
            fee: Some(BANK_TRANSFER_FEE),
            tx: LogTX{
                op: Operation::Mint{ to: Account{owner: p1, subaccount: None}.into(), kind: MintKind::CMC{ caller: p1, icp_block_height: 2 } },  
                fee: None,
                amt: p1_mint_cycles,
                memo: None,
                ts: None,
            }
        }
    );
    assert_eq!(
        p1_logs[0],
        log_0
    );
    let log_1 = (
        1,
        Log{
            phash: Some(icrc3_value_of_a_block_log(&p1_logs[0].1).hash().into()),
            ts: p1_logs[1].1.ts,
            fee: Some(BANK_TRANSFER_FEE),
            tx: LogTX{
                op: Operation::Xfer{ 
                    from: Account{owner: p1, subaccount: None}.into(),  
                    to: Account{owner: p2, subaccount: None}.into(), 
                },  
                fee: None,
                amt: 1234*TRILLION,
                memo: None,
                ts: None,
            }
        }
    ); 
    assert_eq!(
        p1_logs[1],
        log_1,
    );
    
    let p2_logs = get_logs_backwards(&pic, BANK, &Account{owner: p2, subaccount: None}, None::<u128>).logs;  
    //assert_eq!(p2_logs.len(), 2);
    assert_eq!(
        p2_logs[0],
        log_1,
    );
    let log_2 = (
        2,
        Log{
            phash: Some(icrc3_value_of_a_block_log(&log_1.1).hash().into()),
            ts: p2_logs[1].1.ts,
            fee: None,
            tx: LogTX{
                op: Operation::Burn{ 
                    from: Account{owner: p2, subaccount: None}.into(),  
                    for_canister: CTS, 
                },  
                fee: Some(BANK_TRANSFER_FEE),
                amt: 123*TRILLION + BANK_TRANSFER_FEE, // in a cycles_out the fee is clude in the burn-mount since icrc1 doesn't have fees for burn.
                memo: None,
                ts: None,
            }
        }
    ); 
    assert_eq!(
        p2_logs[1],
        log_2,
    );
    
    let canister_caller_logs = get_logs_backwards(&pic, BANK, &Account{owner: canister_caller, subaccount: None}, None::<u128>).logs;  
    assert_eq!(canister_caller_logs.len(), 1);
    let log_3 = (
        3,
        Log{
            phash: Some(icrc3_value_of_a_block_log(&log_2.1).hash().into()),
            ts: canister_caller_logs[0].1.ts,
            fee: None,
            tx: LogTX{
                op: Operation::Mint{ to: Account{owner: canister_caller, subaccount: None}.into(), kind: MintKind::CyclesIn{ from_canister: canister_caller } },  
                fee: Some(BANK_TRANSFER_FEE),
                amt: 123*TRILLION,
                memo: None,
                ts: None,
            }
        }
    );
    assert_eq!(
        canister_caller_logs[0],
        log_3,
    );
    
    use icrc_ledger_types::icrc3::blocks::{GetBlocksResult, BlockWithId};
    let get_blocks_sponse = call_candid_::<_, (GetBlocksResult,)>(&pic, BANK, "icrc3_get_blocks", (vec![StartAndLength{ start: 0, length: 100000 }],)).unwrap().0;   
    assert_eq!(
        get_blocks_sponse.blocks[0],
        BlockWithId{ id: 0u128.into(), block: (&icrc3_value_of_a_block_log(&log_0.1)).into() },
    );
    assert_eq!(
        get_blocks_sponse.blocks[1],
        BlockWithId{ id: 1u128.into(), block: (&icrc3_value_of_a_block_log(&log_1.1)).into() },
    );
    assert_eq!(
        get_blocks_sponse.blocks[2],
        BlockWithId{ id: 2u128.into(), block: (&icrc3_value_of_a_block_log(&log_2.1)).into() },
    );
    assert_eq!(
        get_blocks_sponse.blocks[3],
        BlockWithId{ id: 3u128.into(), block: (&icrc3_value_of_a_block_log(&log_3.1)).into() },
    );
    
    let icrc45_pair = call_candid_::<_, (icrc45::PairResponse,)>(
        &pic, 
        tc, 
        "icrc_45_get_pairs", 
        (icrc45::PairRequest{
            pairs: vec![icrc45::PairId{
                base: icrc45::TokenId{
                    platform: icrc45::INTERNET_COMPUTER_PLATFORM_ID, 
                    path: ByteBuf::from(BANK.as_slice()) 
                },
                quote: icrc45::TokenId{
                    platform: icrc45::INTERNET_COMPUTER_PLATFORM_ID, 
                    path: ByteBuf::from(ICP_LEDGER.as_slice())
                } 
            }],
            depth: None,
        },)
    )
    .unwrap()
    .0
    .unwrap()
    .into_iter().next().unwrap();
    
    assert_eq!(
        icrc45_pair.last,
        cts_lib::tools::cycles_per_token_rate_as_f64(p2_trade_cycles_quest.cycles_per_token_rate, 8),
    );
    
    println!("last rate f64: {}", icrc45_pair.last);
    
    
    
}


// ---------------



type GitCommitId = String;
#[derive(Debug)]
struct CanisterGitVersions {
    cts: GitCommitId,
    bank: GitCommitId,
    cm_main: GitCommitId,
    fueler: GitCommitId,
    top_level_upgrader: GitCommitId,
    cm_tc: GitCommitId,
    cm_positions_storage: GitCommitId,
    cm_trades_storage: GitCommitId,
}
impl CanisterGitVersions{
    /*
    fn same(git_commit_id: GitCommitId) -> Self {
        Self {
            cts: git_commit_id.clone(),
            bank: git_commit_id.clone(),
            cm_main: git_commit_id.clone(),
            fueler: git_commit_id.clone(),
            top_level_upgrader: git_commit_id.clone(),
            cm_tc: git_commit_id.clone(),
            cm_positions_storage: git_commit_id.clone(),
            cm_trades_storage: git_commit_id.clone(),
            
        }
    }
    */
    fn current_live_mainnet_versions() -> Self {
        fn get_current_mainnet_canister_git_commit_id(c: Principal) -> GitCommitId {
            use ic_agent::{Agent};
            let rt = tokio::runtime::Runtime::new().unwrap();
            let agent = Agent::builder().with_url("https://icp-api.io").build().unwrap();
            let b = rt.block_on(agent.read_state_canister_metadata(c, "git_commit_id")).unwrap();
            String::from_utf8(b).unwrap()
        }
        let cm_tc_git_commit_id = get_current_mainnet_canister_git_commit_id(Principal::from_text("xvedx-siaaa-aaaar-qactq-cai").unwrap());
        Self {
            cts: get_current_mainnet_canister_git_commit_id(CTS),
            bank: get_current_mainnet_canister_git_commit_id(BANK),
            cm_main: get_current_mainnet_canister_git_commit_id(CM_MAIN),
            fueler: get_current_mainnet_canister_git_commit_id(FUELER),
            top_level_upgrader: get_current_mainnet_canister_git_commit_id(TOP_LEVEL_UPGRADER),
            cm_tc: cm_tc_git_commit_id.clone(),   
            cm_positions_storage: cm_tc_git_commit_id.clone(), // there are no live storage-canisters yet so for now we'll use the cm_tc git version
            cm_trades_storage: cm_tc_git_commit_id,
        }
    }
}




fn get_canister_modules_of_the_git_versions(start_at_version: CanisterGitVersions) -> (TopLevelModules, TCsModules) {

    let mut top_level_modules = TopLevelModules::blank();
    let mut tcs_modules = TCsModules::blank();
    
    let mut commit_canisters_modules = BTreeMap::<String, BTreeMap<&str, &mut Vec<u8>>>::new();
    commit_canisters_modules.entry(start_at_version.cts)                 .or_insert(BTreeMap::new()).insert("cts.wasm",                   &mut top_level_modules.cts);
    commit_canisters_modules.entry(start_at_version.bank)                .or_insert(BTreeMap::new()).insert("bank.wasm",                  &mut top_level_modules.bank);
    commit_canisters_modules.entry(start_at_version.cm_main)             .or_insert(BTreeMap::new()).insert("cm_main.wasm",               &mut top_level_modules.cm_main);
    commit_canisters_modules.entry(start_at_version.fueler)              .or_insert(BTreeMap::new()).insert("fueler.wasm",                &mut top_level_modules.fueler);
    commit_canisters_modules.entry(start_at_version.top_level_upgrader)  .or_insert(BTreeMap::new()).insert("top_level_upgrader.wasm",    &mut top_level_modules.top_level_upgrader);
    commit_canisters_modules.entry(start_at_version.cm_tc)               .or_insert(BTreeMap::new()).insert("cm_tc.wasm",                 &mut tcs_modules.cm_tc);
    commit_canisters_modules.entry(start_at_version.cm_positions_storage).or_insert(BTreeMap::new()).insert("cm_positions_storage.wasm",  &mut tcs_modules.cm_positions_storage);
    commit_canisters_modules.entry(start_at_version.cm_trades_storage)   .or_insert(BTreeMap::new()).insert("cm_trades_storage.wasm",     &mut tcs_modules.cm_trades_storage);
        
    let (tx, rx) = mpsc::channel();
    let mut join = Vec::new();
    
    for (commit, filenames_modules) in commit_canisters_modules.iter() {
        let tx_clone = tx.clone();
        let commit = commit.clone();
        let filenames: Vec<&str> = filenames_modules.keys().copied().collect();
        join.push(
            thread::spawn(move || {
                let modules = build_canisters_with_git_commit(&filenames, &commit);
                tx_clone.send((commit, modules)).unwrap();
            })
        );
    }

    for _ in 0..join.len() { 
        let (commit, modules) = rx.recv().unwrap();
        for (module_placeholder, module) in commit_canisters_modules.get_mut(&commit).unwrap().values_mut().zip(modules) {   
            **module_placeholder = module;
        }
    }
    
    for handle in join {
        handle.join().unwrap();
    }
    
    (top_level_modules, tcs_modules)
}




fn build_canisters_with_git_commit<'a>(file_names: &[&'static str], git_commit_id: &'a str) -> Vec<Vec<u8>> {
    
    // make new temp folder , 
    let dir = std::env::temp_dir().join(&format!("cts_test_upgrade_temp_dir_{}_{}", file_names.concat(), git_commit_id));
    let _ = std::fs::remove_dir_all(&dir); // ignore result
    std::fs::create_dir(&dir).unwrap(); // check result
    
    // clone git repo,
    {
        let git_clone_status: ExitStatus = Command::new("git")
            .arg("clone")
            .arg(git_dir())
            .arg("cts")
            .env_clear()
            .current_dir(&dir)
            .status()
            .expect("Error starting process to clone git repo");
        assert!(git_clone_status.success());
    }
    
    // checkout the specific commit
    {
        let git_checkout_status: ExitStatus = Command::new("git")
            .arg("checkout")
            .arg(git_commit_id)
            .env_clear()
            .current_dir((&dir).join("cts"))
            .status()
            .expect("Error git checkout the specific commit");
        assert!(git_checkout_status.success());
    }
    
    // just build canisters in the new repo
    {
        let build_status: ExitStatus = Command::new("just")
            .arg("build")
            .arg("release")
            .env_clear()
            .env("PATH", env!("PATH"))
            .current_dir((&dir).join("cts"))
            .status()
            .expect("Error starting process to build canisters");
        /*
        let build_status: ExitStatus = Command::new("bash")
            .arg("scripts/podman_build.sh")
            .env_clear()
            .env("PATH", env!("PATH"))
            .current_dir((&dir).join("cts"))
            .status()
            .expect("Error starting process to build canisters");
        */
        assert!(build_status.success());
    }
    
    // get modules and return them
    let mut modules = Vec::new();
    for file_name in file_names {
        let module = std::fs::read((&dir).join(&format!("cts/build/{file_name}"))).unwrap();
        modules.push(module);
    }
    
    // clean up
    let _ = std::fs::remove_dir_all(&dir); // ignore result
    
    modules
}

/*
#[derive(Clone)]
struct CanisterMemoryIds {
    canister: Principal,
    controller: Principal, 
    global_variables: Vec<u8>, // list of memory-ids
    stable_variables: Vec<u8>, // list of memory-ids
}
#[derive(Clone, PartialEq, Eq, Debug)]
struct CanisterMemoriesRawData {
    global_variables: Vec<Vec<u8>>, // list of memory-backups
    stable_variables: Vec<Vec<u8>>, // list of memory-backups
}

fn download_canister_memories(canister_memory_ids: &CanisterMemoryIds) -> CanisterMemoriesRawData {
    CanisterMemoriesRawData{
        global_variables: vec![],
        stable_variables: vec![]
    }
}

fn get_canisters_memory_ids(tcs: &[Principal]) -> Vec<CanisterMemoryIds> {
    [
        CanisterMemoryIds{
            canister: CTS,
            controller: SNS_ROOT,
            global_variables: vec![0],
            stable_variables: vec![],
        },
        CanisterMemoryIds{
            canister: CM_MAIN,
            controller: SNS_ROOT,
            global_variables: vec![0],
            stable_variables: vec![],
        },
        CanisterMemoryIds{
            canister: BANK,
            controller: SNS_ROOT,
            global_variables: vec![0, 3],
            stable_variables: vec![1, 2]
        },
        CanisterMemoryIds{
            canister: FUELER,
            controller: SNS_ROOT,
            global_variables: vec![0],
            stable_variables: vec![],
        },
        CanisterMemoryIds{
            canister: TOP_LEVEL_UPGRADER,
            controller: SNS_ROOT,
            global_variables: vec![0],
            stable_variables: vec![],
        }
    ].into_iter()
    .chain(
        tcs
        .iter().copied().map(|tc| { // put storage canisters too by calling the view_storage_canisters' methods on the tcs
            CanisterMemoryIds{
                canister: tc,
                controller: CM_MAIN,
                global_variables: vec![0, 1, 2],
                stable_variables: vec![]
            }
        })
    )
    .collect()
}
*/

fn upgrade_to_current_release_versions(pic: &PocketIc) {
    // upgrade to current canisters in this working copy directory 
    // use the management-canister install_code api for this upgrade of the top-level. don't use pic upgrade-functions for this upgrade.
    let upgrade_to_top_level_modules = TopLevelModules::default_release();
    let upgrade_to_tcs_modules       = TCsModules::default_release();
    
    use outsiders::management_canister::{InstallCodeArgs, CanisterInstallMode, StopCanisterArgs, StartCanisterArgs};
    
    // upgrades done by the SNS root
    for (canister, upgrade_to_module) in [
        (FUELER, upgrade_to_top_level_modules.fueler),
        (TOP_LEVEL_UPGRADER, upgrade_to_top_level_modules.top_level_upgrader)
    ] {
        fn call__<T: CandidType>(pic: &PocketIc, canister: Principal, method_name: &str, q: T) {
            call_candid_as::<_, ()>(
                &pic,
                Principal::management_canister(),
                pocket_ic::common::rest::RawEffectivePrincipal::CanisterId(canister.as_slice().into()),
                SNS_ROOT,
                method_name,
                (q,),
            ).unwrap()
        }
        call__(&pic, canister, "stop_canister", StopCanisterArgs{canister_id: canister});
        call__(&pic, canister, "install_code", InstallCodeArgs{
            arg: ByteBuf::from(candid::encode_args(()).unwrap()),
            wasm_module: ByteBuf::from(upgrade_to_module),
            mode: CanisterInstallMode::Upgrade(None),
            canister_id: canister,
            sender_canister_version: None,
        });
        call__(&pic, canister, "start_canister", StartCanisterArgs{canister_id: canister})
        
    } 
    
    // uprades done by the top-level-upgrader
    for (canister, upgrade_to_module) in [
        (CTS, upgrade_to_top_level_modules.cts), 
        (BANK, upgrade_to_top_level_modules.bank), 
        (CM_MAIN, upgrade_to_top_level_modules.cm_main), 
    ] {
        let uo = call_candid_as_::<_, (UpgradeOutcome,)>(
            &pic,
            TOP_LEVEL_UPGRADER,
            SNS_GOVERNANCE,
            "upgrade_top_level_canister",
            (UpgradeTopLevelCanisterQuest{
                canister_id: canister,
                cc: CanisterCode::new(upgrade_to_module), 
                post_upgrade_quest: Vec::new(),
                take_canister_snapshot: false, // for now false while pic doesn't support it 
            },)
        ).unwrap().0;
        uo.stop_canister_result.unwrap().unwrap();
        uo.install_code_result.unwrap().unwrap();
        uo.start_canister_result.unwrap().unwrap();
    }
    
    // upgrade tcs
    use cts_lib::tools::upgrade_canisters::*;
    let upgrade_tcs_rs = call_candid_as_::<_, (Vec<(Principal, UpgradeOutcome)>,)>(
        &pic,
        CM_MAIN,
        SNS_GOVERNANCE,
        "controller_upgrade_tcs",
        (ControllerUpgradeCSQuest {
            specific_cs: None, 
            new_canister_code: Some(CanisterCode::new(upgrade_to_tcs_modules.cm_tc)), 
            post_upgrade_quest: candid::encode_args(()).unwrap(),
            take_canisters_snapshots: false,
        },)
    ).unwrap().0;
    for (_tc, r) in upgrade_tcs_rs {
        r.stop_canister_result.unwrap().unwrap();
        r.install_code_result.unwrap().unwrap();    
        r.start_canister_result.unwrap().unwrap();
    }
    
    // for the do: upgrade storage canisters


}
