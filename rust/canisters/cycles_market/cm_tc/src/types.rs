use std::thread::LocalKey;
use std::cell::RefCell;

use crate::*;
use serde::Serialize;

pub type VoidCyclesPositionId = PositionId;
pub type VoidTokenPositionId = PositionId;




pub trait StorageLogTrait {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>>;    
    const STABLE_MEMORY_SERIALIZE_SIZE: usize;
    const STABLE_MEMORY_VERSION: u16;
    fn stable_memory_serialize(&self) -> Vec<u8>;// Self::STABLE_MEMORY_SERIALIZE_SIZE]; const generics not stable yet
    fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128;
    type LogIndexKey: CandidType + for<'a> Deserialize<'a> + PartialEq + Eq;
    fn index_keys_of_the_log_serialization(log_b: &[u8]) -> Vec<Self::LogIndexKey>;
}


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
    pub void_token_position_payout_ledger_transfer_fee: u64, // in the use for the token-positions.
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
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
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>> = &POSITIONS_STORAGE_DATA;
    const STABLE_MEMORY_SERIALIZE_SIZE: usize = position_log::STABLE_MEMORY_SERIALIZE_SIZE;  
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
        s[164..172].copy_from_slice(&self.void_token_position_payout_ledger_transfer_fee.to_be_bytes());
        s.to_vec()
    }  
    fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128 {
        position_log::log_id_of_the_log_serialization(log_b)
    }
    type LogIndexKey = Principal;
    fn index_keys_of_the_log_serialization(log_b: &[u8]) -> Vec<Self::LogIndexKey> {
        position_log::index_keys_of_the_log_serialization(log_b)
    }
}







// ------------------


pub trait CurrentPositionTrait {
    fn id(&self) -> PositionId;
    fn positor(&self) -> Principal;
    fn current_position_available_cycles_per_token_rate(&self) -> CyclesPerToken;
    fn timestamp_nanos(&self) -> u128;
    
    type VoidPositionType: VoidPositionTrait;
    fn into_void_position_type(self, position_termination_cause: PositionTerminationCause) -> Self::VoidPositionType;
    
    fn current_position_quantity(&self) -> u128;
    
    fn current_position_tokens(&self, rate: CyclesPerToken) -> Tokens;
    fn subtract_tokens(&mut self, sub_tokens: Tokens, rate: CyclesPerToken) -> /*payout_fee_cycles*/Cycles;
    
    // if the position is compatible with the match_rate, 
    // returns the middle rate between this position's available rate and between the match_rate.
    fn is_this_position_better_than_or_equal_to_the_match_rate(&self, match_rate: CyclesPerToken) -> Option<CyclesPerToken>;
    
    const POSITION_KIND: PositionKind;
            
    fn as_stable_memory_position_log(&self, position_termination_cause: Option<PositionTerminationCause>) -> PositionLog;
}




#[derive(CandidType, Serialize, Deserialize)]
pub struct CyclesPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub quest: BuyTokensQuest,
    pub current_position_cycles: Cycles,
    pub purchases_rates_times_cycles_quantities_sum: u128,
    pub fill_quantity_tokens: Tokens,
    pub tokens_payouts_fees_sum: Tokens,
    pub timestamp_nanos: u128,
}


impl CurrentPositionTrait for CyclesPosition {
    fn id(&self) -> PositionId { self.id }
    fn positor(&self) -> Principal { self.positor }
    fn current_position_available_cycles_per_token_rate(&self) -> CyclesPerToken { 
        self.quest.cycles_per_token_rate
        /*
        let total_position_cycles: Cycles = tokens_transform_cycles(self.match_tokens_quest.tokens, self.match_tokens_quest.cycles_per_token_rate);
        find_current_position_available_rate(
            self.purchases_rates_times_cycles_quantities_sum,
            self.match_tokens_quest.cycles_per_token_rate,
            total_position_cycles,
            self.current_position_cycles
        )
        */
    }
    fn timestamp_nanos(&self) -> u128 { self.timestamp_nanos }
    
    type VoidPositionType = VoidCyclesPosition;
    fn into_void_position_type(self, position_termination_cause: PositionTerminationCause) -> Self::VoidPositionType {
        VoidCyclesPosition{
            position_id: self.id,
            positor: self.positor,                                
            cycles: self.current_position_cycles,
            cycles_payout_lock: false,
            cycles_payout_data: CyclesPayoutData::new(),
            timestamp_nanos: time_nanos(),
            update_storage_position_data: VPUpdateStoragePositionData{
                lock: false,
                status: false,
                update_storage_position_log: self.as_stable_memory_position_log(Some(position_termination_cause))
            },
            
        }
    }
    fn current_position_quantity(&self) -> u128 {
        self.current_position_cycles
    }
    fn current_position_tokens(&self, rate: CyclesPerToken) -> Tokens {
        if rate == 0 { return 0; }
        self.current_position_cycles / rate
        //self.match_tokens_quest.tokens.saturating_sub(self.fill_quantity_tokens)
    }
    
    fn subtract_tokens(&mut self, sub_tokens: Tokens, rate: CyclesPerToken) -> /*payout_fee_cycles*/Cycles {
        self.fill_quantity_tokens = self.fill_quantity_tokens.saturating_add(sub_tokens);
        let sub_cycles: Cycles = tokens_transform_cycles(sub_tokens, rate);
        let fee_cycles: Cycles = calculate_trade_fee(self.quest.cycles - self.current_position_cycles, sub_cycles); // calculate trade fee before subtracting from the current_cycles_position in the next line.          
        self.current_position_cycles = self.current_position_cycles.saturating_sub(sub_cycles);
        self.purchases_rates_times_cycles_quantities_sum = self.purchases_rates_times_cycles_quantities_sum.saturating_add(rate * sub_cycles);        
        self.tokens_payouts_fees_sum = self.tokens_payouts_fees_sum.saturating_add(cycles_transform_tokens(fee_cycles, rate));
        fee_cycles
    }
    
    fn is_this_position_better_than_or_equal_to_the_match_rate(&self, match_rate: CyclesPerToken) -> Option<CyclesPerToken> {
        let current_position_available_cycles_per_token_rate = self.current_position_available_cycles_per_token_rate();
        (current_position_available_cycles_per_token_rate >= match_rate).then(|| {
            let difference = current_position_available_cycles_per_token_rate - match_rate;
            current_position_available_cycles_per_token_rate - difference / 2
        })
    }
    
    const POSITION_KIND: PositionKind = PositionKind::Cycles;
    
    fn as_stable_memory_position_log(&self, position_termination_cause: Option<PositionTerminationCause>) -> PositionLog {
        let cycles_sold: Cycles = self.quest.cycles - self.current_position_cycles;
        let fill_average_rate = {
            if cycles_sold == 0 {
                self.quest.cycles_per_token_rate
            } else {
                self.purchases_rates_times_cycles_quantities_sum / cycles_sold
            }
        };
        PositionLog {
            id: self.id,
            positor: self.positor,
            quest: self.quest.clone().into(),
            position_kind: PositionKind::Cycles,
            mainder_position_quantity: self.current_position_cycles, // if cycles position this is: Cycles, if Token position this is: Tokens.
            fill_quantity: self.fill_quantity_tokens, // if mainder_position_quantity is: Cycles, this is: Tokens. if mainder_position_quantity is: Tokens, this is Cycles.
            fill_average_rate,
            payouts_fees_sum: self.tokens_payouts_fees_sum, // // if cycles-position this is: Tokens, if token-position this is: Cycles.
            creation_timestamp_nanos: self.timestamp_nanos,
            position_termination: position_termination_cause.map(|c| {
                PositionTerminationData{
                    timestamp_nanos: time_nanos(),
                    cause: c
                }
            }),
            void_position_payout_dust_collection: false, // this field is update when void-position-payout is done.
            void_token_position_payout_ledger_transfer_fee: 0, // this field is not used for the cycles-positions.
        }
    }


}




#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TokenPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub quest: SellTokensQuest,
    pub current_position_tokens: Tokens,
    pub purchases_rates_times_token_quantities_sum: u128,
    pub cycles_payouts_fees_sum: Cycles,
    pub timestamp_nanos: u128,
}
/*
impl TokenPosition {
    fn cycles_left_for_the_buy(&self) -> Cycles { // private, just a helper function cause we need this in two places in the impl CurrentPositionTrait for TokenPosition.
        tokens_transform_cycles(self.match_tokens_quest.tokens, self.match_tokens_quest.cycles_per_token_rate)
        .saturating_sub(self.purchases_rates_times_token_quantities_sum)
    }
}
*/
impl CurrentPositionTrait for TokenPosition {
    fn id(&self) -> PositionId { self.id }
    fn positor(&self) -> Principal { self.positor }
    fn current_position_available_cycles_per_token_rate(&self) -> CyclesPerToken { 
        self.quest.cycles_per_token_rate
        /*
        let mut rate = find_current_position_available_rate(
            self.purchases_rates_times_token_quantities_sum,
            self.match_tokens_quest.cycles_per_token_rate,
            self.match_tokens_quest.tokens,
            self.current_position_tokens
        ); // if too low due to mainder-cutoff during division, add a couple here to make sure we don't take too low of a cycles-per-token-rate.
        while rate * self.current_position_tokens < self.cycles_left_for_the_buy() {
            rate += 1;
        }
        rate
        */
    }
    fn timestamp_nanos(&self) -> u128 { self.timestamp_nanos }
    
    type VoidPositionType = VoidTokenPosition;
    fn into_void_position_type(self, position_termination_cause: PositionTerminationCause) -> Self::VoidPositionType {
        VoidTokenPosition{
            position_id: self.id,
            positor: self.positor,
            tokens: self.current_position_tokens,
            timestamp_nanos: time_nanos(),
            token_payout_lock: false,
            token_payout_data: TokenPayoutData::new(),
            update_storage_position_data: VPUpdateStoragePositionData{
                status: false,
                lock: false,
                update_storage_position_log: self.as_stable_memory_position_log(Some(position_termination_cause))
                
            }
        }
    }
    fn current_position_quantity(&self) -> u128 {
        self.current_position_tokens
    }
    fn current_position_tokens(&self, _rate: CyclesPerToken) -> Tokens {
        self.current_position_tokens
        //self.cycles_left_for_the_buy() / rate
    }
    fn subtract_tokens(&mut self, sub_tokens: Tokens, rate: CyclesPerToken) -> /*payout_fee_cycles*/Cycles {
        self.current_position_tokens = self.current_position_tokens.saturating_sub(sub_tokens);
        let fee_cycles: Cycles = calculate_trade_fee(self.purchases_rates_times_token_quantities_sum, rate * sub_tokens); // make sure we call this line before we add to the purchases_rates_times_token_quantities_sum, bc we need the total position volume in cycles before this purchase for the calculate_trade_fee fn. 
        self.purchases_rates_times_token_quantities_sum = self.purchases_rates_times_token_quantities_sum.saturating_add(rate * sub_tokens);
        self.cycles_payouts_fees_sum = self.cycles_payouts_fees_sum.saturating_add(fee_cycles);
        fee_cycles
    }
    fn is_this_position_better_than_or_equal_to_the_match_rate(&self, match_rate: CyclesPerToken) -> Option<CyclesPerToken> {
        let current_position_available_cycles_per_token_rate = self.current_position_available_cycles_per_token_rate();
        (current_position_available_cycles_per_token_rate <= match_rate).then(|| {
            let difference = match_rate - current_position_available_cycles_per_token_rate;
            current_position_available_cycles_per_token_rate + difference / 2
        })
    }
    
    const POSITION_KIND: PositionKind = PositionKind::Token;
    
    fn as_stable_memory_position_log(&self, position_termination_cause: Option<PositionTerminationCause>) -> PositionLog {
        let tokens_sold: Cycles = self.quest.tokens - self.current_position_tokens;
        let fill_average_rate = {
            if tokens_sold == 0 {
                self.quest.cycles_per_token_rate
            } else {
                self.purchases_rates_times_token_quantities_sum / tokens_sold
            }
        };
        PositionLog {
            id: self.id,
            positor: self.positor,
            quest: self.quest.clone().into(),
            position_kind: PositionKind::Token,
            mainder_position_quantity: self.current_position_tokens, // if cycles position this is: Cycles, if Token position this is: Tokens.
            fill_quantity: self.purchases_rates_times_token_quantities_sum, // if mainder_position_quantity is: Cycles, this is: Tokens. if mainder_position_quantity is: Tokens, this is Cycles.
            fill_average_rate,
            payouts_fees_sum: self.cycles_payouts_fees_sum, // // if cycles-position this is: Tokens, if token-position this is: Cycles.
            creation_timestamp_nanos: self.timestamp_nanos,
            position_termination: position_termination_cause.map(|c| {
                PositionTerminationData{
                    timestamp_nanos: time_nanos(),
                    cause: c
                }
            }),
            void_position_payout_dust_collection: false, // this field is update when void-position-payout is done.   
            void_token_position_payout_ledger_transfer_fee: 0, // this field is update when a void-token-position-payout is done.                    
        }
    }

}

/*
fn find_current_position_available_rate(
    position_purchases_rates_times_quantities_sum: u128,
    match_quest_cycles_per_token_rate: CyclesPerToken,
    match_quest_quantity: u128, 
    current_position_quantity: u128,
) -> CyclesPerToken {
    let position_purchases_quantity_sum: u128 = match_quest_quantity - current_position_quantity;   
    if position_purchases_quantity_sum == 0 || current_position_quantity == 0 {
        return match_quest_cycles_per_token_rate;
    }
    let average_position_purchases_rate = position_purchases_rates_times_quantities_sum / position_purchases_quantity_sum;
    let rate_for_current_position = ((match_quest_cycles_per_token_rate * match_quest_quantity).saturating_sub(average_position_purchases_rate * position_purchases_quantity_sum)) / current_position_quantity;
    rate_for_current_position
}
*/





// ----------------------------


pub trait PayoutDataTrait {
    fn is_complete(&self) -> bool;
    fn dust_collection(&self) -> bool;
    fn token_payout_ledger_transfer_fee(&self) -> Option<Tokens>; // Some(ledger_transfer_fee) if this is a TokenPayoutData and if the transfer is complete
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CyclesPayoutData {
    pub cycles_payout: Option<bool>, // false means dust-collection.
}
impl CyclesPayoutData {
    pub fn new() -> Self {
        Self {
            cycles_payout: None,
        }
    }
}
impl PayoutDataTrait for CyclesPayoutData {
    fn is_complete(&self) -> bool {
        self.cycles_payout.is_some() 
    }
    fn dust_collection(&self) -> bool {
        if let Some(did_transfer) = self.cycles_payout {
            if did_transfer == false {
                return true;
            }
        }
        false
    }
    fn token_payout_ledger_transfer_fee(&self) -> Option<Tokens> { None }
}


pub trait CyclesPayoutTrait {
    fn cycles_payout_data(&self) -> CyclesPayoutData;
    fn cycles_payout_lock(&self) -> bool;
    fn cycles_payout_payee(&self) -> Principal;
    fn cycles_payout_payee_method(&self) -> &'static str;
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError>;
    fn cycles(&self) -> Cycles;
    fn cycles_payout_fee(&self) -> Cycles;
}



#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TokenTransferData {
    pub did_transfer: bool, // if false that means it is dust-collection.
    pub ledger_transfer_fee: Tokens,
}
#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TokenPayoutData {
    pub token_transfer: Option<TokenTransferData>,
}
impl TokenPayoutData {
    pub fn new() -> Self {
        Self{
            token_transfer: None,
        }
    }
}
impl PayoutDataTrait for TokenPayoutData {
    fn is_complete(&self) -> bool {
        self.token_transfer.is_some()
    }    
    fn dust_collection(&self) -> bool {
        if let Some(ref token_transfer_data) = self.token_transfer {
            if token_transfer_data.did_transfer == false {
                return true;
            }
        }
        false
    }
    fn token_payout_ledger_transfer_fee(&self) -> Option<Tokens> {
        self.token_transfer.as_ref().map(|ttd| ttd.ledger_transfer_fee)
    }
}

pub trait TokenPayoutTrait {
    fn token_payout_data(&self) -> TokenPayoutData;
    fn token_payout_lock(&self) -> bool;
    fn token_payout_payee(&self) -> Principal;
    fn token_payout_payor(&self) -> Principal;
    fn tokens(&self) -> Tokens;
    fn token_transfer_memo(&self) -> Option<IcrcMemo>;
    fn token_fee_collection_transfer_memo(&self) -> Option<IcrcMemo>;
    fn token_ledger_transfer_fee(&self) -> Tokens;
    fn tokens_payout_fee(&self) -> Tokens;
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
    pub cycles_payout_lock: bool,
    pub token_payout_lock: bool,
    pub cycles_payout_data: CyclesPayoutData,
    pub token_payout_data: TokenPayoutData
}

impl TradeLog {
    pub fn can_move_into_the_stable_memory_for_the_long_term_storage(&self) -> bool {
        self.cycles_payout_lock == false
        && self.token_payout_lock == false
        && self.cycles_payout_data.is_complete() == true
        && self.token_payout_data.is_complete() == true
    }
}

impl StorageLogTrait for TradeLog {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>> = &TRADES_STORAGE_DATA;
    const STABLE_MEMORY_SERIALIZE_SIZE: usize = trade_log::STABLE_MEMORY_SERIALIZE_SIZE;    
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
        if let Some(ref token_transfer) = self.token_payout_data.token_transfer {
            s[207..223].copy_from_slice(&token_transfer.ledger_transfer_fee.to_be_bytes())    
        }
        s[223] = self.cycles_payout_data.dust_collection() as u8;
        s[224] = self.token_payout_data.dust_collection() as u8;
        Vec::from(s)
    }
    fn log_id_of_the_log_serialization(log_b: &[u8]) -> u128 {
        trade_log::log_id_of_the_log_serialization(log_b)
    }
    type LogIndexKey = PositionId;    
    fn index_keys_of_the_log_serialization(log_b: &[u8]) -> Vec<Self::LogIndexKey> {
        trade_log::index_keys_of_the_log_serialization(log_b)
    }
}

impl CyclesPayoutTrait for TradeLog {
    fn cycles_payout_data(&self) -> CyclesPayoutData { self.cycles_payout_data.clone() }
    fn cycles_payout_lock(&self) -> bool { self.cycles_payout_lock }
    fn cycles_payout_payee(&self) -> Principal { 
        match self.matchee_position_kind { 
            PositionKind::Cycles => self.matcher_position_positor,
            PositionKind::Token => self.matchee_position_positor,
        }
    }
    fn cycles_payout_payee_method(&self) -> &'static str { 
        CM_MESSAGE_METHOD_TRADE_TOKENS_CYCLES_PAYOUT
    }
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMTradeTokensCyclesPayoutMessageQuest {
                token_position_id: match self.matchee_position_kind { 
                    PositionKind::Cycles => self.position_id_matcher,
                    PositionKind::Token => self.position_id_matchee
                },
                purchase_id: self.id,
            }
        )
    }
    fn cycles(&self) -> Cycles { self.cycles }
    fn cycles_payout_fee(&self) -> Cycles { self.cycles_payout_fee }
}
impl TokenPayoutTrait for TradeLog {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
    fn token_payout_lock(&self) -> bool { self.token_payout_lock }
    fn token_payout_payee(&self) -> Principal { 
        match self.matchee_position_kind { 
            PositionKind::Cycles => self.matchee_position_positor,
            PositionKind::Token => self.matcher_position_positor,
        }
    }
    fn token_payout_payor(&self) -> Principal { 
        match self.matchee_position_kind { 
            PositionKind::Cycles => self.matcher_position_positor,
            PositionKind::Token => self.matchee_position_positor,
        }
    }
    fn tokens(&self) -> Tokens { self.tokens }
    fn token_transfer_memo(&self) -> Option<IcrcMemo> { 
        Some(IcrcMemo(ByteBuf::from(position_purchase_token_transfer_memo(self.matchee_position_kind, self.id))))
    }
    fn token_fee_collection_transfer_memo(&self) -> Option<IcrcMemo> {
        Some(IcrcMemo(ByteBuf::from(position_purchase_token_fee_collection_transfer_memo(self.matchee_position_kind, self.id))))
    }
    fn token_ledger_transfer_fee(&self) -> Tokens { localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) }
    fn tokens_payout_fee(&self) -> Tokens { self.tokens_payout_fee }
}


// --------


pub trait VoidPositionTrait: Clone {
    fn position_id(&self) -> PositionId;
    fn positor(&self) -> Principal;    
    type PayoutData: PayoutDataTrait;
    fn payout_data(&self) -> &Self::PayoutData;
    fn payout_data_mut(&mut self) -> &mut Self::PayoutData;
    fn payout_lock(&mut self) -> &mut bool;
    fn can_remove(&self) -> bool;
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData;
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData;
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct VPUpdateStoragePositionData {
    pub lock: bool,
    pub status: bool,
    pub update_storage_position_log: PositionLog,
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct VoidCyclesPosition {
    pub position_id: PositionId,
    pub positor: Principal,
    pub cycles: Cycles,
    pub cycles_payout_lock: bool,  // lock for the payout
    pub cycles_payout_data: CyclesPayoutData,
    pub timestamp_nanos: u128,
    pub update_storage_position_data: VPUpdateStoragePositionData,
}

impl VoidPositionTrait for VoidCyclesPosition {
    fn position_id(&self) -> PositionId {
        self.position_id
    }    
    fn positor(&self) -> Principal {
        self.positor
    }
    type PayoutData = CyclesPayoutData;
    fn payout_data(&self) -> &Self::PayoutData {
        &self.cycles_payout_data
    }
    fn payout_data_mut(&mut self) -> &mut Self::PayoutData {
        &mut self.cycles_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.cycles_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.cycles_payout_data.is_complete() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData {
        &self.update_storage_position_data
    }
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData {
        &mut self.update_storage_position_data
    }
}


impl CyclesPayoutTrait for VoidCyclesPosition {
    fn cycles_payout_data(&self) -> CyclesPayoutData {
        self.cycles_payout_data.clone()
    }
    fn cycles_payout_lock(&self) -> bool { self.cycles_payout_lock }
    fn cycles_payout_payee(&self) -> Principal {
        self.positor
    }
    fn cycles_payout_payee_method(&self) -> &'static str {
        CM_MESSAGE_METHOD_VOID_CYCLES_POSITION_POSITOR
    } 
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMVoidCyclesPositionPositorMessageQuest{
                position_id: self.position_id,
                timestamp_nanos: self.timestamp_nanos,
            }
        )
    }
    fn cycles(&self) -> Cycles {
        self.cycles
    }
    fn cycles_payout_fee(&self) -> Cycles {
        0
    }
}



// --------



#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct VoidTokenPosition {
    pub position_id: PositionId,
    pub tokens: Tokens,
    pub positor: Principal,
    pub token_payout_lock: bool,  // lock for the payout
    pub token_payout_data: TokenPayoutData,
    pub timestamp_nanos: u128,
    pub update_storage_position_data: VPUpdateStoragePositionData,    
}

impl VoidPositionTrait for VoidTokenPosition {
    fn position_id(&self) -> PositionId {
        self.position_id
    }    
    fn positor(&self) -> Principal {
        self.positor
    }    
    type PayoutData = TokenPayoutData;
    fn payout_data(&self) -> &Self::PayoutData {
        &self.token_payout_data
    }
    fn payout_data_mut(&mut self) -> &mut Self::PayoutData {
        &mut self.token_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.token_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.token_payout_data.is_complete() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData {
        &self.update_storage_position_data
    }
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData {
        &mut self.update_storage_position_data
    }
}

impl TokenPayoutTrait for VoidTokenPosition {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
    fn token_payout_lock(&self) -> bool { self.token_payout_lock }
    fn token_payout_payee(&self) -> Principal { self.positor }
    fn token_payout_payor(&self) -> Principal { self.positor }
    fn tokens(&self) -> Tokens { self.tokens }
    fn token_transfer_memo(&self) -> Option<IcrcMemo> { Some(IcrcMemo(ByteBuf::from(create_void_token_position_transfer_memo(self.position_id)))) }
    fn token_fee_collection_transfer_memo(&self) -> Option<IcrcMemo> { None }
    fn token_ledger_transfer_fee(&self) -> Tokens { localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) }
    fn tokens_payout_fee(&self) -> Tokens { 0 } 
}


