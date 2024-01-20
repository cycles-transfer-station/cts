use super::*;

struct TradeFeeTier {
    // the max volume (in-clusive) of the trade fees of this tier. anything over this amount is the next tier
    volume_tcycles: u128,
    trade_fee_ten_thousandths: u128,   
}
impl TradeFeeTier {
    fn volume_cycles(&self) -> Cycles {
        self.volume_tcycles.saturating_mul(TRILLION)
    }
}

#[allow(non_upper_case_globals)]
const trade_fees_tiers: &[TradeFeeTier; 5] = &[
    TradeFeeTier{
        volume_tcycles: 100_000,
        trade_fee_ten_thousandths: 50,
    },
    TradeFeeTier{
        volume_tcycles: 500_000,
        trade_fee_ten_thousandths: 30,
    },
    TradeFeeTier{
        volume_tcycles: 1_000_000,
        trade_fee_ten_thousandths: 10,
    },
    TradeFeeTier{
        volume_tcycles: 5_000_000,
        trade_fee_ten_thousandths: 5,
    },
    TradeFeeTier{
        volume_tcycles: u128::MAX,
        trade_fee_ten_thousandths: 1,
    },
]; 

pub fn calculate_trade_fee(current_position_trade_volume_cycles: Cycles, trade_cycles: Cycles) -> Cycles/*fee-cycles*/ {    
    let mut trade_cycles_mainder: Cycles = trade_cycles;
    let mut fee_cycles: Cycles = 0;
    for i in 0..trade_fees_tiers.len() {
        if current_position_trade_volume_cycles + trade_cycles - trade_cycles_mainder + 1/*plus one for start with the fee tier for the current-trade-mount*/ 
        <= trade_fees_tiers[i].volume_cycles() {
            let trade_cycles_in_the_current_tier: Cycles = std::cmp::min(
                trade_cycles_mainder,
                trade_fees_tiers[i].volume_cycles().saturating_sub(current_position_trade_volume_cycles + trade_cycles - trade_cycles_mainder), 
            );
            trade_cycles_mainder -= trade_cycles_in_the_current_tier;
            fee_cycles += trade_cycles_in_the_current_tier / 10_000 * trade_fees_tiers[i].trade_fee_ten_thousandths; 
            
            if trade_cycles_mainder == 0 {
                break;
            }
        } 
    } 
    
    fee_cycles
}
