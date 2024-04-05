use super::*;
use cts_lib::types::cm::tc::*;




pub fn view_candles(pic: &PocketIc, tc: Principal, q: ViewCandlesQuest) -> ViewCandlesSponseOwned {
    call_candid_::<_, (ViewCandlesSponseOwned,)>(&pic, tc, "view_candles", (q,))
    .unwrap().0
}
