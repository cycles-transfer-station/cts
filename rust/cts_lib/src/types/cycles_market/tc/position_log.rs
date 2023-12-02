use candid::Principal;
use crate::tools::thirty_bytes_as_principal;
use super::*;
pub const STABLE_MEMORY_SERIALIZE_SIZE: usize = 172;

pub fn index_keys_of_the_log_serialization(b: &[u8]) -> Vec<Principal> {
    vec![ thirty_bytes_as_principal(&b[18..48].try_into().unwrap()) ]
} 

pub fn log_id_of_the_log_serialization(b: &[u8]) -> u128 {
    u128::from_be_bytes(b[2..18].try_into().unwrap())
} 


// this one goes into the PositionLog storage and gets updated for the position-termination.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PositionLog {
    pub id: PositionId,
    pub positor: Principal,
    pub quest: CreatePositionQuestLog,
    pub position_kind: PositionKind,
    pub mainder_position_quantity: u128, // if cycles position this is: Cycles, if Token position this is: Tokens.
    pub fill_quantity: u128, // if mainder_position_quantity is: Cycles, this is: Tokens. if mainder_position_quantity is: Tokens, this is Cycles.
    pub fill_average_rate: CyclesPerToken,
    pub payouts_fees_sum: u128, // // if cycles-position this is: Tokens, if token-position this is: Cycles.
    pub creation_timestamp_nanos: u128,
    pub position_termination: Option<PositionTerminationData>,
    pub void_position_payout_dust_collection: bool,
    pub void_token_position_payout_ledger_transfer_fee: u64, // in the use for the token-positions.
}


#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CreatePositionQuestLog {
    pub quantity: u128,
    pub cycles_per_token_rate: CyclesPerToken
}

impl From<BuyTokensQuest> for CreatePositionQuestLog {
    fn from(q: BuyTokensQuest) -> Self {
        Self {
            quantity: q.cycles,
            cycles_per_token_rate: q.cycles_per_token_rate 
        }
    }
}
impl From<SellTokensQuest> for CreatePositionQuestLog {
    fn from(q: SellTokensQuest) -> Self {
        Self {
            quantity: q.tokens,
            cycles_per_token_rate: q.cycles_per_token_rate 
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct PositionTerminationData {
    pub timestamp_nanos: u128,
    pub cause: PositionTerminationCause
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum PositionTerminationCause {
    Fill, // the position is fill[ed]. position.amount < minimum_token_match()
    Bump, // the position got bumped
    TimePass, // expired
    UserCallVoidPosition, // the user cancelled the position by calling void_position
}
