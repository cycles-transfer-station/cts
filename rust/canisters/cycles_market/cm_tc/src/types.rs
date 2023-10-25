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


// this one goes into the PositionLog storage but gets updated for the position-termination.
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
            s[162] = data.cause.ser();
        }        
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

pub struct PositionTerminationData {
    pub timestamp_nanos: u128,
    pub cause: PositionTerminationCause
}

pub enum PositionTerminationCause {
    Fill, // the position is fill[ed]. position.amount < minimum_token_match()
    Bump, // the position got bumped
    TimePass, // expired
    UserCallVoidPosition, // the user cancelled the position by calling void_position
}
impl PositionTerminationCause {
    pub fn ser(&self) -> u8 {
        match self {
            PositionTerminationCause::Fill => 0,
            PositionTerminationCause::Bump => 1,
            PositionTerminationCause::TimePass => 2,
            PositionTerminationCause::UserCallVoidPosition => 3
        }
    }
}





// ------------------


pub trait CurrentPositionTrait {
    fn id(&self) -> PositionId;
    fn positor(&self) -> Principal;
    fn current_position_available_cycles_per_token_rate(&self) -> CyclesPerToken;
    fn timestamp_nanos(&self) -> u128;
    
    type VoidPositionType: VoidPositionTrait;
    fn into_void_position_type(self, position_termination_cause: Option<PositionTerminationCause>) -> Self::VoidPositionType;
    
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
    fn into_void_position_type(self, position_termination_cause: Option<PositionTerminationCause>) -> Self::VoidPositionType {
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
                update_storage_position_log_serialization_b: self.as_stable_memory_position_log(position_termination_cause).stable_memory_serialize()    
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
        }
    }


}




#[derive(CandidType, Serialize, Deserialize)]
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
    fn into_void_position_type(self, position_termination_cause: Option<PositionTerminationCause>) -> Self::VoidPositionType {
        VoidTokenPosition{
            position_id: self.id,
            positor: self.positor,
            tokens: self.current_position_tokens,
            timestamp_nanos: time_nanos(),
            token_payout_lock: false,
            token_payout_data: TokenPayoutData{
                token_transfer: if self.current_position_tokens < localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) {
                    Some(TokenTransferData{
                        block_height: None,
                        timestamp_nanos: time_nanos(),
                        ledger_transfer_fee: 0,
                    })                
                } else { None },
                token_fee_collection: Some(TokenTransferData{
                    block_height: None,
                    timestamp_nanos: time_nanos(),
                    ledger_transfer_fee: 0,
                }),
                cm_message_call: None,
            },
            update_storage_position_data: VPUpdateStoragePositionData{
                status: false,
                lock: false,
                update_storage_position_log_serialization_b: self.as_stable_memory_position_log(position_termination_cause).stable_memory_serialize()
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
}


#[derive(Clone, Serialize, Deserialize)]
pub struct CyclesPayoutData {
    pub cycles_payout: bool,
}
impl CyclesPayoutData {
    pub fn new() -> Self {
        Self {
            cycles_payout: false,
        }
    }
}
impl PayoutDataTrait for CyclesPayoutData {
    fn is_complete(&self) -> bool {
        self.cycles_payout 
    }
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



#[derive(Clone, Serialize, Deserialize)]
pub struct TokenTransferData {
    pub block_height: Option<BlockId>, // if None that means there was no transfer in this token-payout. it is an unlock of the funds within the same-token-id.
    pub timestamp_nanos: u128,
    pub ledger_transfer_fee: Tokens,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TokenPayoutData {
    pub token_transfer: Option<TokenTransferData>,
    pub token_fee_collection: Option<TokenTransferData>,
    pub cm_message_call: Option<Option<(u32, String)>>, // first option for the callback-completion, second option for the possible-message-call-error
}
impl TokenPayoutData {
    // separate new fns because a void_token_position.token_payout_data must start with the token_transfer = Some(TokenTransferData)
    pub fn new_for_a_trade_log() -> Self {
        Self{
            token_transfer: None,
            token_fee_collection: None,
            cm_message_call: None,
        }
    }
}
impl PayoutDataTrait for TokenPayoutData {
    fn is_complete(&self) -> bool {
        self.cm_message_call.is_some()
    }    
}

pub trait TokenPayoutTrait {
    fn token_payout_data(&self) -> TokenPayoutData;
    fn token_payout_lock(&self) -> bool;
    fn token_payout_payee(&self) -> Principal;
    fn token_payout_payor(&self) -> Principal;
    fn token_payout_payee_method(&self) -> &'static str;
    fn token_payout_payee_method_quest_bytes(&self, token_payout_data_token_transfer: TokenTransferData) -> Result<Vec<u8>, CandidError>; 
    fn tokens(&self) -> Tokens;
    fn token_transfer_memo(&self) -> Option<IcrcMemo>;
    fn token_fee_collection_transfer_memo(&self) -> Option<IcrcMemo>;
    fn token_ledger_transfer_fee(&self) -> Tokens;
    fn tokens_payout_fee(&self) -> Tokens;
}



// -----------------


#[derive(Clone, Serialize, Deserialize)]
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
        match self.matchee_position_kind { 
            PositionKind::Cycles => CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_PURCHASER,
            PositionKind::Token => CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_POSITOR,
        } 
    }
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
        match self.matchee_position_kind { 
            PositionKind::Cycles => {
                encode_one(
                    CMCyclesPositionPurchasePurchaserMessageQuest {
                        cycles_position_id: self.position_id_matchee,
                        cycles_position_positor: self.matchee_position_positor,
                        cycles_position_cycles_per_token_rate: self.cycles_per_token_rate,
                        purchase_id: self.id,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                        token_payment: self.tokens,
                    }
                ) 
            }
            PositionKind::Token => {
                encode_one(
                    CMTokenPositionPurchasePositorMessageQuest{
                        token_position_id: self.position_id_matchee,
                        token_position_cycles_per_token_rate: self.cycles_per_token_rate,
                        purchase_id: self.id,
                        purchaser: self.matcher_position_positor,
                        token_purchase: self.tokens,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                    }
                )
            }
        }
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
    fn token_payout_payee_method(&self) -> &'static str { 
        match self.matchee_position_kind { 
            PositionKind::Cycles => CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR,
            PositionKind::Token => CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_PURCHASER,
        }
    }
    fn token_payout_payee_method_quest_bytes(&self, token_payout_data_token_transfer: TokenTransferData) -> Result<Vec<u8>, CandidError> {
        match self.matchee_position_kind { 
            PositionKind::Cycles => {
                encode_one(
                    CMCyclesPositionPurchasePositorMessageQuest {
                        cycles_position_id: self.position_id_matchee,
                        purchase_id: self.id,
                        purchaser: self.matcher_position_positor,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                        cycles_purchase: self.cycles,
                        cycles_position_cycles_per_token_rate: self.cycles_per_token_rate,
                        token_payment: self.tokens,
                        token_transfer_block_height: token_payout_data_token_transfer.block_height.unwrap(), 
                        token_transfer_timestamp_nanos: token_payout_data_token_transfer.timestamp_nanos,
                    }    
                )
            }
            PositionKind::Token => {
                encode_one(
                    CMTokenPositionPurchasePurchaserMessageQuest {
                        token_position_id: self.position_id_matchee,
                        purchase_id: self.id, 
                        positor: self.matchee_position_positor,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                        token_purchase: self.tokens,
                        token_position_cycles_per_token_rate: self.cycles_per_token_rate,
                        cycles_payment: self.cycles,
                        token_transfer_block_height: token_payout_data_token_transfer.block_height.unwrap(),
                        token_transfer_timestamp_nanos: token_payout_data_token_transfer.timestamp_nanos,
                    }
                )
            }
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
    type PayoutData: PayoutDataTrait;
    fn payout_data(&mut self) -> &mut Self::PayoutData;
    fn payout_lock(&mut self) -> &mut bool;
    fn can_remove(&self) -> bool;
    fn update_storage_position_data(&mut self) -> &mut VPUpdateStoragePositionData;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VPUpdateStoragePositionData {
    pub lock: bool,
    pub status: bool,
    pub update_storage_position_log_serialization_b: Vec<u8>// const generics [u8; PositionLog::STABLE_MEMORY_SERIALIZE_SIZE],
}

#[derive(Clone, Serialize, Deserialize)]
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
    type PayoutData = CyclesPayoutData;
    fn payout_data(&mut self) -> &mut Self::PayoutData {
        &mut self.cycles_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.cycles_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.cycles_payout_data.is_complete() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&mut self) -> &mut VPUpdateStoragePositionData {
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



#[derive(Serialize, Deserialize, Clone)]
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
    type PayoutData = TokenPayoutData;
    fn payout_data(&mut self) -> &mut Self::PayoutData {
        &mut self.token_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.token_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.token_payout_data.is_complete() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&mut self) -> &mut VPUpdateStoragePositionData {
        &mut self.update_storage_position_data
    }
}

impl TokenPayoutTrait for VoidTokenPosition {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
    fn token_payout_lock(&self) -> bool { self.token_payout_lock }
    fn token_payout_payee(&self) -> Principal { self.positor }
    fn token_payout_payor(&self) -> Principal { self.positor }
    fn token_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_VOID_TOKEN_POSITION_POSITOR }
    fn token_payout_payee_method_quest_bytes(&self, _token_payout_data_token_transfer: TokenTransferData) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMVoidTokenPositionPositorMessageQuest {
                position_id: self.position_id,
                void_tokens: self.tokens(),
                timestamp_nanos: self.timestamp_nanos
            }
        )
    }
    fn tokens(&self) -> Tokens { self.tokens }
    fn token_transfer_memo(&self) -> Option<IcrcMemo> { Some(Vec::from(*b"vtp").into()) }
    fn token_fee_collection_transfer_memo(&self) -> Option<IcrcMemo> { None }
    fn token_ledger_transfer_fee(&self) -> Tokens { localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) }
    fn tokens_payout_fee(&self) -> Tokens { 0 } 
}


