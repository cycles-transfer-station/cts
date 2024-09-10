use ic_cdk::{update, query};
use candid::{Principal, CandidType, Deserialize};
use cts_lib::{
    tools::{
        upgrade_canisters::{upgrade_canisters, UpgradeOutcome},
        caller_is_sns_governance_guard,
    },
    types::CanisterCode,
};


#[derive(CandidType, Deserialize)]
pub struct UpgradeTopLevelCanisterQuest{
    canister_id: Principal,
    cc: CanisterCode, 
    post_upgrade_quest: Vec<u8>
}

#[query]
pub fn sns_validate_upgrade_top_level_canister(q: UpgradeTopLevelCanisterQuest) -> Result<String,String> {
    q.cc.verify_module_hash().unwrap();
    let mut str = format!("Upgrade the top-level canister: {} with the module-hash: {}.", q.canister_id, q.cc.module_hash_hex()); 
    str.push_str(&format!("\npost_upgrade_arg: {}", hex::encode(&q.post_upgrade_quest)));
    Ok(str)
}

#[update]
pub async fn upgrade_top_level_canister(q: UpgradeTopLevelCanisterQuest) {
    caller_is_sns_governance_guard();
    
    q.cc.verify_module_hash().unwrap();
    
    ic_cdk::print(&format!("Upgrading: {} with module-hash: {}", q.canister_id, q.cc.module_hash_hex()));
    
    let uo: UpgradeOutcome = upgrade_canisters(
        vec![q.canister_id],
        &q.cc,
        &q.post_upgrade_quest,
        true,
    ).await.into_iter().next().unwrap().1;

    ic_cdk::print(&format!("Upgrade outcome when upgrading: {} with module-hash: {}:\n{:?}", q.canister_id, q.cc.module_hash_hex(), uo));
}



ic_cdk::export_candid!();
