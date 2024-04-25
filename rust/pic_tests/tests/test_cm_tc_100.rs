use pic_tools::{*, bank::mint_cycles};
use cts_lib::{
    types::{
        Cycles,
        bank::BANK_TRANSFER_FEE,
        cm::tc::{*, storage_logs::{*, position_log::*, trade_log::*}},
    },
    tools::principal_token_subaccount,
    consts::{KiB, TRILLION},
    icrc::Tokens,
};
use icrc_ledger_types::icrc1::{
    account::Account,
    transfer::{TransferArg, TransferError},
};
use core::time::Duration;
use pocket_ic::*;
use candid::Principal;


#[test]
fn test_100() {
    let pic = set_up();
    let tc = set_up_tc(&pic);
    
    let (p1,p2): (Principal,Principal) = (
        Principal::from_slice(&[1,1,1,1,1]),
        Principal::from_slice(&[2,2,2,2,2]),
    );
    
    let p_burn_icp = 50_000 * TRILLION / CMC_RATE;
    #[allow(non_snake_case)]
    let P_START_CYCLES_BALANCE: Cycles  = mint_cycles(&pic, &Account{owner: p1, subaccount: None}, p_burn_icp);
                                          pic.advance_time(Duration::from_secs(4000)); // for the cmc limit
                                          pic.tick();
                                          mint_cycles(&pic, &Account{owner: p2, subaccount: None}, p_burn_icp);
                                          
    const P_START_TOKEN_BALANCE: Tokens = 100_000_000_00000000;
    
    mint_icp(&pic, &Account{owner: p1, subaccount: None}, P_START_TOKEN_BALANCE);
    mint_icp(&pic, &Account{owner: p2, subaccount: None}, P_START_TOKEN_BALANCE);
    
    let tc_positions_subaccount = Account{
        owner: tc,
        subaccount: Some([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,5]),
    };
    
    let trade_tokens: Tokens = 110000;
    let trade_tokens_rate: CyclesPerToken = 1 * TRILLION / trade_tokens; // 9090909
    let trade_cycles: Cycles = trade_tokens * trade_tokens_rate; 
    let cycles_payout_fee = trade_cycles / 10_000 * 50;    
    let tokens_payout_fee = cycles_payout_fee / trade_tokens_rate;
    
    let [trades_flushes_at_len, positions_flushes_at_len] = {
        [TradeLog::STABLE_MEMORY_SERIALIZE_SIZE, PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]
        .map(|stable_memory_serialize_size| {
            if FLUSH_STORAGE_BUFFER_AT_SIZE % stable_memory_serialize_size == 0 {
                FLUSH_STORAGE_BUFFER_AT_SIZE
            } else {   
                FLUSH_STORAGE_BUFFER_AT_SIZE + (stable_memory_serialize_size - (FLUSH_STORAGE_BUFFER_AT_SIZE % stable_memory_serialize_size))
            }
        })
    };
    println!("trades_flushes_at_len: {trades_flushes_at_len}");
    println!("positions_flushes_at_len: {positions_flushes_at_len}");    
    
    for i in 0_u128..100 {
        println!("{i}");
        
        let _block = call_candid_as_::<_, (Result<u128, TransferError>,)>(
            &pic,
            ICP_LEDGER,
            p1,
            "icrc1_transfer",
            (TransferArg{
                from_subaccount: None,
                to: Account{
                    owner: tc,
                    subaccount: Some(principal_token_subaccount(&p1))
                },
                fee: Some(ICP_LEDGER_TRANSFER_FEE.into()),
                created_at_time: None,
                memo: None,
                amount: (trade_tokens + ICP_LEDGER_TRANSFER_FEE/*for the tc-collect-posit-transfer*/).into(),
            },)
        ).unwrap().0.unwrap();
        
        let token_position_id: u128 = call_candid_as_::<_, (TradeResult,)>(&pic, tc, p1, "trade_tokens", (
            TradeTokensQuest{
                tokens: trade_tokens,
                cycles_per_token_rate: trade_tokens_rate,
                posit_transfer_ledger_fee: Some(ICP_LEDGER_TRANSFER_FEE),
                return_tokens_to_subaccount: None,
                payout_cycles_to_subaccount: None,
            },
        )).unwrap().0.unwrap().position_id;  
        
        assert_eq!(token_position_id, i*2);
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner: tc, subaccount: Some(principal_token_subaccount(&p1))}), 
            0
        );
        assert_eq!(icrc1_balance(&pic, ICP_LEDGER, &tc_positions_subaccount), trade_tokens + tokens_payout_fee * i);
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner: p1, subaccount: None}), 
            P_START_TOKEN_BALANCE - ((trade_tokens + (ICP_LEDGER_TRANSFER_FEE * 2))*(i+1)) 
        );
        
        let mut token_position_log = PositionLog{
            id: i * 2,
            positor: p1,
            quest: CreatePositionQuestLog {
                quantity: trade_tokens,
                cycles_per_token_rate: trade_tokens_rate,
            },
            position_kind: PositionKind::Token,
            mainder_position_quantity: trade_tokens, 
            fill_quantity: 0, 
            fill_average_rate: trade_tokens_rate,
            payouts_fees_sum: 0,
            creation_timestamp_nanos: pic_get_time_nanos(&pic),
            position_termination: None,
            void_position_payout_dust_collection: false, 
            void_position_payout_ledger_transfer_fee: 0u64,
        };
        
        let cm_data: CMData = create_and_download_state_snapshot::<CMData>(&pic, CM_MAIN, tc, 0);
        assert_eq!(
            cm_data.token_positions[0],
            TokenPosition{
                id: i * 2,
                positor: p1,
                quest: TradeTokensQuest{
                    tokens: trade_tokens,
                    cycles_per_token_rate: trade_tokens_rate,
                    posit_transfer_ledger_fee: Some(ICP_LEDGER_TRANSFER_FEE),
                    payout_cycles_to_subaccount: None,
                    return_tokens_to_subaccount: None,
                },
                current_position_tokens: trade_tokens,
                purchases_rates_times_token_quantities_sum: 0,
                cycles_payouts_fees_sum: 0,
                timestamp_nanos: cm_data.token_positions[0].timestamp_nanos,
            }
        );
        assert_eq!(cm_data.token_positions.len(), 1);
        assert_eq!(cm_data.cycles_positions.len(), 0);
        assert_eq!(cm_data.void_token_positions.len(), 0);
        assert_eq!(cm_data.void_cycles_positions.len(), 0);
        
        let positions_log_storage_data = create_and_download_state_snapshot::<LogStorageData>(&pic, CM_MAIN, tc, 1);
        assert_eq!(
            PositionLog::stable_memory_serialize_backwards(
                &positions_log_storage_data.storage_buffer[
                    (positions_log_storage_data.storage_buffer.len() - PositionLog::STABLE_MEMORY_SERIALIZE_SIZE)
                    ..
                ]
            ),
            token_position_log,
        );
        
        // do_payouts - nothing happens, do_payouts_returns early.
        pic.advance_time(Duration::from_secs(1));
        for _ in 0..20 {
            pic.tick();
        }
                        
        let _block = call_candid_as_::<_, (Result<u128, TransferError>,)>(
            &pic,
            BANK,
            p2,
            "icrc1_transfer",
            (TransferArg{
                from_subaccount: None,
                to: Account{
                    owner: tc,
                    subaccount: Some(principal_token_subaccount(&p2))
                },
                fee: Some(BANK_TRANSFER_FEE.into()),
                created_at_time: None,
                memo: None,
                amount: (trade_cycles + BANK_TRANSFER_FEE/*for the tc-collect-posit-transfer*/).into(),
            },)
        ).unwrap().0.unwrap();
        
        let cycles_position_id = call_candid_as_::<_, (TradeResult,)>(&pic, tc, p2, "trade_cycles", (
            TradeCyclesQuest{
                cycles: trade_cycles,
                cycles_per_token_rate: trade_tokens_rate,
                posit_transfer_ledger_fee: Some(BANK_TRANSFER_FEE),
                return_cycles_to_subaccount: None,
                payout_tokens_to_subaccount: None,
            },
        )).unwrap().0.unwrap().position_id;    
        
        assert_eq!(cycles_position_id, i*2+1);
        assert_eq!(
            icrc1_balance(&pic, BANK, &Account{owner: p2, subaccount: None}), 
            P_START_CYCLES_BALANCE - ((trade_cycles + (BANK_TRANSFER_FEE * 2))*(i+1))
        );
        
        token_position_log = PositionLog{
            mainder_position_quantity: 0, 
            fill_quantity: trade_cycles, 
            payouts_fees_sum: cycles_payout_fee,
            position_termination: Some(PositionTerminationData{
                timestamp_nanos: pic_get_time_nanos(&pic),
                cause: PositionTerminationCause::Fill,
            }),
            ..token_position_log
        };
        
        let mut cycles_position_log = PositionLog{
            id: i * 2 + 1,
            positor: p2,
            quest: CreatePositionQuestLog {
                quantity: trade_cycles,
                cycles_per_token_rate: trade_tokens_rate,
            },
            position_kind: PositionKind::Cycles,
            mainder_position_quantity: 0, 
            fill_quantity: trade_tokens, 
            fill_average_rate: trade_tokens_rate,
            payouts_fees_sum: tokens_payout_fee,
            creation_timestamp_nanos: pic_get_time_nanos(&pic),
            position_termination: Some(PositionTerminationData{
                timestamp_nanos: pic_get_time_nanos(&pic),
                cause: PositionTerminationCause::Fill,
            }),
            void_position_payout_dust_collection: false,
            void_position_payout_ledger_transfer_fee: 0u64,
        };

        let mut void_token_position = VoidTokenPosition{
            position_id: token_position_log.id,
            positor: token_position_log.positor,
            tokens: 0,
            token_payout_lock: false,
            token_payout_data: None,
            timestamp_nanos: pic_get_time_nanos(&pic),
            update_storage_position_data: VPUpdateStoragePositionData {
                lock: false,
                status: false,
                update_storage_position_log: token_position_log.clone(),
            },
            return_tokens_to_subaccount: None,
        };
        
        let mut void_cycles_position = VoidCyclesPosition{
            position_id: cycles_position_log.id,
            positor: cycles_position_log.positor,
            cycles: 0,
            cycles_payout_lock: false,
            cycles_payout_data: None,
            timestamp_nanos: pic_get_time_nanos(&pic),
            update_storage_position_data: VPUpdateStoragePositionData {
                lock: false,
                status: false,
                update_storage_position_log: cycles_position_log.clone(),
            },
            return_cycles_to_subaccount: None,
        };
        
        let cm_data: CMData = create_and_download_state_snapshot::<CMData>(&pic, CM_MAIN, tc, 0);
        assert_eq!(cm_data.token_positions.len(), 0);
        assert_eq!(cm_data.cycles_positions.len(), 0);
        assert_eq!(cm_data.void_token_positions.len(), 1);
        assert_eq!(cm_data.void_cycles_positions.len(), 1);
        assert_eq!(cm_data.trade_logs.len(), 1);
        assert_eq!(
            cm_data.void_token_positions[0],
            void_token_position,
        );
        assert_eq!(
            cm_data.void_cycles_positions[0],
            void_cycles_position
        );
        
        // do_payouts
        pic.advance_time(Duration::from_secs(1));
        for _ in 0..20 {
            pic.tick();
        }
        
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &tc_positions_subaccount), 
            tokens_payout_fee + tokens_payout_fee*i 
        );
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner: p2, subaccount: None}), 
            P_START_TOKEN_BALANCE + ((trade_tokens - tokens_payout_fee - ICP_LEDGER_TRANSFER_FEE)*(i+1))
        );
        assert_eq!(
            icrc1_balance(&pic, BANK, &Account{owner: p1, subaccount: None}), 
            P_START_CYCLES_BALANCE + ((trade_cycles - cycles_payout_fee - BANK_TRANSFER_FEE)*(i+1))
        );   
        
        token_position_log = PositionLog{
            void_position_payout_dust_collection: true,
            void_position_payout_ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE as u64,
            ..token_position_log
        };
        
        cycles_position_log = PositionLog{
            void_position_payout_dust_collection: true,
            void_position_payout_ledger_transfer_fee: BANK_TRANSFER_FEE as u64,
            ..cycles_position_log
        };
        
        void_token_position = VoidTokenPosition{
            token_payout_data: Some(PayoutData{
                did_transfer: false,
                ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE,
            }),
            update_storage_position_data: VPUpdateStoragePositionData {
                update_storage_position_log: token_position_log.clone(),
                ..void_token_position.update_storage_position_data
            },
            ..void_token_position
        };
        
        void_cycles_position = VoidCyclesPosition{
            cycles_payout_data: Some(PayoutData{
                did_transfer: false,
                ledger_transfer_fee: BANK_TRANSFER_FEE,
            }),
            update_storage_position_data: VPUpdateStoragePositionData {
                update_storage_position_log: cycles_position_log.clone(),
                ..void_cycles_position.update_storage_position_data
            },
            ..void_cycles_position
        };
        
        let cm_data: CMData = create_and_download_state_snapshot::<CMData>(&pic, CM_MAIN, tc, 0);
        assert_eq!(cm_data.void_token_positions.len(), 1);
        assert_eq!(cm_data.void_cycles_positions.len(), 1);
        assert_eq!(cm_data.trade_logs.len(), 0);
        assert_eq!(
            cm_data.void_token_positions[0],
            void_token_position,
        );
        assert_eq!(
            cm_data.void_cycles_positions[0],
            void_cycles_position
        );
                
        let void_positions_pending_b: Vec<u8> = pic.query_call(
            tc,
            Principal::anonymous(),
            "view_void_positions_pending",
            candid::encode_one(
                ViewStorageLogsQuest::<Principal>{
                    opt_start_before_id: None,
                    index_key: None
                }
            ).unwrap(),
        ).unwrap().unwrap();
        
        assert_eq!(
            void_positions_pending_b.len(),
            (PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1) * 2,
        );
        
        let (pl_1, pl_1_payout_status): (PositionLog, bool) = (
            PositionLog::stable_memory_serialize_backwards(&void_positions_pending_b[0..PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]),
            void_positions_pending_b[PositionLog::STABLE_MEMORY_SERIALIZE_SIZE] == 1,
        );
        
        assert_eq!(pl_1_payout_status, true);
        assert_eq!(
            pl_1,
            token_position_log,
        );
        
        let (pl_2, pl_2_payout_status): (PositionLog, bool) = (
            PositionLog::stable_memory_serialize_backwards(&void_positions_pending_b[
                PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1
                ..
                PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1 + PositionLog::STABLE_MEMORY_SERIALIZE_SIZE
            ]),
            void_positions_pending_b[(PositionLog::STABLE_MEMORY_SERIALIZE_SIZE * 2) + 1] == 1,
        );
        
        assert_eq!(pl_2_payout_status, true);
        assert_eq!(
            pl_2,
            cycles_position_log
        );

        let view_trades_storage_logs: Vec<u8> = pic.query_call(
            tc,
            Principal::anonymous(),
            "view_position_purchases_logs",
            candid::encode_one(
                ViewStorageLogsQuest::<u128>{
                    opt_start_before_id: None,
                    index_key: None
                }
            ).unwrap(),
        ).unwrap().unwrap();
            
        assert_eq!(
            view_trades_storage_logs.len(),
            TradeLog::STABLE_MEMORY_SERIALIZE_SIZE * (i+1) as usize % trades_flushes_at_len,
        ); 
        
        let tl_log_backwards: TradeLog;        
        if ((i as usize)+1) * TradeLog::STABLE_MEMORY_SERIALIZE_SIZE % trades_flushes_at_len != 0 {
            tl_log_backwards = TradeLog::stable_memory_serialize_backwards(&view_trades_storage_logs[view_trades_storage_logs.len() - TradeLog::STABLE_MEMORY_SERIALIZE_SIZE..]);
        } else {
            let (trades_storage_canisters,): (Vec<StorageCanister>,) = query_candid(
                &pic,
                tc,
                "view_trades_storage_canisters",
                (),
            ).unwrap();
            println!("trades_storage_canisters_len: {:?}", trades_storage_canisters.len());
            let trades_storage_canister = trades_storage_canisters.last().unwrap().canister_id;
            let user_trades: Vec<u8> = pic.query_call(
                trades_storage_canister,
                Principal::anonymous(),
                "map_logs_rchunks",
                candid::encode_args((i*2, None::<u128>, (512*KiB*3/TradeLog::STABLE_MEMORY_SERIALIZE_SIZE) as u32)).unwrap()
            ).unwrap().unwrap();
            tl_log_backwards = TradeLog::stable_memory_serialize_backwards(&user_trades[user_trades.len() - TradeLog::STABLE_MEMORY_SERIALIZE_SIZE..]);
        }
        
        let trade_log = TradeLog{
            position_id_matcher: i*2 + 1,
            position_id_matchee: i*2,
            id: i,
            matchee_position_positor: p1,
            matcher_position_positor: p2,
            tokens: trade_tokens,
            cycles: trade_cycles,
            cycles_per_token_rate: trade_tokens_rate,
            matchee_position_kind: PositionKind::Token,
            timestamp_nanos: tl_log_backwards.timestamp_nanos,
            tokens_payout_fee: tokens_payout_fee,
            cycles_payout_fee: cycles_payout_fee,
            cycles_payout_data: Some(PayoutData{
                did_transfer: true,
                ledger_transfer_fee: BANK_TRANSFER_FEE
                
            }),
            token_payout_data: Some(PayoutData{
                did_transfer: true,
                ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE
            })
        };
        assert_eq!(
            tl_log_backwards,
            trade_log,
        );        
        
        pic.advance_time(Duration::from_secs(40)); // update-storage-position-log
        for _ in 0..20 {
            pic.tick();
        }
        
        let cm_data: CMData = create_and_download_state_snapshot::<CMData>(&pic, CM_MAIN, tc, 0);
        assert_eq!(cm_data.void_token_positions.len(), 0);
        assert_eq!(cm_data.void_cycles_positions.len(), 0);
        
        let view_positions_storage_logs: Vec<u8> = pic.query_call(
            tc,
            Principal::anonymous(),
            "view_user_positions_logs",
            candid::encode_one(
                ViewStorageLogsQuest::<Principal>{
                    opt_start_before_id: None,
                    index_key: None
                }
            ).unwrap(),
        ).unwrap().unwrap();
        
        let total_pls_ser_size = PositionLog::STABLE_MEMORY_SERIALIZE_SIZE * (i+1) as usize * 2;
        
        assert_eq!(
            view_positions_storage_logs.len(),
            total_pls_ser_size % positions_flushes_at_len   
        );
        
        let pl1_log_backwards: PositionLog;        
        let pl2_log_backwards: PositionLog;
        
        if total_pls_ser_size % positions_flushes_at_len != 0 {
            pl1_log_backwards = PositionLog::stable_memory_serialize_backwards(
                &view_positions_storage_logs[
                    view_positions_storage_logs.len() - (PositionLog::STABLE_MEMORY_SERIALIZE_SIZE * 2)
                    ..
                    (view_positions_storage_logs.len() - PositionLog::STABLE_MEMORY_SERIALIZE_SIZE)
                ]
            );
            pl2_log_backwards = PositionLog::stable_memory_serialize_backwards(
                &view_positions_storage_logs[
                    view_positions_storage_logs.len() - PositionLog::STABLE_MEMORY_SERIALIZE_SIZE
                    ..
                ]
            );
        } else {
            let (positions_storage_canisters,): (Vec<StorageCanister>,) = query_candid(
                &pic,
                tc,
                "view_positions_storage_canisters",
                (),
            ).unwrap();
            println!("positions_storage_canisters_len: {:?}", positions_storage_canisters.len());
            let positions_storage_canister = positions_storage_canisters.last().unwrap().canister_id;
            [pl1_log_backwards, pl2_log_backwards] = [p1, p2].map(|p| {
                let user_positions: Vec<u8> = pic.query_call(
                    positions_storage_canister,
                    Principal::anonymous(),
                    "map_logs_rchunks",
                    candid::encode_args((p, None::<u128>, (512*KiB*3/PositionLog::STABLE_MEMORY_SERIALIZE_SIZE) as u32)).unwrap()
                ).unwrap().unwrap();
                PositionLog::stable_memory_serialize_backwards(
                    &user_positions[
                        user_positions.len() - PositionLog::STABLE_MEMORY_SERIALIZE_SIZE
                        ..
                    ]
                )                                
            });
        } 
        
        assert_eq!(pl1_log_backwards, token_position_log);
        assert_eq!(pl2_log_backwards, cycles_position_log);
    }
}



