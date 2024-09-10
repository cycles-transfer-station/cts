use pic_tools::*;
use pocket_ic::call_candid_as;

use std::thread;
use std::sync::mpsc;
use std::process::{Command, ExitStatus};

use candid::Principal;
use serde_bytes::ByteBuf;

use icrc_ledger_types::icrc1::{account::Account};



#[test]
#[ignore]
fn test_upgrade_1() {
    // will create the canisters at their start_at_versions, 
    // then will put data into them
    // then will upgrade some canisters to the working copy version
    // and check that the data is still there
    
    
    // set up starting versions 
    // update these git-commit-ids when mainnet upgrades
    // these should be in sync with the mainnet canisters
    let start_at_version = CanisterGitVersions::current_live_mainnet_versions();
    
    println!("Starting at versions: {:?}", start_at_version);

    let start_at_modules = get_canister_modules_of_the_git_versions(start_at_version);
            
    let pic = set_up_with_modules(start_at_modules.clone());
    let icp_tc = set_up_tc_with_modules(&pic, start_at_modules.clone());
    let (_ledger1, tc1) = set_up_new_ledger_and_tc(&pic);
    let (_ledger2, tc2) = set_up_new_ledger_and_tc(&pic);
    let (_ledger3, tc3) = set_up_new_ledger_and_tc(&pic);
    
    // put some data    
    let mc_cycles_balance = pic_tools::bank::mint_cycles(&pic, &Account{ owner: Principal::management_canister(), subaccount: None }, 50000000000);    
    
    // backup    
    let canisters_memory_ids: Vec<CanisterMemoryIds> = [
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
        }
    ].into_iter()
    .chain(
        [icp_tc, tc1, tc2, tc3]
        .into_iter().map(|tc| { // put storage canisters too by calling the view_storage_canisters' methods on the tcs
            CanisterMemoryIds{
                canister: tc,
                controller: CM_MAIN,
                global_variables: vec![0, 1, 2],
                stable_variables: vec![]
            }
        })
    )
    .collect();
        
    let canisters_memories_before_upgrades: Vec<CanisterMemoriesRawData> 
        = canisters_memory_ids
            .iter()
            .map(download_canister_memories)
            .collect();
   

    // upgrade to current canisters in this working copy directory 
    // use the management-canister install_code api for this upgrade. don't use pic functions for this upgrade.
    let update_to_modules = CanisterModules::default_release();//get_canister_modules_of_the_git_versions(CanisterGitVersions::same());
    
    use outsiders::management_canister::{InstallCodeArgs, CanisterInstallMode, StopCanisterArgs, StartCanisterArgs};
    
    for (first_level_canister, update_to_module) in [
        (CTS, update_to_modules.cts), 
        (BANK, update_to_modules.bank), 
        (CM_MAIN, update_to_modules.cm_main), 
        (FUELER, update_to_modules.fueler)
    ] {
        let raw_effective_principal = pocket_ic::common::rest::RawEffectivePrincipal::CanisterId(first_level_canister.as_slice().into()); 
        call_candid_as::<_, ()>(
            &pic,
            Principal::management_canister(),
            raw_effective_principal.clone(),
            SNS_ROOT,
            "stop_canister",
            (StopCanisterArgs{
              canister_id: first_level_canister,
            },)
        ).unwrap();
        call_candid_as::<_, ()>(
            &pic,
            Principal::management_canister(),
            raw_effective_principal.clone(),
            SNS_ROOT,
            "install_code",
            (InstallCodeArgs{
              arg: ByteBuf::from(candid::encode_args(()).unwrap()),
              wasm_module: ByteBuf::from(update_to_module),
              mode: CanisterInstallMode::Upgrade(None),
              canister_id: first_level_canister,
              sender_canister_version: None,
            },)
        ).unwrap();
        call_candid_as::<_, ()>(
            &pic,
            Principal::management_canister(),
            raw_effective_principal,
            SNS_ROOT,
            "start_canister",
            (StartCanisterArgs{
              canister_id: first_level_canister,
            },)
        ).unwrap();
    } 
    
    // update tcs
    use cts_lib::tools::upgrade_canisters::*;
    let upgrade_tcs_rs = call_candid_as_::<_, (Vec<(Principal, UpgradeOutcome)>,)>(
        &pic,
        CM_MAIN,
        SNS_GOVERNANCE,
        "controller_upgrade_tcs",
        (ControllerUpgradeCSQuest {
            specific_cs: None, 
            new_canister_code: Some(cts_lib::types::CanisterCode::new(update_to_modules.cm_tc)), 
            post_upgrade_quest: candid::encode_args(()).unwrap(),
            take_snapshots: false,
        },)
    ).unwrap().0;
    for (_tc, upgrade_tc_r) in upgrade_tcs_rs {
        assert!(upgrade_tc_r.take_canister_snapshot_result.is_none()); // temp while waiting for pic to implement snapshots          
        upgrade_tc_r.stop_canister_result.unwrap().unwrap();
        upgrade_tc_r.install_code_result.unwrap().unwrap();    
        upgrade_tc_r.start_canister_result.unwrap().unwrap();
    }
    
    // for the do: upgrade storage canisters
    
    
    
    // check that the data is still there
    let canisters_memories_after_upgrades: Vec<CanisterMemoriesRawData> 
        = canisters_memory_ids
            .iter()
            .map(download_canister_memories)
            .collect();
    
    assert_eq!(
        canisters_memories_before_upgrades,
        canisters_memories_after_upgrades,
    );
    
    assert_eq!(
        mc_cycles_balance,
        icrc1_balance(&pic, BANK, &Account{owner: Principal::management_canister(), subaccount: None}),
    );
    
}


// ---------------



type GitCommitId = String;
#[derive(Debug)]
struct CanisterGitVersions {
    cts: GitCommitId,
    bank: GitCommitId,
    cm_main: GitCommitId,
    fueler: GitCommitId,
    cm_tc: GitCommitId,
    cm_positions_storage: GitCommitId,
    cm_trades_storage: GitCommitId,
}
impl CanisterGitVersions{
    fn same(git_commit_id: GitCommitId) -> Self {
        Self {
            cts: git_commit_id.clone(),
            bank: git_commit_id.clone(),
            cm_main: git_commit_id.clone(),
            fueler: git_commit_id.clone(),
            cm_tc: git_commit_id.clone(),
            cm_positions_storage: git_commit_id.clone(),
            cm_trades_storage: git_commit_id.clone(),
            
        }
    }
    fn current_live_mainnet_versions() -> Self {
        let cm_tc_git_commit_id = get_current_mainnet_canister_git_commit_id(Principal::from_text("xvedx-siaaa-aaaar-qactq-cai").unwrap());
        Self {
            cts: get_current_mainnet_canister_git_commit_id(CTS),
            bank: get_current_mainnet_canister_git_commit_id(BANK),
            cm_main: get_current_mainnet_canister_git_commit_id(CM_MAIN),
            fueler: get_current_mainnet_canister_git_commit_id(FUELER),
            cm_tc: cm_tc_git_commit_id.clone(),   
            cm_positions_storage: cm_tc_git_commit_id.clone(), // there are no live storage-canisters yet so for now we'll use the cm_tc git version
            cm_trades_storage: cm_tc_git_commit_id,
        }
    }
}





fn get_current_mainnet_canister_git_commit_id(c: Principal) -> GitCommitId {
    use ic_agent::{Agent};
    let rt = tokio::runtime::Runtime::new().unwrap();
    let agent = Agent::builder().with_url("https://icp-api.io").build().unwrap();
    let b = rt.block_on(agent.read_state_canister_metadata(c, "git_commit_id")).unwrap();
    String::from_utf8(b).unwrap()
}



fn get_canister_modules_of_the_git_versions(start_at_version: CanisterGitVersions) -> CanisterModules {

    let (tx, rx) = mpsc::channel::<Box<dyn FnOnce(&mut CanisterModules)->() + Send + Sync>>();
    let tx1 = tx.clone();
    let tx2 = tx.clone();
    let tx3 = tx.clone();
    let tx4 = tx.clone();
    let tx5 = tx.clone();
    let tx6 = tx.clone();
    
    let mut modules = CanisterModules::blank();
    
    let forthejoin = [
        thread::spawn(move || { // moves tx into thread
            let module = build_canister_with_git_commit("cts.wasm", &start_at_version.cts);
            tx.send(Box::new(move |modules: &mut CanisterModules| { modules.cts = module; })).unwrap(); // moves module into main thread
        }),
        thread::spawn(move || { // moves tx into thread
            let module = build_canister_with_git_commit("bank.wasm", &start_at_version.bank);
            tx1.send(Box::new(move |modules: &mut CanisterModules| { modules.bank = module; })).unwrap(); // moves module into main thread
        }),
        thread::spawn(move || { // moves tx into thread
            let module = build_canister_with_git_commit("cm_main.wasm", &start_at_version.cm_main);
            tx2.send(Box::new(move |modules: &mut CanisterModules| { modules.cm_main = module; })).unwrap(); // moves module into main thread
        }),
        thread::spawn(move || { // moves tx into thread
            let module = build_canister_with_git_commit("fueler.wasm", &start_at_version.fueler);
            tx3.send(Box::new(move |modules: &mut CanisterModules| { modules.fueler = module; })).unwrap(); // moves module into main thread
        }),
        thread::spawn(move || { // moves tx into thread
            let module = build_canister_with_git_commit("cm_tc.wasm", &start_at_version.cm_tc);
            tx4.send(Box::new(move |modules: &mut CanisterModules| { modules.cm_tc = module; })).unwrap(); // moves module into main thread
        }),
        thread::spawn(move || { // moves tx into thread
            let module = build_canister_with_git_commit("cm_positions_storage.wasm", &start_at_version.cm_positions_storage);
            tx5.send(Box::new(move |modules: &mut CanisterModules| { modules.cm_positions_storage = module; })).unwrap(); // moves module into main thread
        }),
        thread::spawn(move || { // moves tx into thread
            let module = build_canister_with_git_commit("cm_trades_storage.wasm", &start_at_version.cm_trades_storage);
            tx6.send(Box::new(move |modules: &mut CanisterModules| { modules.cm_trades_storage = module; })).unwrap(); // moves module into main thread
        }),
    ];

    for _ in 0..forthejoin.len() { 
        let f = rx.recv().unwrap();
        f(&mut modules);
    }
    
    for handle in forthejoin {
        handle.join().unwrap();
    }
    
    modules
}




fn build_canister_with_git_commit<'a>(file_name: &'static str, git_commit_id: &'a str) -> Vec<u8> {
    
    // make new temp folder , 
    let dir = std::env::temp_dir().join(&format!("cts_test_upgrade_temp_dir_{}_{}", file_name, git_commit_id));
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
    
    // get canister module and return it
    let module = std::fs::read((&dir).join(&format!("cts/build/{file_name}"))).unwrap();
    
    // clean up
    let _ = std::fs::remove_dir_all(&dir); // ignore result
    
    module
}


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
