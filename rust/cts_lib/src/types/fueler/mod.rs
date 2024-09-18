use candid::{CandidType, Deserialize};
use std::time::Duration;
use crate::consts::{SECONDS_IN_A_DAY, TRILLION};


pub const RHYTHM: Duration = Duration::from_secs(SECONDS_IN_A_DAY as u64 * 1);
pub const FUEL_TOPUP_TRIGGER_THRESHOLD: u128 = 40 * TRILLION; // canisters that go below this threshold will be topped up.
pub const FUEL_TOPUP_TO_MINIMUM_BALANCE: u128 = 60 * TRILLION; // how much to top up canisters that went below the threshold.


#[derive(CandidType, Deserialize)]
pub struct FuelerData {}
impl FuelerData {
    pub fn new() -> Self {
        Self {}
    }
}
