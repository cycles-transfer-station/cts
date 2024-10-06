use super::*;
use cts_lib::types::cm::tc::*;




pub fn view_candles(pic: &PocketIc, tc: Principal, q: ViewCandlesQuest) -> ViewCandlesSponseOwned {
    call_candid_::<_, (ViewCandlesSponseOwned,)>(&pic, tc, "view_candles", (q,))
    .unwrap().0
}

pub fn call_trade_cycles(pic: &PocketIc, tc: Principal, caller: Principal, q: &TradeCyclesQuest) -> TradeResult {
    call_candid_as_::<_, (TradeResult,)>(&pic, tc, caller, "trade_cycles", (q,)).unwrap().0
}

pub fn call_trade_tokens(pic: &PocketIc, tc: Principal, caller: Principal, q: &TradeTokensQuest) -> TradeResult {
    call_candid_as_::<_, (TradeResult,)>(&pic, tc, caller, "trade_tokens", (q,)).unwrap().0 
}