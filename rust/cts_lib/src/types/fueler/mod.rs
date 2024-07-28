use candid::{Principal, CandidType, Deserialize};
use std::time::Duration;
use crate::consts::{SECONDS_IN_A_DAY, TRILLION};


pub const RHYTHM: Duration = Duration::from_secs(SECONDS_IN_A_DAY as u64 * 1);
pub const FUEL_TOPUP_TRIGGER_THRESHOLD: u128 = 30 * TRILLION; // canisters that go below this threshold will be topped up.
pub const FUEL_TOPUP_TO_MINIMUM_BALANCE: u128 = 50 * TRILLION; // how much to top up canisters that went below the threshold.


#[derive(CandidType, Deserialize)]
pub struct FuelerData {
    pub sns_root: Principal, // use the sns_root to find the canisters that the sns-root controlls.
    pub cm_main: Principal, // use the cm_main to get the cycles-balances of the cm_tcs
    pub cts_cycles_bank: Principal, // use to call the special method canister_cycles_balance_minus_total_supply to know when to topup this canister.
}
impl FuelerData {
    pub fn new() -> Self {
        Self {
            sns_root: Principal::from_slice(&[]),
            cm_main: Principal::from_slice(&[]),
            cts_cycles_bank: Principal::from_slice(&[]),
        }
    }
}

