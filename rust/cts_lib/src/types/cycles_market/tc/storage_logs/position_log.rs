use super::*;
use candid::Principal;
use crate::tools::{thirty_bytes_as_principal, principal_as_thirty_bytes};



// this one goes into the PositionLog storage and gets updated for the position-termination.
#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
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
    pub void_position_payout_ledger_transfer_fee: u64, // in the use for the token-positions.
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CreatePositionQuestLog {
    pub quantity: u128,
    pub cycles_per_token_rate: CyclesPerToken
}
impl From<TradeCyclesQuest> for CreatePositionQuestLog {
    fn from(q: TradeCyclesQuest) -> Self {
        Self {
            quantity: q.cycles,
            cycles_per_token_rate: q.cycles_per_token_rate 
        }
    }
}
impl From<TradeTokensQuest> for CreatePositionQuestLog {
    fn from(q: TradeTokensQuest) -> Self {
        Self {
            quantity: q.tokens,
            cycles_per_token_rate: q.cycles_per_token_rate 
        }
    }
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct PositionTerminationData {
    pub timestamp_nanos: u128,
    pub cause: PositionTerminationCause
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum PositionTerminationCause {
    Fill, // the position is fill[ed]. position.amount < minimum_token_match()
    Bump, // the position got bumped
    TimePass, // expired
    UserCallVoidPosition, // the user cancelled the position by calling void_position
}

impl StorageLogTrait for PositionLog {
    const STABLE_MEMORY_SERIALIZE_SIZE: usize = 172;  
    const STABLE_MEMORY_VERSION: u16 = 0;
    fn stable_memory_serialize(&self) -> Vec<u8> {// [u8; PositionLog::STABLE_MEMORY_SERIALIZE_SIZE] {
        let mut s: [u8; PositionLog::STABLE_MEMORY_SERIALIZE_SIZE] = [0u8; PositionLog::STABLE_MEMORY_SERIALIZE_SIZE];
        s[0..2].copy_from_slice(&(<Self as StorageLogTrait>::STABLE_MEMORY_VERSION).to_be_bytes());        
        s[2..18].copy_from_slice(&self.id.to_be_bytes());
        s[18..48].copy_from_slice(&principal_as_thirty_bytes(&self.positor));
        s[48..64].copy_from_slice(&self.quest.quantity.to_be_bytes());
        s[64..80].copy_from_slice(&self.quest.cycles_per_token_rate.to_be_bytes());
        s[80] = if let PositionKind::Cycles = self.position_kind { 0 } else { 1 };
        s[81..97].copy_from_slice(&self.mainder_position_quantity.to_be_bytes());
        s[97..113].copy_from_slice(&self.fill_quantity.to_be_bytes());
        s[113..129].copy_from_slice(&self.fill_average_rate.to_be_bytes());
        s[129..145].copy_from_slice(&self.payouts_fees_sum.to_be_bytes());
        s[145..153].copy_from_slice(&(self.creation_timestamp_nanos as u64).to_be_bytes());
        if let Some(ref data) = self.position_termination { 
            s[153] = 1; 
            s[154..162].copy_from_slice(&(data.timestamp_nanos as u64).to_be_bytes());
            s[162] = match data.cause {
                PositionTerminationCause::Fill => 0,
                PositionTerminationCause::Bump => 1,
                PositionTerminationCause::TimePass => 2,
                PositionTerminationCause::UserCallVoidPosition => 3
            };
        }        
        s[163] = self.void_position_payout_dust_collection as u8;
        s[164..172].copy_from_slice(&self.void_position_payout_ledger_transfer_fee.to_be_bytes());
        s.to_vec()
    }  
    fn stable_memory_serialize_backwards(b: &[u8]) -> Self {
        Self {
            id: PositionId::from_be_bytes(b[2..18].try_into().unwrap()),
            positor: thirty_bytes_as_principal(b[18..48].try_into().unwrap()),
            quest: CreatePositionQuestLog {
                quantity: u128::from_be_bytes(b[48..64].try_into().unwrap()),
                cycles_per_token_rate: u128::from_be_bytes(b[64..80].try_into().unwrap()),
            },
            position_kind: if b[80] == 0 { PositionKind::Cycles } else { PositionKind::Token },
            mainder_position_quantity: u128::from_be_bytes(b[81..97].try_into().unwrap()), 
            fill_quantity: u128::from_be_bytes(b[97..113].try_into().unwrap()), 
            fill_average_rate: CyclesPerToken::from_be_bytes(b[113..129].try_into().unwrap()),
            payouts_fees_sum: u128::from_be_bytes(b[129..145].try_into().unwrap()),
            creation_timestamp_nanos: u64::from_be_bytes(b[145..153].try_into().unwrap()) as u128,
            position_termination: if b[153] == 1 {
                Some(PositionTerminationData{
                    timestamp_nanos: u64::from_be_bytes(b[154..162].try_into().unwrap()) as u128,
                    cause: match b[162] {
                        0 => PositionTerminationCause::Fill,
                        1 => PositionTerminationCause::Bump,
                        2 => PositionTerminationCause::TimePass,
                        3 => PositionTerminationCause::UserCallVoidPosition,
                        _ => panic!("unknown PositionTerminationCause serialization"),
                    }
                })
            } else { None },
            void_position_payout_dust_collection: b[163] == 1,
            void_position_payout_ledger_transfer_fee: u64::from_be_bytes(b[164..172].try_into().unwrap()),
        }
    }
    fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128 {
        u128::from_be_bytes(log_b[2..18].try_into().unwrap())
    }
    type LogIndexKey = Principal;
    fn index_keys_of_the_log_serialization(log_b: &[u8]) -> Vec<Self::LogIndexKey> {
        vec![ thirty_bytes_as_principal(&log_b[18..48].try_into().unwrap()) ]
    }
}








