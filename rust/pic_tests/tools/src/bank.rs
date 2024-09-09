use super::*;
use cts_lib::types::bank::*;


pub fn mint_cycles(pic: &PocketIc, countid: &Account, burn_icp: u128) -> Cycles {
    let mint_cycles_quest = MintCyclesQuest{ 
        burn_icp,
        burn_icp_transfer_fee: ICP_LEDGER_TRANSFER_FEE, 
        to: countid.clone().into(),
        fee: None,
        memo: None,
    };
    mint_icp(&pic, &Account{owner: BANK, subaccount: Some(principal_token_subaccount(&countid.owner))}, burn_icp + ICP_LEDGER_TRANSFER_FEE);
    call_candid_as::<_, (MintCyclesResult,)>(&pic, BANK, RawEffectivePrincipal::None, countid.owner, "mint_cycles", (mint_cycles_quest,)).unwrap().0
    .unwrap().mint_cycles
}

pub fn get_logs_backwards(pic: &PocketIc, bank: Principal, icrcid: &Account, opt_start_before_block: Option<u128>) -> GetLogsBackwardsSponse {
    call_candid::<_, (GetLogsBackwardsSponse,)>(&pic, bank, RawEffectivePrincipal::None, "get_logs_backwards", (icrcid, opt_start_before_block)).unwrap().0
}
