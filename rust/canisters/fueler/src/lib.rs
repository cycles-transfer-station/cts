use ic_cdk::{
    init,
    pre_upgrade,
    post_upgrade
};
use cts_lib::{
    tools::{
        localkey::refcell::{with_mut},
        canister_status::ViewCanistersStatusSponse,
    },
    ic_ledger_types::{MAINNET_CYCLES_MINTING_CANISTER_ID, ICP_LEDGER_TRANSFER_DEFAULT_FEE},
    types::{
        fueler::*,
        bank::{BANK_TRANSFER_FEE, MintCyclesQuest, MintCyclesResult, MintCyclesSuccess, MintCyclesError, CompleteMintCyclesResult, CompleteMintCyclesError, CyclesOutQuest, CyclesOutError},
    },
    icrc::{BlockId, IcrcId},
    consts::{MAINNET_BANK, MAINNET_CM_MAIN, MAINNET_SNS_ROOT, MAINNET_TOP_LEVEL_UPGRADER},
};
use outsiders::{
    sns_root::{Service as SNSRootService, GetSnsCanistersSummaryRequest},
    cmc::{Service as CMCService},
};
use candid::{Principal};
use canister_tools::MemoryId;
use std::cell::RefCell;
use std::collections::HashMap;

const FUELER_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);

thread_local! {
    static FUELER_DATA: RefCell<FuelerData> = RefCell::new(FuelerData::new()); 
}

//pub struct FuelerInit {}


#[init]
fn init(q: FuelerData) {
    with_mut(&FUELER_DATA, |fueler_data| {
        *fueler_data = q;
    });
    canister_tools::init(&FUELER_DATA, FUELER_DATA_MEMORY_ID);    
    check_thresholds();
    start_timer();
}

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    //canister_tools::post_upgrade(&FUELER_DATA, FUELER_DATA_MEMORY_ID, None::<fn(FuelerData) -> FuelerData>);
    #[derive(candid::CandidType, candid::Deserialize)]
    pub struct OldFuelerData {
        pub sns_root: Principal,
        pub cm_main: Principal,
        pub cts_cycles_bank: Principal,
    }
    canister_tools::post_upgrade(&FUELER_DATA, FUELER_DATA_MEMORY_ID, Some::<fn(OldFuelerData) -> FuelerData>(
        |_old| {
            FuelerData{}
        }
    ));
    
    
    check_thresholds();
    start_timer();
}

fn check_thresholds() {
    if (FUEL_TOPUP_TRIGGER_THRESHOLD < FUEL_TOPUP_TO_MINIMUM_BALANCE) == false {
        ic_cdk::trap("FUEL_TOPUP_TO_MINIMUM_BALANCE must be greater than FUEL_TOPUP_TRIGGER_THRESHOLD.");
    }
}

fn start_timer() {
    ic_cdk_timers::set_timer_interval(
        RHYTHM,
        || ic_cdk::spawn(fuel())
    );
}

async fn fuel() {
    ic_cdk::print("fuel");
    
    // get CMC icp-xdr rate to know about how much icp to burn
    let cmc_service = CMCService(MAINNET_CYCLES_MINTING_CANISTER_ID);
    let xdr_permyriad_per_icp: u64 = match cmc_service.get_icp_xdr_conversion_rate().await {
        Ok((s,)) => {
            s.data.xdr_permyriad_per_icp
        }
        Err(call_error) => {
            ic_cdk::print(&format!("Call error when calling cmc get_icp_xdr_conversion_rate.\nError: {:?}", call_error));
            return;
        }
    };
    
    ic_cdk::print(&format!("Success getting the current xdr_permyriad_per_icp rate: {:?}", xdr_permyriad_per_icp));
    
    let mut topup_canisters: HashMap<Principal, u128/*topup-mount*/> = HashMap::new();
    
    let sns_root_service = SNSRootService(MAINNET_SNS_ROOT);
            
    // the get_sns_canisters_summary does not poll for new archives. the response might not include new archives. 
    match sns_root_service.get_sns_canisters_summary(
        GetSnsCanistersSummaryRequest{ 
            update_canister_list: None // only governance canister caller can set this to true.
        }        
    ).await {
        Ok((s,)) => {
            let s_clone = s.clone(); // for the possible logging need.
            for opt_canister_summary in [
                s.root,
                s.swap,
                s.ledger,
                s.index,
                s.governance,
            ]
            .into_iter()
            .chain(s.dapps.into_iter().map(|cs| Some(cs)))
            .chain(s.archives.into_iter().map(|cs| Some(cs))) {
                
                let canister_summary = match opt_canister_summary {
                    Some(cs) => cs,
                    None => {
                        ic_cdk::print(&format!("Strange, the Option<CanisterSummary> on the response returned by the get_sns_canisters_summary struct is None.\n{:?}", s_clone));
                        continue;
                    }
                };
                
                let canister_id: Principal = match canister_summary.canister_id { 
                    Some(c) => c, 
                    None => {
                        ic_cdk::print(&format!("Strange, canister_summary.canister_id field is None.\n{:?}", s_clone));
                        continue;
                    }
                };
                
                match canister_summary.status {
                    Some(status) => {
                        if status.cycles < FUEL_TOPUP_TRIGGER_THRESHOLD {
                            topup_canisters.insert(canister_id, FUEL_TOPUP_TO_MINIMUM_BALANCE - status.cycles);
                        }
                    }
                    None => {
                        ic_cdk::print(&format!("Strange, canister_summary.status field is None.\n{:?}", s_clone));
                        continue;
                    }
                }   
            }
        }
        Err(call_error) => {
            ic_cdk::print(&format!("Call error when calling root get_sns_canisters_summary.\nError: {:?}", call_error));
            return;
        }
    }    
    
    for (canister, method_name) in [
        (MAINNET_TOP_LEVEL_UPGRADER, "view_top_level_canisters_status"), 
        (MAINNET_CM_MAIN, "view_tcs_status")
    ] {
        match ic_cdk::call::<(), (ViewCanistersStatusSponse,)>(
            canister,
            method_name,
            (),
        ).await { 
            Ok((s,)) => {
                for (tc, status) in s.0.into_iter() {
                    if status.cycles < FUEL_TOPUP_TRIGGER_THRESHOLD {
                        topup_canisters.insert(tc, FUEL_TOPUP_TO_MINIMUM_BALANCE - status.cycles);
                    }
                }
                for (tc, call_error) in s.1.into_iter() {
                    ic_cdk::print(&format!("Error received when cm_main tried getting canister_status of a tc: {},\nError: {:?}", tc, call_error));
                }
            },
            Err(call_error) => {
                ic_cdk::print(&format!("{} call error: {:?}", method_name, call_error));
            }
        };        
    }
        
    // do special method call for the cts-cycles-bank call the canister_cycles_balance_minus_total_supply method. 
    // override entry from view-top-level-canisters-status-sponse
    let cts_cycles_bank: Principal = MAINNET_BANK; 
    match ic_cdk::call::<(), (i128,)>(
        cts_cycles_bank,
        "canister_cycles_balance_minus_total_supply",
        (),
    ).await { 
        Ok((b,)) => {
            if b < FUEL_TOPUP_TRIGGER_THRESHOLD as i128 {
                topup_canisters.insert(cts_cycles_bank, ((FUEL_TOPUP_TO_MINIMUM_BALANCE as i128) - b) as u128);
            }        
        }
        Err(call_error) => {
            ic_cdk::print(&format!("Call error when calling bank canister_cycles_balance_minus_total_supply.\nError: {:?}", call_error));
        }
    };
    
    // fuel up
    
    let sum_fuel: u128 = topup_canisters.values().sum::<u128>().saturating_add(BANK_TRANSFER_FEE * topup_canisters.len() as u128);
    
    // get fueler's bank cycles-balance so we know how much we have on the count. this can come from the shareholder-payouts canister when it collects cycles from the tc-fees.
    let mut fueler_bank_balance: u128 = match cts_lib::icrc::icrc1_balance_of(cts_cycles_bank, IcrcId{owner: ic_cdk::id(), subaccount: None}).await {
        Ok(b) => b,
        Err(e) => {
            ic_cdk::print(&format!("Call error when trying to view the fueler's cts-cycles-bank balance.\n{:?}", e));
            return;
        }
    };
    ic_cdk::print(&format!("fueler's cts-cycles-bank cycles-balance before mint and fuel: {}", fueler_bank_balance));

    if sum_fuel > fueler_bank_balance {

        let icp_need: u128 = (sum_fuel - fueler_bank_balance + BANK_TRANSFER_FEE/*mint-cycles-fee*/) / (xdr_permyriad_per_icp as u128) + 1; // +1 for a possible division remainder

        // call mint_cycles on the cts-cycles-bank. ICP should be in the subaccount from sns-transfer-treasury-funds proposals and the icp-tc fees-collection shareholder treasury transfers.
        let mint_cycles_success: MintCyclesSuccess = match ic_cdk::call::<(MintCyclesQuest,), (MintCyclesResult,)>(
            cts_cycles_bank,
            "mint_cycles",
            (MintCyclesQuest{
                burn_icp: icp_need,
                burn_icp_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() as u128,
                to: IcrcId{owner: ic_cdk::api::id(), subaccount: None},
                fee: None,
                memo: None,
            },)
        ).await {
            Ok((mint_cycles_result,)) => match mint_cycles_result {
                Ok(mint_cycles_success) => {
                    mint_cycles_success
                }
                Err(mint_cycles_error) => {
                    if let MintCyclesError::MidCallError(mid_call_error) = mint_cycles_error {
                        ic_cdk::print(&format!("MidCallError returned when calling mint_cycles on the CTS-CYCLES-BANK: {:?}.", mid_call_error));
                        let mut i: usize = 0;
                        loop {
                            ic_cdk::print(&format!("Performing complete_mint_cycles call number {}", i));
                            match ic_cdk::call::<(Option<Principal>,), (CompleteMintCyclesResult,)>(
                                cts_cycles_bank,
                                "complete_mint_cycles",
                                (None,)
                            ).await {
                                Ok((complete_mint_cycles_result,)) => match complete_mint_cycles_result {
                                    Ok(mint_cycles_success) => {
                                        break mint_cycles_success;
                                    }
                                    Err(complete_mint_cycles_error) => {
                                        if let CompleteMintCyclesError::MintCyclesError(MintCyclesError::MidCallError(mid_call_error)) = complete_mint_cycles_error {
                                            ic_cdk::print(&format!("MidCallError returned when calling complete_mint_cycles on the CTS-CYCLES-BANK: {:?}.", mid_call_error));
                                        } else {
                                            ic_cdk::print(&format!("Error returned when calling complete_mint_cycles on the CTS-CYCLES-BANK: {:?}", complete_mint_cycles_error));
                                            return;
                                        }
                                    }
                                }
                                Err(call_error) => {
                                    ic_cdk::print(&format!("Call error calling complete_mint_cycles on the CTS-CYCLES-BANK: {:?}", call_error));
                                    return;
                                }
                            }
                            i += 1;
                            const COMPLETE_MINT_CYCLES_MAX_TRIES: usize = 20;
                            if i == COMPLETE_MINT_CYCLES_MAX_TRIES {
                                ic_cdk::print(&format!("Called complete_mint_cycles on the CTS-CYCLES-BANK {COMPLETE_MINT_CYCLES_MAX_TRIES} times, got errors."));
                                return;
                            }
                        }
                    } else {
                        ic_cdk::print(&format!("Error returned when calling mint_cycles on the CTS-CYCLES-BANK: {:?}", mint_cycles_error));
                        return;
                    }
                }
            }
            Err(call_error) => {
                ic_cdk::print(&format!("Call error calling mint_cycles on the CTS-CYCLES-BANK: {:?}", call_error));
                return;
            }
        };

        // mint_cycles could be less than the (sum_fuel - fueler_bank_balance) if the icp-xdr rate on the cmc can change between the time we got it and now.
        fueler_bank_balance = fueler_bank_balance.saturating_add(mint_cycles_success.mint_cycles);
    }


    for (for_canister, need_cycles) in topup_canisters.into_iter() {
        // call cycles_out on the cts-cycles-bank
        let quest_cycles = std::cmp::min(need_cycles+BANK_TRANSFER_FEE, fueler_bank_balance).saturating_sub(BANK_TRANSFER_FEE);
        match ic_cdk::call::<(CyclesOutQuest,), (Result<BlockId, CyclesOutError>,)>(
            cts_cycles_bank,
            "cycles_out",
            (CyclesOutQuest{
                cycles: quest_cycles,
                fee: Some(BANK_TRANSFER_FEE),                   // set the fee here because we need to count for the fueler_bank_balance
                from_subaccount: None,
                memo: None,
                for_canister: for_canister,
            },)
        ).await {
            Ok((r,)) => match r {
                Ok(block_id) => {
                    ic_cdk::print(&format!("Topped up canister {} with {} cycles at block-height {}", for_canister, quest_cycles, block_id));
                    fueler_bank_balance -= quest_cycles + BANK_TRANSFER_FEE;
                }
                Err(cycles_out_error) => {
                    ic_cdk::print(&format!("Error returned when calling cycles_out on the CTS-CYCLES-BANK to top-up canister {} with {} cycles. \n{:?}", for_canister, quest_cycles, cycles_out_error));       
                }
            }
            Err(call_error) => {
                ic_cdk::print(&format!("Call error when calling cycles_out on the CTS-CYCLES-BANK to top-up canister {} with {} cycles. \n{:?}", for_canister, quest_cycles, call_error));       
            }
        }
    }
    
    ic_cdk::print("fuel done");
}






ic_cdk::export_candid!();
