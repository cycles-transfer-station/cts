use pic_tools::{*, bank::mint_cycles};
use candid::Principal;
use cts_lib::{
    types::{
        bank::BANK_TRANSFER_FEE,
        cm::tc::{
            *,
            storage_logs::{*, position_log::*, trade_log::*},
            
        },
    },
    tools::principal_token_subaccount,
};
use icrc_ledger_types::icrc1::account::Account;



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
    
    let p2_view_user_current_positions_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_user_current_positions",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p2)
        }).unwrap(),
    ).unwrap().unwrap();
    assert_eq!(p2_view_user_current_positions_sponse_b.len(), 0);
    
    // check pending trades, p1-current-position, p2-void-position-pending
    
    // pic.advance_time for the do_payouts 
    
    // p2 void positions pending (pending for the update-position-storage-log), trade is in the trades-storage, and pending-trades.len() == 0    
    
    // pic.advance_time for the update-storage-position-log do_payouts. 
    
    // p2 position is in the storage-logs, and void-positions-pending.len() == 0 
    
    
    /*    
    let p2_view_void_positions_pending_sponse_b = pic.query_call(tc, Principal::anonymous(), "view_void_positions_pending",
        candid::encode_one(ViewStorageLogsQuest{
            opt_start_before_id: None,
            index_key: Some(p2)
        }).unwrap(),
    ).unwrap().unwrap();
    
    assert_eq!(p2_view_void_positions_pending_sponse_b.len(), PositionLog::STABLE_MEMORY_SERIALIZE_SIZE)
    */
    
}
