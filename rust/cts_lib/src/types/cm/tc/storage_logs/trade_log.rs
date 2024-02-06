use crate::{
    icrc::Tokens,
    types::{
        cm::{
            tc::{
                CyclesPerToken,
                PositionKind,
            }
        }        
    },
    tools::{principal_as_thirty_bytes, thirty_bytes_as_principal},
};
use super::{*};




#[derive(Clone, Copy, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct PayoutData {
    pub did_transfer: bool, // if false that means it is dust-collection.
    pub ledger_transfer_fee: Tokens,
}

// -----------------

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TradeLog {
    pub position_id_matcher: PositionId,
    pub position_id_matchee: PositionId,
    pub id: PurchaseId,
    pub matchee_position_positor: Principal,
    pub matcher_position_positor: Principal,
    pub tokens: Tokens,
    pub cycles: Cycles,
    pub cycles_per_token_rate: CyclesPerToken,
    pub matchee_position_kind: PositionKind,
    pub timestamp_nanos: u128,
    pub tokens_payout_fee: Tokens,
    pub cycles_payout_fee: Cycles,
    pub cycles_payout_data: Option<PayoutData>,
    pub token_payout_data: Option<PayoutData>,
}




impl StorageLogTrait for TradeLog {
    const STABLE_MEMORY_SERIALIZE_SIZE: usize = 225;    
    const STABLE_MEMORY_VERSION: u16 = 0; 
    fn stable_memory_serialize(&self) -> Vec<u8> {//[u8; Self::STABLE_MEMORY_SERIALIZE_SIZE] {
        let mut s: [u8; Self::STABLE_MEMORY_SERIALIZE_SIZE] = [0; Self::STABLE_MEMORY_SERIALIZE_SIZE];
        s[0..2].copy_from_slice(&(<Self as StorageLogTrait>::STABLE_MEMORY_VERSION).to_be_bytes());
        s[2..18].copy_from_slice(&self.position_id_matchee.to_be_bytes());
        s[18..34].copy_from_slice(&self.id.to_be_bytes());
        s[34..64].copy_from_slice(&principal_as_thirty_bytes(&self.matchee_position_positor));
        s[64..94].copy_from_slice(&principal_as_thirty_bytes(&self.matcher_position_positor));
        s[94..110].copy_from_slice(&self.tokens.to_be_bytes());
        s[110..126].copy_from_slice(&self.cycles.to_be_bytes());
        s[126..142].copy_from_slice(&self.cycles_per_token_rate.to_be_bytes());
        s[142] = if let PositionKind::Cycles = self.matchee_position_kind { 0 } else { 1 };
        s[143..159].copy_from_slice(&self.timestamp_nanos.to_be_bytes());
        s[159..175].copy_from_slice(&self.tokens_payout_fee.to_be_bytes());
        s[175..191].copy_from_slice(&self.cycles_payout_fee.to_be_bytes());
        s[191..207].copy_from_slice(&self.position_id_matcher.to_be_bytes());
        if let Some(ref cycles_payout_data) = self.cycles_payout_data {
            s[207..215].copy_from_slice(&(cycles_payout_data.ledger_transfer_fee as u64).to_be_bytes());
            s[223] = (cycles_payout_data.did_transfer == false) as u8;
        }
        if let Some(ref token_payout_data) = self.token_payout_data {
            s[215..223].copy_from_slice(&(token_payout_data.ledger_transfer_fee as u64).to_be_bytes());    
            s[224] = (token_payout_data.did_transfer == false) as u8;    
        }
        Vec::from(s)
    }
    fn stable_memory_serialize_backwards(b: &[u8]) -> Self {
        Self {
            position_id_matchee: u128::from_be_bytes(b[2..18].try_into().unwrap()),
            id: u128::from_be_bytes(b[18..34].try_into().unwrap()),
            matchee_position_positor: thirty_bytes_as_principal(b[34..64].try_into().unwrap()),
            matcher_position_positor: thirty_bytes_as_principal(b[64..94].try_into().unwrap()),
            tokens: u128::from_be_bytes(b[94..110].try_into().unwrap()),
            cycles: u128::from_be_bytes(b[110..126].try_into().unwrap()),
            cycles_per_token_rate: u128::from_be_bytes(b[126..142].try_into().unwrap()),
            matchee_position_kind: if b[142] == 0 { PositionKind::Cycles } else { PositionKind::Token },
            timestamp_nanos: u128::from_be_bytes(b[143..159].try_into().unwrap()),
            tokens_payout_fee: u128::from_be_bytes(b[159..175].try_into().unwrap()),
            cycles_payout_fee: u128::from_be_bytes(b[175..191].try_into().unwrap()),
            position_id_matcher: u128::from_be_bytes(b[191..207].try_into().unwrap()),
            cycles_payout_data: {
                let ledger_transfer_fee = u64::from_be_bytes(b[207..215].try_into().unwrap()) as u128;
                if ledger_transfer_fee == 0 { None } else {
                    Some(PayoutData{
                        ledger_transfer_fee,
                        did_transfer: b[223] == 0
                    })
                }                 
            },
            token_payout_data: {
                let ledger_transfer_fee = u64::from_be_bytes(b[215..223].try_into().unwrap()) as u128;
                if ledger_transfer_fee == 0 { None } else {
                    Some(PayoutData{
                        ledger_transfer_fee,
                        did_transfer: b[224] == 0
                    })
                }                 
            },
        }  
    }
    fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128 {
        u128::from_be_bytes(log_b[18..34].try_into().unwrap())
    }
    type LogIndexKey = PositionId;    
    fn index_keys_of_the_log_serialization(log_b: &[u8]) -> Vec<Self::LogIndexKey> {
        vec![ 
            u128::from_be_bytes(log_b[2..18].try_into().unwrap()),
            u128::from_be_bytes(log_b[191..207].try_into().unwrap())  
        ]
    }
}


pub fn tokens_quantity_of_the_log_serialization(log_b: &[u8]) -> Tokens {
    u128::from_be_bytes(log_b[94..110].try_into().unwrap())        
}
pub fn rate_of_the_log_serialization(log_b: &[u8]) -> CyclesPerToken {
    u128::from_be_bytes(log_b[126..142].try_into().unwrap())        
}
pub fn timestamp_nanos_of_the_log_serialization(log_b: &[u8]) -> u128 {
    u128::from_be_bytes(log_b[143..159].try_into().unwrap())        
}


