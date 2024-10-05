use cts_lib::{
    icrc::{
        Tokens,
        IcrcSubaccount,
    },
    tools::{
        time_nanos,
        tokens_transform_cycles,
        cycles_transform_tokens,
    },
    types::{
        Cycles,
        cm::tc::{
            PositionId,
            CyclesPerToken,
            PositionKind,
            CyclesPosition,
            TokenPosition,
            VoidCyclesPosition,
            VoidTokenPosition,
            VPUpdateStoragePositionData,
            storage_logs::{
                position_log::{
                    PositionLog,
                    PositionTerminationCause,
                    PositionTerminationData,
                }
            },
            trade_fee::calculate_trade_fee,
        }
    },
};
use candid::Principal;
use super::VoidPositionTrait;



pub trait CurrentPositionTrait {
    fn id(&self) -> PositionId;
    fn positor(&self) -> Principal;
    fn current_position_available_cycles_per_token_rate(&self) -> CyclesPerToken;
    //fn timestamp_nanos(&self) -> u128;

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

    //fn return_to_subaccount(&self) -> Option<IcrcSubaccount>;
    fn payout_to_subaccount(&self) -> Option<IcrcSubaccount>;
}


impl CurrentPositionTrait for CyclesPosition {
    fn id(&self) -> PositionId { self.id }
    fn positor(&self) -> Principal { self.positor }
    fn current_position_available_cycles_per_token_rate(&self) -> CyclesPerToken {
        self.quest.cycles_per_token_rate
    }
    //fn timestamp_nanos(&self) -> u128 { self.timestamp_nanos }

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
            void_position_payout_ledger_transfer_fee: 0,
        }
    }
    /*
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.return_cycles_to_subaccount.clone()
    }
    */
    fn payout_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.payout_tokens_to_subaccount.clone()
    }
}


impl CurrentPositionTrait for TokenPosition {
    fn id(&self) -> PositionId { self.id }
    fn positor(&self) -> Principal { self.positor }
    fn current_position_available_cycles_per_token_rate(&self) -> CyclesPerToken {
        self.quest.cycles_per_token_rate
    }
    //fn timestamp_nanos(&self) -> u128 { self.timestamp_nanos }

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
            void_position_payout_ledger_transfer_fee: 0, // this field is update when a void-position-payout is done.
        }
    }
    /*
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.return_tokens_to_subaccount.clone()
    }
    */
    fn payout_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.quest.payout_cycles_to_subaccount.clone()
    }
}
