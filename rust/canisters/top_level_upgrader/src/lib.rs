use ic_cdk::{update, query};
use candid::Principal;
use cts_lib::{
    tools::{
        upgrade_canisters::{upgrade_canisters, UpgradeOutcome},
        caller_is_sns_governance_guard,
        canister_status::{view_canisters_status, ViewCanistersStatusSponse},
    },
    types::top_level_upgrader::*,
    consts::MAINNET_TOP_LEVEL_CANISTERS,
};



#[query]
pub fn sns_validate_upgrade_top_level_canister(q: UpgradeTopLevelCanisterQuest) -> Result<String,String> {
    q.cc.verify_module_hash().unwrap();
    let mut str = format!("Upgrade the top-level canister: {} with the module-hash: {}.", q.canister_id, q.cc.module_hash_hex()); 
    str.push_str(&format!("\npost_upgrade_arg: {}", hex::encode(&q.post_upgrade_quest)));
    str.push_str(&format!("\ntake_canister_snapshot: {}", q.take_canister_snapshot));
    Ok(str)
}

#[update]
pub async fn upgrade_top_level_canister(q: UpgradeTopLevelCanisterQuest) -> UpgradeOutcome {
    caller_is_sns_governance_guard();
    
    q.cc.verify_module_hash().unwrap();
    
    ic_cdk::print(&format!("Upgrading: {} with module-hash: {}", q.canister_id, q.cc.module_hash_hex()));
    
    let uo: UpgradeOutcome = upgrade_canisters(
        vec![q.canister_id],
        &q.cc,
        &q.post_upgrade_quest,
        q.take_canister_snapshot,
    ).await.into_iter().next().unwrap().1;

    ic_cdk::print(&format!("Upgrade outcome when upgrading: {} with module-hash: {}:\n{:?}", q.canister_id, q.cc.module_hash_hex(), uo.clone()));

    uo
}



#[query]
pub fn view_top_level_canisters() -> Vec<Principal> { 
    MAINNET_TOP_LEVEL_CANISTERS.to_vec()
}    


#[update]
pub async fn view_top_level_canisters_status() -> ViewCanistersStatusSponse { 
    view_canisters_status(MAINNET_TOP_LEVEL_CANISTERS.into()).await
}    




ic_cdk::export_candid!();
