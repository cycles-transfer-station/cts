use pic_tools::{*, bank::mint_cycles, tc::*};
use candid::Principal;
use cts_lib::{
    types::{
        bank::BANK_TRANSFER_FEE,
        cm::tc::{
            *,
            storage_logs::{*, position_log::*, trade_log::*},
            
        },
    },
    tools::{
        principal_token_subaccount,
        tokens_transform_cycles,
        cycles_transform_tokens,
    },
    consts::{NANOS_IN_A_SECOND, SECONDS_IN_A_MINUTE},
};
use icrc_ledger_types::icrc1::account::Account;
use core::time::Duration;


#[test]
fn test_1() {
    let pic = set_up();
    let tc = set_up_tc(&pic);
    
    let (p1,p2): (Principal,Principal) = (
        Principal::from_slice(&[1,1,1,1,1]),
        Principal::from_slice(&[2,2,2,2,2]),
    );
    
    let p1_trade_icp = 10000000000;
    let trade_rate = 77777;
    mint_icp(&pic, &Account{owner: tc, subaccount: Some(principal_token_subaccount(&p1))}, p1_trade_icp + ICP_LEDGER_TRANSFER_FEE);
    
    let p1_position_id = call_candid_as_::<_, (TradeResult,)>(&pic, tc, p1, "trade_tokens", (
        TradeTokensQuest{
            tokens: p1_trade_icp,
            cycles_per_token_rate: trade_rate,
            posit_transfer_ledger_fee: Some(ICP_LEDGER_TRANSFER_FEE),
            return_tokens_to_subaccount: None,
            payout_cycles_to_subaccount: None,
        },
    )).unwrap().0.unwrap().position_id;    
    assert_eq!(p1_position_id, 0);
    
    let p1_view_user_current_positions_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_user_current_positions",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p1)
        }).unwrap(),
    ).unwrap().unwrap();
    
    assert_eq!(p1_view_user_current_positions_sponse_b.len(), PositionLog::STABLE_MEMORY_SERIALIZE_SIZE);
    
    let p1_position = PositionLog::stable_memory_serialize_backwards(&p1_view_user_current_positions_sponse_b);
    
    assert_eq!(
        p1_position, 
        PositionLog{
            id: 0,
            positor: p1,
            quest: CreatePositionQuestLog{
                quantity: p1_trade_icp,
                cycles_per_token_rate: trade_rate,
            },
            position_kind: PositionKind::Token,
            mainder_position_quantity: p1_trade_icp,
            fill_quantity: 0,
            fill_average_rate: trade_rate,
            payouts_fees_sum: 0,
            creation_timestamp_nanos: pic_get_time_nanos(&pic),
            position_termination: None,
            void_position_payout_dust_collection: false,
            void_position_payout_ledger_transfer_fee: 0,
        }
    );
    
    let p2_mint_cycles = mint_cycles(&pic, &Account{owner: tc, subaccount: Some(principal_token_subaccount(&p2))}, 500000000);
    let p2_trade_cycles = p2_mint_cycles - BANK_TRANSFER_FEE;
    
    let p2_position_id = call_candid_as_::<_, (TradeResult,)>(&pic, tc, p2, "trade_cycles", (
        TradeCyclesQuest{
            cycles: p2_trade_cycles,
            cycles_per_token_rate: trade_rate,
            posit_transfer_ledger_fee: Some(BANK_TRANSFER_FEE),
            return_cycles_to_subaccount: None,
            payout_tokens_to_subaccount: None,
        },
    )).unwrap().0.unwrap().position_id;    
    assert_eq!(p2_position_id, 1);
    
    let p2_trade_cycles_timestamp_nanos = pic_get_time_nanos(&pic);    
    
    let p2_view_user_current_positions_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_user_current_positions",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p2)
        }).unwrap(),
    ).unwrap().unwrap();
    assert_eq!(p2_view_user_current_positions_sponse_b.len(), 0);
    
    // check pending trades, p1-current-position, p2-void-position-pending
    
    let p2_view_void_positions_pending_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_void_positions_pending",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p2)
        }).unwrap(),
    ).unwrap().unwrap();
    assert_eq!(p2_view_void_positions_pending_sponse_b.len(), PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1); 
    assert_eq!(
        PositionLog::stable_memory_serialize_backwards(&p2_view_void_positions_pending_sponse_b[..(p2_view_void_positions_pending_sponse_b.len() - 1)]),
        PositionLog{
            id: 1,
            positor: p2,
            quest: CreatePositionQuestLog{
                quantity: p2_trade_cycles,
                cycles_per_token_rate: trade_rate,
            },
            position_kind: PositionKind::Cycles,
            mainder_position_quantity: p2_trade_cycles % trade_rate,
            fill_quantity: p2_trade_cycles / trade_rate,
            fill_average_rate: trade_rate,
            payouts_fees_sum: (p2_trade_cycles / trade_rate) * trade_rate / 10_000 * 50 / trade_rate,
            creation_timestamp_nanos: p2_trade_cycles_timestamp_nanos,
            position_termination: Some(PositionTerminationData{
                cause: PositionTerminationCause::Fill,
                timestamp_nanos: pic_get_time_nanos(&pic)
            }),
            void_position_payout_dust_collection: false, // because the payout didn't run yet
            void_position_payout_ledger_transfer_fee: 0, // because the payout didn't run yet
        }
    );    
    assert_eq!(p2_view_void_positions_pending_sponse_b[p2_view_void_positions_pending_sponse_b.len()-1], 0); // payout did not run yet
    
    let p1_view_user_current_positions_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_user_current_positions",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p1)
        }).unwrap(),
    ).unwrap().unwrap();
    assert_eq!(p1_view_user_current_positions_sponse_b.len(), PositionLog::STABLE_MEMORY_SERIALIZE_SIZE);    
    assert_eq!(
        PositionLog::stable_memory_serialize_backwards(&p1_view_user_current_positions_sponse_b), 
        PositionLog{
            id: 0,
            positor: p1,
            quest: CreatePositionQuestLog{
                quantity: p1_trade_icp,
                cycles_per_token_rate: trade_rate,
            },
            position_kind: PositionKind::Token,
            mainder_position_quantity: p1_trade_icp - (p2_trade_cycles / trade_rate),
            fill_quantity: p2_trade_cycles - (p2_trade_cycles % trade_rate),
            fill_average_rate: trade_rate,
            payouts_fees_sum: (p2_trade_cycles - (p2_trade_cycles % trade_rate)) / 10_000 * 50,
            creation_timestamp_nanos: pic_get_time_nanos(&pic),
            position_termination: None,
            void_position_payout_dust_collection: false,
            void_position_payout_ledger_transfer_fee: 0,
        }
    );
    
    for index_key in [Some(0u128), Some(1u128), None::<u128>] {
        let view_position_pending_trades_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_position_pending_trades",
            candid::encode_one(ViewStorageLogsQuest{
                opt_start_before_id: None,
                index_key: index_key,
            }).unwrap(),
        ).unwrap().unwrap();
        assert_eq!(view_position_pending_trades_sponse_b.len(), TradeLog::STABLE_MEMORY_SERIALIZE_SIZE + 2);    
        assert_eq!(
            TradeLog::stable_memory_serialize_backwards(&view_position_pending_trades_sponse_b),
            TradeLog{
                position_id_matcher: 1,
                position_id_matchee: 0,
                id: 0,
                matchee_position_positor: p1,
                matcher_position_positor: p2,
                tokens: p2_trade_cycles / trade_rate,
                cycles: p2_trade_cycles - (p2_trade_cycles % trade_rate),
                cycles_per_token_rate: trade_rate,
                matchee_position_kind: PositionKind::Token,
                timestamp_nanos: pic_get_time_nanos(&pic),
                tokens_payout_fee: (p2_trade_cycles / trade_rate) * trade_rate / 10_000 * 50 / trade_rate,
                cycles_payout_fee: (p2_trade_cycles - (p2_trade_cycles % trade_rate)) / 10_000 * 50,
                cycles_payout_data: None,
                token_payout_data: None,
            }
        );
        assert_eq!(view_position_pending_trades_sponse_b[view_position_pending_trades_sponse_b.len()-2], 0);
        assert_eq!(view_position_pending_trades_sponse_b[view_position_pending_trades_sponse_b.len()-1], 0);
    }
    
    // pic.advance_time for the do_payouts 
    pic.advance_time(Duration::from_millis(1));
    for _i in 0..5 { pic.tick(); }

    // check icrc1_balances, p2 void positions pending (pending for the update-position-storage-log), trade is in the trades-storage, and pending-trades.len() == 0    
    
    assert_eq!(
        icrc1_balance(&pic, ICP_LEDGER, &Account{ owner: p2, subaccount: None}),
        (p2_trade_cycles / trade_rate) - ((p2_trade_cycles / trade_rate) * trade_rate / 10_000 * 50 / trade_rate) - ICP_LEDGER_TRANSFER_FEE
    );
    assert_eq!(
        icrc1_balance(&pic, BANK, &Account{ owner: p1, subaccount: None }),
        (p2_trade_cycles - (p2_trade_cycles % trade_rate)) - ((p2_trade_cycles - (p2_trade_cycles % trade_rate)) / 10_000 * 50) - BANK_TRANSFER_FEE
    );

    let p2_view_void_positions_pending_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_void_positions_pending",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p2)
        }).unwrap(),
    ).unwrap().unwrap();
    assert_eq!(p2_view_void_positions_pending_sponse_b.len(), PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1); 
    assert_eq!(
        PositionLog::stable_memory_serialize_backwards(&p2_view_void_positions_pending_sponse_b[..(p2_view_void_positions_pending_sponse_b.len() - 1)]),
        PositionLog{
            id: 1,
            positor: p2,
            quest: CreatePositionQuestLog{
                quantity: p2_trade_cycles,
                cycles_per_token_rate: trade_rate,
            },
            position_kind: PositionKind::Cycles,
            mainder_position_quantity: p2_trade_cycles % trade_rate,
            fill_quantity: p2_trade_cycles / trade_rate,
            fill_average_rate: trade_rate,
            payouts_fees_sum: (p2_trade_cycles / trade_rate) * trade_rate / 10_000 * 50 / trade_rate,
            creation_timestamp_nanos: p2_trade_cycles_timestamp_nanos,
            position_termination: Some(PositionTerminationData{
                cause: PositionTerminationCause::Fill,
                timestamp_nanos: p2_trade_cycles_timestamp_nanos
            }),
            void_position_payout_dust_collection: if p2_trade_cycles % trade_rate <= BANK_TRANSFER_FEE { true } else { false },
            void_position_payout_ledger_transfer_fee: BANK_TRANSFER_FEE as u64,
        }
    );    
    assert_eq!(p2_view_void_positions_pending_sponse_b[p2_view_void_positions_pending_sponse_b.len()-1], 1); // payout did not run yet
    
    let view_position_pending_trades_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_position_pending_trades",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: None::<u128>,
        }).unwrap(),
    ).unwrap().unwrap();
    assert_eq!(view_position_pending_trades_sponse_b.len(), 0);

    for index_key in [Some(0u128), Some(1u128), None::<u128>] {
        let view_position_purchases_logs_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_position_purchases_logs",
            candid::encode_one(ViewStorageLogsQuest{
                opt_start_before_id: None,
                index_key: index_key,
            }).unwrap(),
        ).unwrap().unwrap();
        assert_eq!(view_position_purchases_logs_sponse_b.len(), TradeLog::STABLE_MEMORY_SERIALIZE_SIZE);    
        assert_eq!(
            TradeLog::stable_memory_serialize_backwards(&view_position_purchases_logs_sponse_b),
            TradeLog{
                position_id_matcher: 1,
                position_id_matchee: 0,
                id: 0,
                matchee_position_positor: p1,
                matcher_position_positor: p2,
                tokens: p2_trade_cycles / trade_rate,
                cycles: p2_trade_cycles - (p2_trade_cycles % trade_rate),
                cycles_per_token_rate: trade_rate,
                matchee_position_kind: PositionKind::Token,
                timestamp_nanos: p2_trade_cycles_timestamp_nanos,
                tokens_payout_fee: (p2_trade_cycles / trade_rate) * trade_rate / 10_000 * 50 / trade_rate,
                cycles_payout_fee: (p2_trade_cycles - (p2_trade_cycles % trade_rate)) / 10_000 * 50,
                cycles_payout_data: Some(PayoutData{
                    did_transfer: if (p2_trade_cycles - (p2_trade_cycles % trade_rate)) - ((p2_trade_cycles - (p2_trade_cycles % trade_rate)) / 10_000 * 50) > BANK_TRANSFER_FEE { true } else { false },
                    ledger_transfer_fee: BANK_TRANSFER_FEE
                }),
                token_payout_data: Some(PayoutData{
                    did_transfer: if (p2_trade_cycles / trade_rate) - ((p2_trade_cycles / trade_rate) * trade_rate / 10_000 * 50 / trade_rate) > ICP_LEDGER_TRANSFER_FEE { true } else { false },
                    ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE                
                }),
            }
        );
    }
    
    // pic.advance_time for the update-storage-position-log do_payouts. 
    pic.advance_time(Duration::from_secs(30));
    for _i in 0..5 { pic.tick(); }
    
    // p2 position is in the storage-logs, and void-positions-pending.len() == 0
    
    for index_key in [Some(p2), None] {
        let p2_view_void_positions_pending_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_void_positions_pending",
            candid::encode_one(ViewStorageLogsQuest{
                opt_start_before_id: None,
                index_key,
            }).unwrap(),
        ).unwrap().unwrap();
        assert_eq!(p2_view_void_positions_pending_sponse_b.len(), 0); 
    }
    
    let p2_view_user_positions_logs_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_user_positions_logs",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p2)
        }).unwrap(),
    ).unwrap().unwrap();
    assert_eq!(p2_view_user_positions_logs_sponse_b.len(), PositionLog::STABLE_MEMORY_SERIALIZE_SIZE); 
    assert_eq!(
        PositionLog::stable_memory_serialize_backwards(&p2_view_user_positions_logs_sponse_b),
        PositionLog{
            id: 1,
            positor: p2,
            quest: CreatePositionQuestLog{
                quantity: p2_trade_cycles,
                cycles_per_token_rate: trade_rate,
            },
            position_kind: PositionKind::Cycles,
            mainder_position_quantity: p2_trade_cycles % trade_rate,
            fill_quantity: p2_trade_cycles / trade_rate,
            fill_average_rate: trade_rate,
            payouts_fees_sum: (p2_trade_cycles / trade_rate) * trade_rate / 10_000 * 50 / trade_rate,
            creation_timestamp_nanos: p2_trade_cycles_timestamp_nanos,
            position_termination: Some(PositionTerminationData{
                cause: PositionTerminationCause::Fill,
                timestamp_nanos: p2_trade_cycles_timestamp_nanos
            }),
            void_position_payout_dust_collection: if p2_trade_cycles % trade_rate <= BANK_TRANSFER_FEE { true } else { false },
            void_position_payout_ledger_transfer_fee: BANK_TRANSFER_FEE as u64,
        }
    );    
    
    // cancel p1 position.
}


#[test]
fn test_candle_counter_1() {
    let pic = set_up();
    let tc = set_up_tc(&pic);
    
    let (p1,p2): (Principal,Principal) = (
        Principal::from_slice(&[1,1,1,1,1]),
        Principal::from_slice(&[2,2,2,2,2]),
    );
    
    // trades
    let p1_trade_icp = 10000000000;
    let trade_rate = 77777;
    mint_icp(&pic, &Account{owner: tc, subaccount: Some(principal_token_subaccount(&p1))}, p1_trade_icp + ICP_LEDGER_TRANSFER_FEE);
    
    call_candid_as_::<_, (TradeResult,)>(&pic, tc, p1, "trade_tokens", (
        TradeTokensQuest{
            tokens: p1_trade_icp,
            cycles_per_token_rate: trade_rate,
            posit_transfer_ledger_fee: Some(ICP_LEDGER_TRANSFER_FEE),
            return_tokens_to_subaccount: None,
            payout_cycles_to_subaccount: None,
        },
    )).unwrap().0.unwrap();
    
    let p2_mint_cycles = mint_cycles(&pic, &Account{owner: tc, subaccount: Some(principal_token_subaccount(&p2))}, 500000000);
    let p2_trade_cycles = p2_mint_cycles - BANK_TRANSFER_FEE;
    
    call_candid_as_::<_, (TradeResult,)>(&pic, tc, p2, "trade_cycles", (
        TradeCyclesQuest{
            cycles: p2_trade_cycles,
            cycles_per_token_rate: trade_rate,
            posit_transfer_ledger_fee: Some(BANK_TRANSFER_FEE),
            return_cycles_to_subaccount: None,
            payout_tokens_to_subaccount: None,
        },
    )).unwrap().0.unwrap();    

    // -
    let candles: Vec<Candle> = view_candles(&pic, tc, 
        ViewCandlesQuest{
    		segment_length: ViewCandlesSegmentLength::OneMinute,
    		opt_start_before_time_nanos: None,
    	}
    ).candles;
    
    assert_eq!(candles.len(), 1);
	assert_eq!(candles[0], Candle{
        time_nanos: pic_get_time_nanos(&pic) as u64 - (pic_get_time_nanos(&pic) as u64 % (NANOS_IN_A_SECOND * SECONDS_IN_A_MINUTE * 1) as u64),
        volume_cycles: {
            // trade cycles amount is a multiple of the rate
            let trade_cycles_amount = p2_trade_cycles - (p2_trade_cycles % trade_rate);
            std::cmp::min(trade_cycles_amount, tokens_transform_cycles(p1_trade_icp, trade_rate))
        },
        volume_tokens: std::cmp::min(p1_trade_icp, cycles_transform_tokens(p2_trade_cycles, trade_rate)),
        open_rate: trade_rate,
        high_rate: trade_rate,
        low_rate: trade_rate,
        close_rate: trade_rate,
    });    
    
}



