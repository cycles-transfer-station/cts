use core::future::Future;
use std::collections::{BTreeMap, HashSet};
use cts_lib::{
    icrc::{
        Icrc1TransferQuest,
    },
    types::{
        cm::tc::{
            PositionId,
            CMData,
            CyclesPerToken,
            TradeCyclesQuest,
            TradeTokensQuest,
            CyclesPosition,
            TokenPosition,

        }
    },
    tools::{
        cycles_transform_tokens,
        tokens_transform_cycles,
        time_nanos,
    }
};
use super::CurrentPositionTrait;
use crate::{
    ledger_transfer::{
        LedgerTransferReturnType,
        cycles_transfer,
        token_transfer,
    },
    MAX_CYCLES_POSITIONS,
    MAX_TOKEN_POSITIONS,
    MAX_VOID_CYCLES_POSITIONS,
    MAX_VOID_TOKEN_POSITIONS,
    minimum_cycles_match,
    minimum_tokens_match,
};
use candid::Principal;



pub trait TradeQuest {
    
    type MatcherPositionType: CurrentPositionTrait;
    type MatcheePositionType: CurrentPositionTrait;
    
    const MAX_POSITIONS: usize;
    const MAX_VOID_POSITIONS: usize;
    
    fn quantity(&self) -> u128;
    fn cycles_per_token_rate(&self) -> CyclesPerToken;
    fn posit_transfer_ledger_fee(&self) -> Option<u128>;
    fn is_less_than_minimum_position(&self) -> bool;
    fn mid_call_balance_locks(cm_data: &mut CMData) -> &mut HashSet<Principal>;
    fn posit_transfer(q: Icrc1TransferQuest) -> impl Future<Output=LedgerTransferReturnType>;
    fn create_current_position(self, id: PositionId, positor: Principal) -> Self::MatcherPositionType;
    fn matcher_positions(cm_data: &mut CMData) -> &mut BTreeMap<PositionId, Self::MatcherPositionType>;
    fn matcher_void_positions(cm_data: &mut CMData) -> &mut BTreeMap<PositionId, <Self::MatcherPositionType as CurrentPositionTrait>::VoidPositionType>;
    fn match_trades(cm_data: &mut CMData, matcher_position_id: PositionId);
} 


impl TradeQuest for TradeCyclesQuest {
    type MatcherPositionType = CyclesPosition;
    type MatcheePositionType = TokenPosition;
    
    const MAX_POSITIONS: usize = MAX_CYCLES_POSITIONS;
    const MAX_VOID_POSITIONS: usize = MAX_VOID_CYCLES_POSITIONS;
    
    fn quantity(&self) -> u128 { self.cycles }
    fn cycles_per_token_rate(&self) -> CyclesPerToken { self.cycles_per_token_rate }
    fn posit_transfer_ledger_fee(&self) -> Option<u128> { self.posit_transfer_ledger_fee }
    fn is_less_than_minimum_position(&self) -> bool {
        self.cycles < minimum_cycles_match() || cycles_transform_tokens(self.cycles, self.cycles_per_token_rate) < minimum_tokens_match() 
    }
    fn mid_call_balance_locks(cm_data: &mut CMData) -> &mut HashSet<Principal> { &mut cm_data.mid_call_user_cycles_balance_locks }
    fn posit_transfer(q: Icrc1TransferQuest) -> impl Future<Output=LedgerTransferReturnType> { cycles_transfer(q) }
    fn create_current_position(self, id: PositionId, positor: Principal) -> Self::MatcherPositionType {
        CyclesPosition{
            id,
            positor,
            current_position_cycles: self.cycles,
            quest: self,
            purchases_rates_times_cycles_quantities_sum: 0,
            fill_quantity_tokens: 0,
            tokens_payouts_fees_sum: 0,
            timestamp_nanos: time_nanos(),
        }
    }
    fn matcher_positions(cm_data: &mut CMData) -> &mut BTreeMap<PositionId, Self::MatcherPositionType> { &mut cm_data.cycles_positions }    
    fn matcher_void_positions(cm_data: &mut CMData) -> &mut BTreeMap<PositionId, <Self::MatcherPositionType as CurrentPositionTrait>::VoidPositionType> { &mut cm_data.void_cycles_positions }
    fn match_trades(cm_data: &mut CMData, matcher_position_id: PositionId) {
        crate::match_trades(
            matcher_position_id,
            &mut cm_data.cycles_positions,
            &mut cm_data.token_positions,
            &mut cm_data.void_cycles_positions,
            &mut cm_data.void_token_positions,
            &mut cm_data.trade_logs,
            &mut cm_data.trade_logs_id_counter,
            &mut cm_data.candle_counter,  
        );
    }
}

impl TradeQuest for TradeTokensQuest {
    type MatcherPositionType = TokenPosition;
    type MatcheePositionType = CyclesPosition;
    
    const MAX_POSITIONS: usize = MAX_TOKEN_POSITIONS;
    const MAX_VOID_POSITIONS: usize = MAX_VOID_TOKEN_POSITIONS;
    
    fn quantity(&self) -> u128 { self.tokens }
    fn cycles_per_token_rate(&self) -> CyclesPerToken { self.cycles_per_token_rate }
    fn posit_transfer_ledger_fee(&self) -> Option<u128> { self.posit_transfer_ledger_fee }
    fn is_less_than_minimum_position(&self) -> bool {
        self.tokens < minimum_tokens_match() || tokens_transform_cycles(self.tokens, self.cycles_per_token_rate) < minimum_cycles_match()
    }
    fn mid_call_balance_locks(cm_data: &mut CMData) -> &mut HashSet<Principal> { &mut cm_data.mid_call_user_token_balance_locks }
    fn posit_transfer(q: Icrc1TransferQuest) -> impl Future<Output=LedgerTransferReturnType> { token_transfer(q) }
    fn create_current_position(self, id: PositionId, positor: Principal) -> Self::MatcherPositionType {
        TokenPosition{
            id,
            positor,
            current_position_tokens: self.tokens,
            quest: self,
            purchases_rates_times_token_quantities_sum: 0,
            cycles_payouts_fees_sum: 0,
            timestamp_nanos: time_nanos(),                
        }
    }
    fn matcher_positions(cm_data: &mut CMData) -> &mut BTreeMap<PositionId, Self::MatcherPositionType> { &mut cm_data.token_positions }
    fn matcher_void_positions(cm_data: &mut CMData) -> &mut BTreeMap<PositionId, <Self::MatcherPositionType as CurrentPositionTrait>::VoidPositionType> { &mut cm_data.void_token_positions }     
    fn match_trades(cm_data: &mut CMData, matcher_position_id: PositionId) {
        crate::match_trades(
            matcher_position_id,
            &mut cm_data.token_positions,
            &mut cm_data.cycles_positions,
            &mut cm_data.void_token_positions,
            &mut cm_data.void_cycles_positions,
            &mut cm_data.trade_logs,
            &mut cm_data.trade_logs_id_counter,
            &mut cm_data.candle_counter,  
        );
    }

}


