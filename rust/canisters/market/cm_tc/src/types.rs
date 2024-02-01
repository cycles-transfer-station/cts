use std::thread::LocalKey;
use std::cell::RefCell;
use crate::*;
use cts_lib::icrc::IcrcSubaccount;
use serde::Serialize;
pub use cts_lib::types::cm::tc::storage_logs::{*, position_log::PositionLog, trade_log::{TradeLog, PayoutData}};

// -------

pub type VoidCyclesPositionId = PositionId;
pub type VoidTokenPositionId = PositionId;

// -------

pub trait LocalKeyRefCellLogStorageDataTrait {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>>;    
}

impl LocalKeyRefCellLogStorageDataTrait for PositionLog {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>> = &POSITIONS_STORAGE_DATA;
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

    fn return_to_subaccount(&self) -> Option<IcrcSubaccount>;
    fn payout_to_subaccount(&self) -> Option<IcrcSubaccount>;
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct CyclesPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub quest: TradeCyclesQuest,
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
            cycles_payout_data: None,
            timestamp_nanos: time_nanos(),
            update_storage_position_data: VPUpdateStoragePositionData{
                lock: false,
                status: false,
                update_storage_position_log: self.as_stable_memory_position_log(Some(position_termination_cause))
            },
            return_cycles_to_subaccount: self.quest.return_cycles_to_subaccount,
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
            void_position_payout_ledger_transfer_fee: 0, // this field is not used for the cycles-positions.
        }
    }
    
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.return_cycles_to_subaccount.clone()
    }
    fn payout_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.payout_tokens_to_subaccount.clone()
    }
}

#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TokenPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub quest: TradeTokensQuest,
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
            token_payout_data: None,
            update_storage_position_data: VPUpdateStoragePositionData{
                status: false,
                lock: false,
                update_storage_position_log: self.as_stable_memory_position_log(Some(position_termination_cause))   
            },
            return_tokens_to_subaccount: self.quest.return_tokens_to_subaccount,
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
            void_position_payout_ledger_transfer_fee: 0, // this field is update when a void-token-position-payout is done.                    
        }
    }
    
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.return_tokens_to_subaccount.clone()
    }
    fn payout_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.payout_cycles_to_subaccount.clone()
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

// -----------------


impl LocalKeyRefCellLogStorageDataTrait for TradeLog {
    const LOG_STORAGE_DATA: &'static LocalKey<RefCell<LogStorageData>> = &TRADES_STORAGE_DATA;
}


#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TradeLogTemporaryData {
    pub cycles_payout_lock: bool,
    pub token_payout_lock: bool,
    pub payout_cycles_to_subaccount: Option<IcrcSubaccount>,
    pub payout_tokens_to_subaccount: Option<IcrcSubaccount>,
}

#[derive(Clone, CandidType, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TradeLogAndTemporaryData {
    pub log: TradeLog,
    pub temporary_data: TradeLogTemporaryData,
}

impl TradeLogAndTemporaryData {
    pub fn can_move_into_the_stable_memory_for_the_long_term_storage(&self) -> bool {
        self.temporary_data.cycles_payout_lock == false
        && self.temporary_data.token_payout_lock == false
        && self.log.cycles_payout_data.is_some()
        && self.log.token_payout_data.is_some()
    }
}

// --------

pub trait VoidPositionTrait: Clone {
    fn position_id(&self) -> PositionId;
    fn positor(&self) -> Principal;    
    fn quantity(&self) -> u128;
    fn payout_data(&self) -> &Option<PayoutData>;
    fn payout_data_mut(&mut self) -> &mut Option<PayoutData>;
    fn payout_lock(&mut self) -> &mut bool;
    fn can_remove(&self) -> bool;
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData;
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData;
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount>;
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
    pub cycles_payout_data: Option<PayoutData>,
    pub timestamp_nanos: u128,
    pub update_storage_position_data: VPUpdateStoragePositionData,
    pub return_cycles_to_subaccount: Option<IcrcSubaccount>,
}

impl VoidPositionTrait for VoidCyclesPosition {
    fn position_id(&self) -> PositionId {
        self.position_id
    }    
    fn positor(&self) -> Principal {
        self.positor
    }
    fn quantity(&self) -> u128 {
        self.cycles
    }
    fn payout_data(&self) -> &Option<PayoutData> {
        &self.cycles_payout_data
    }
    fn payout_data_mut(&mut self) -> &mut Option<PayoutData> {
        &mut self.cycles_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.cycles_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.cycles_payout_data.is_some() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData {
        &self.update_storage_position_data
    }
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData {
        &mut self.update_storage_position_data
    }
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.return_cycles_to_subaccount.clone()
    }
}

// --------

#[derive(CandidType, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct VoidTokenPosition {
    pub position_id: PositionId,
    pub tokens: Tokens,
    pub positor: Principal,
    pub token_payout_lock: bool,  // lock for the payout
    pub token_payout_data: Option<PayoutData>,
    pub timestamp_nanos: u128,
    pub update_storage_position_data: VPUpdateStoragePositionData,    
    pub return_tokens_to_subaccount: Option<IcrcSubaccount>,
}

impl VoidPositionTrait for VoidTokenPosition {
    fn position_id(&self) -> PositionId {
        self.position_id
    }    
    fn positor(&self) -> Principal {
        self.positor
    }    
    fn quantity(&self) -> u128 {
        self.tokens
    }
    fn payout_data(&self) -> &Option<PayoutData> {
        &self.token_payout_data
    }
    fn payout_data_mut(&mut self) -> &mut Option<PayoutData> {
        &mut self.token_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.token_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.token_payout_data.is_some() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData {
        &self.update_storage_position_data
    }
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData {
        &mut self.update_storage_position_data
    }
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.return_tokens_to_subaccount.clone()
    }
}
