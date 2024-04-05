// keep 1 minute segments forever. bout 57 MiB per year

// when making this into 1-minute candlesticks, use the time_nanos as the id for an optional_start_before_id parameter since time_nanos are 1 minute between.  

use crate::*;
use cts_lib::consts::{SECONDS_IN_A_MINUTE, SECONDS_IN_A_DAY};


const MAX_CANDLES_SPONSE: usize = (MiB as usize * 1 + KiB as usize * 512) / std::mem::size_of::<Candle>(); 


#[derive(Default, CandidType, Serialize, Deserialize)]
pub struct CandleCounter {
    segments_1_minute: Vec<Candle>,   // last item is the latest_one_minute
    volume_cycles: Cycles,            // all-time
    volume_tokens: Tokens,            // all-time
}

#[derive(Default, CandidType, Serialize, Deserialize)]
pub struct OldCandleCounter {
    latest_1_minute: Candle, 
    segments_1_minute: Vec<Candle>,
    volume_cycles: Cycles,            // all-time
    volume_tokens: Tokens,            // all-time
}

impl CandleCounter {
    pub fn count_trade(&mut self, tl: &TradeLog) {
        let current_segment_start_time_nanos = segment_start_time_nanos(tl.timestamp_nanos as u64);  /*good for the next 500 years. change when nanos goes over u64::max*/
        
        if self.segments_1_minute.len() == 0 || self.segments_1_minute.last().unwrap().time_nanos < current_segment_start_time_nanos {
            self.segments_1_minute.push(
                Candle{
                    time_nanos: current_segment_start_time_nanos,
                    volume_cycles: tl.cycles,
                    volume_tokens: tl.tokens,
                    open_rate: tl.cycles_per_token_rate,
                    high_rate: tl.cycles_per_token_rate,
                    low_rate: tl.cycles_per_token_rate,
                    close_rate: tl.cycles_per_token_rate,
                }
            );
        } else {
            let current_candle: &mut Candle = self.segments_1_minute.last_mut().unwrap();
            current_candle.volume_cycles = current_candle.volume_cycles.saturating_add(tl.cycles);
            current_candle.volume_tokens = current_candle.volume_tokens.saturating_add(tl.tokens);
            current_candle.high_rate = std::cmp::max(current_candle.high_rate, tl.cycles_per_token_rate);
            current_candle.low_rate = std::cmp::min(current_candle.low_rate, tl.cycles_per_token_rate);
            current_candle.close_rate = tl.cycles_per_token_rate;
        }

        self.volume_cycles = self.volume_cycles.saturating_add(tl.cycles);
        self.volume_tokens = self.volume_tokens.saturating_add(tl.tokens);
    }
    
}

// one-minute segments
fn segment_start_time_nanos(time_nanos: u64) -> u64 {
    time_nanos.saturating_sub(time_nanos % (NANOS_IN_A_SECOND as u64 * SECONDS_IN_A_MINUTE as u64))
}

pub fn create_candles<'a>(candle_counter: &'a CandleCounter, q: ViewCandlesQuest) -> ViewCandlesSponse {
        
    let mut s = &candle_counter.segments_1_minute[..];
    
    if let Some(start_before_time_nanos) = q.opt_start_before_time_nanos {
        s = &s[..s.binary_search_by_key(&segment_start_time_nanos(start_before_time_nanos), |c| { c.time_nanos }).unwrap_or_else(|e| e)];    
    }
    
    if s.len() == 0 {
        return ViewCandlesSponse{
            candles: &[],
            is_earliest_chunk: true
        };
    }
    
    let mut s_rchunks = s.rchunks(MAX_CANDLES_SPONSE);
    let chunk = s_rchunks.next().unwrap();
    
    ViewCandlesSponse{
        candles: chunk,
        is_earliest_chunk: s_rchunks.next().is_none()
    }    
}
    

#[derive(CandidType, Deserialize)]
pub struct ViewVolumeStatsSponse {
    volume_cycles: Volume,
    volume_tokens: Volume,
}
#[derive(CandidType, Deserialize)]
pub struct Volume{
    volume_24_hour: u128,
    volume_7_day: u128,
    volume_30_day: u128,
    volume_sum: u128,
}



pub fn create_view_volume_stats(candle_counter: &CandleCounter) -> ViewVolumeStatsSponse {
    
    let h = |timeframe_length_nanos: u128| {
        let timeframe_start_nanos = time_nanos_u64().saturating_sub(timeframe_length_nanos as u64);
            
        candle_counter.segments_1_minute[
            candle_counter.segments_1_minute.binary_search_by_key(&timeframe_start_nanos, |c| c.time_nanos).unwrap_or_else(|e| e)            
            ..
        ]
        .iter()
        .fold((0u128, 0u128), |(count_cycles, count_tokens), c| {
            (count_cycles.saturating_add(c.volume_cycles), count_tokens.saturating_add(c.volume_tokens))            
        })
    };
    
    let (vc_24_hour, vt_24_hour) = h(NANOS_IN_A_SECOND * SECONDS_IN_A_DAY * 1); 
    let (vc_7_day,   vt_7_day)   = h(NANOS_IN_A_SECOND * SECONDS_IN_A_DAY * 7);
    let (vc_30_day,  vt_30_day)  = h(NANOS_IN_A_SECOND * SECONDS_IN_A_DAY * 30); 
    
    ViewVolumeStatsSponse {
        volume_cycles: Volume{
            volume_24_hour: vc_24_hour,
            volume_7_day: vc_7_day,
            volume_30_day: vc_30_day,
            volume_sum: candle_counter.volume_cycles,
        },
        volume_tokens: Volume{
            volume_24_hour: vt_24_hour,
            volume_7_day: vt_7_day,
            volume_30_day: vt_30_day,
            volume_sum: candle_counter.volume_tokens,
        },
    }    
}













