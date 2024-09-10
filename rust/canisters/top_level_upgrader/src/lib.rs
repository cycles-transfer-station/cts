use ic_cdk::update;
use candid::Principal;
use cts_lib::{
    tools::{
        upgrade_canisters::{upgrade_canisters, UpgradeCanister, UpgradeOutcome},
        caller_is_sns_governance_guard,
    },
    types::{
        CanisterCode,
    }
};


#[update]
pub async fn upgrade_top_level_canister(canister_id: Principal, cc: CanisterCode, post_upgrade_quest: Vec<u8>) {
    caller_is_sns_governance_guard();
    
    cc.verify_module_hash().unwrap();
    
    ic_cdk::print(&format!("Upgrading: {} with module-hash: {}", canister_id, cc.module_hash_hex()));
    
    let uo: UpgradeOutcome = upgrade_canisters(
        vec![UpgradeCanister{ canister_id, take_canister_snapshot: true }],
        &cc,
        &post_upgrade_quest
    ).await.into_iter().next().unwrap().1;

    ic_cdk::print(&format!("Upgrade outcome when upgrading: {} with module-hash: {}:\n{:?}", canister_id, cc.module_hash_hex(), uo));
}



ic_cdk::export_candid!();
