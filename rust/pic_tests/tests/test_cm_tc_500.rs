use pic_tools::{*, bank::mint_cycles};
use cts_lib::{
    types::{
        Cycles,
        bank::BANK_TRANSFER_FEE,
        cm::{
            tc::{*, storage_logs::{*, position_log::*, trade_log::*}},
            cm_main::TradeContractIdAndLedgerId,
        },
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
use pocket_ic::{*, common::rest::RawEffectivePrincipal};
use candid::{Principal, Deserialize};


#[test]
fn test_500() {
    let pic = set_up();
    let tc = set_up_tc(&pic);
    
    let (p1,p2): (Principal,Principal) = (
        Principal::from_slice(&[1,1,1,1,1]),
        Principal::from_slice(&[2,2,2,2,2]),
    );
    
    let p_burn_icp = 50_000 * TRILLION / CMC_RATE;
    let P_START_CYCLES_BALANCE: Cycles  = mint_cycles(&pic, &Account{owner: p1, subaccount: None}, p_burn_icp);
                                          pic.advance_time(Duration::from_secs(4000));
                                          pic.tick();
                                          mint_cycles(&pic, &Account{owner: p2, subaccount: None}, p_burn_icp);
                                          
    const P_START_TOKEN_BALANCE: Tokens = 100_000_000_00000000;
    
    mint_icp(&pic, &Account{owner: p1, subaccount: None}, P_START_TOKEN_BALANCE);
    mint_icp(&pic, &Account{owner: p2, subaccount: None}, P_START_TOKEN_BALANCE);
    
    let tc_id_and_ledger_id = TradeContractIdAndLedgerId{
        icrc1_ledger_canister_id: ICP_LEDGER,
        trade_contract_canister_id: tc,    
    };
    let tc_p1_subaccount = Account{
        owner: tc,
        subaccount: Some(principal_token_subaccount(&p1))
    };
    let tc_p2_subaccount = Account{
        owner: tc,
        subaccount: Some(principal_token_subaccount(&p2))
    };
    let tc_positions_subaccount = Account{
        owner: tc,
        subaccount: Some([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,5]),
    };
    
    let trade_tokens: Tokens = 110000;
    let trade_tokens_rate: CyclesPerToken = 1 * TRILLION / trade_tokens;
    let trade_cycles: Cycles = trade_tokens * trade_tokens_rate; 
    let cycles_payout_fee = trade_cycles / 10_000 * 50;    
    let tokens_payout_fee = cycles_payout_fee / trade_tokens_rate;
    
    for i in 0_u128..500 {
        println!("{i}");
        
        let token_position_id: u128 = create_token_position(&pic, p1, tc, trade_tokens, trade_tokens_rate);
        
        assert_eq!(token_position_id, i*2);
        assert_eq!(icrc1_balance(&pic, ICP_LEDGER, &tc_p1_subaccount), 0);
        assert_eq!(icrc1_balance(&pic, ICP_LEDGER, &tc_positions_subaccount), trade_tokens + tokens_payout_fee * i);
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner: p1, subaccount: None}), 
            P_START_TOKEN_BALANCE - ((trade_tokens + (ICP_LEDGER_TRANSFER_FEE * 2))*(i+1)) 
        );
        
        {
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
        }
        
        let trade_cycles: Cycles = trade_tokens * trade_tokens_rate; 
        
        let cycles_position_id = create_cycles_position(&pic, p2, tc, trade_cycles, trade_tokens_rate);
        assert_eq!(cycles_position_id, i*2+1);
        
        let cycles_payout_fee = trade_cycles / 10_000 * 50;    
        let tokens_payout_fee = cycles_payout_fee / trade_tokens_rate;
        
        pic.advance_time(Duration::from_millis(1));
        pic.tick();
        
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &tc_positions_subaccount), 
            tokens_payout_fee + tokens_payout_fee*i 
        );
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner: p1, subaccount: None}), 
            P_START_TOKEN_BALANCE - ((trade_tokens + (ICP_LEDGER_TRANSFER_FEE * 2)) * (i+1))
        );
        assert_eq!(
            icrc1_balance(&pic, ICP_LEDGER, &Account{owner: p2, subaccount: None}), 
            P_START_TOKEN_BALANCE + ((trade_tokens - tokens_payout_fee - ICP_LEDGER_TRANSFER_FEE)*(i+1))
        );
        assert_eq!(
            icrc1_balance(&pic, BANK, &Account{owner: p1, subaccount: None}), 
            P_START_CYCLES_BALANCE + ((trade_cycles - cycles_payout_fee - BANK_TRANSFER_FEE)*(i+1))
        );   
        assert_eq!(
            icrc1_balance(&pic, BANK, &Account{owner: p2, subaccount: None}), 
            P_START_CYCLES_BALANCE - ((trade_cycles + (BANK_TRANSFER_FEE * 2))*(i+1))
        );   
        
        let trades_flushes_at_len = if FLUSH_STORAGE_BUFFER_AT_SIZE % TradeLog::STABLE_MEMORY_SERIALIZE_SIZE == 0 {
            FLUSH_STORAGE_BUFFER_AT_SIZE
        } else {   
            FLUSH_STORAGE_BUFFER_AT_SIZE + (TradeLog::STABLE_MEMORY_SERIALIZE_SIZE - (FLUSH_STORAGE_BUFFER_AT_SIZE % TradeLog::STABLE_MEMORY_SERIALIZE_SIZE))
        };
        println!("trades_flushes_at_len: {trades_flushes_at_len}");
        let positions_flushes_at_len = if FLUSH_STORAGE_BUFFER_AT_SIZE % PositionLog::STABLE_MEMORY_SERIALIZE_SIZE == 0 {
            FLUSH_STORAGE_BUFFER_AT_SIZE
        } else {   
            FLUSH_STORAGE_BUFFER_AT_SIZE + (PositionLog::STABLE_MEMORY_SERIALIZE_SIZE - (FLUSH_STORAGE_BUFFER_AT_SIZE % PositionLog::STABLE_MEMORY_SERIALIZE_SIZE))
        };
        println!("positions_flushes_at_len: {positions_flushes_at_len}");
                            
        let view_trades_storage_logs: Vec<u8> = pic.query_call(
            tc,
            Principal::anonymous(),
            "view_position_purchases_logs",
            candid::encode_one(
                ViewStorageLogsQuest::<Principal>{
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
            tl_log_backwards = tl_backwards(&view_trades_storage_logs[view_trades_storage_logs.len() - TradeLog::STABLE_MEMORY_SERIALIZE_SIZE..]);
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
            tl_log_backwards = tl_backwards(&user_trades[user_trades.len() - TradeLog::STABLE_MEMORY_SERIALIZE_SIZE..]);
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
            pl_backwards(&void_positions_pending_b[0..PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]),
            void_positions_pending_b[PositionLog::STABLE_MEMORY_SERIALIZE_SIZE] == 1,
        );
        
        let token_position_position_log = PositionLog{
            id: i*2,
            positor: p1,
            quest: CreatePositionQuestLog {
                quantity: trade_tokens,
                cycles_per_token_rate: trade_tokens_rate,
            },
            position_kind: PositionKind::Token,
            mainder_position_quantity: 0, 
            fill_quantity: trade_cycles, 
            fill_average_rate: trade_tokens_rate,
            payouts_fees_sum: cycles_payout_fee,
            creation_timestamp_nanos: pl_1.creation_timestamp_nanos,
            position_termination: Some(
                PositionTerminationData{
                    timestamp_nanos: pl_1.position_termination.as_ref().unwrap().timestamp_nanos,
                    cause: PositionTerminationCause::Fill,
                }
            ),
            void_position_payout_dust_collection: true,
            void_position_payout_ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE as u64,    
        };
        
        assert_eq!(pl_1_payout_status, true);
        assert_eq!(
            pl_1,
            token_position_position_log,
        );
        
        let (pl_2, pl_2_payout_status): (PositionLog, bool) = (
            pl_backwards(&void_positions_pending_b[
                PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1
                ..
                PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1 + PositionLog::STABLE_MEMORY_SERIALIZE_SIZE
            ]),
            void_positions_pending_b[(PositionLog::STABLE_MEMORY_SERIALIZE_SIZE * 2) + 1] == 1,
        );
        
        let cycles_position_position_log = PositionLog{
            id: i*2 + 1,
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
            creation_timestamp_nanos: pl_2.creation_timestamp_nanos,
            position_termination: Some(
                PositionTerminationData{
                    timestamp_nanos: pl_2.position_termination.as_ref().unwrap().timestamp_nanos,
                    cause: PositionTerminationCause::Fill,
                }
            ),
            void_position_payout_dust_collection: true,
            void_position_payout_ledger_transfer_fee: BANK_TRANSFER_FEE as u64,    
        };
        
        assert_eq!(pl_2_payout_status, true);
        assert_eq!(
            pl_2,
            cycles_position_position_log
        );
        
        for (p, pl) in [(p1, &token_position_position_log), (p2, &cycles_position_position_log)] {
            let void_positions_pending_b_cb: Vec<u8> = pic.query_call(
                tc,
                Principal::anonymous(),
                "view_void_positions_pending",
                candid::encode_one(
                    ViewStorageLogsQuest::<Principal>{
                        opt_start_before_id: None,
                        index_key: Some(p)
                    }
                ).unwrap(),
            ).unwrap().unwrap();
            assert_eq!(
                void_positions_pending_b_cb.len(),
                (PositionLog::STABLE_MEMORY_SERIALIZE_SIZE + 1),
            );
            let (pl_here, pl_payout_status): (PositionLog, bool) = (
                pl_backwards(&void_positions_pending_b_cb[0..PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]),
                void_positions_pending_b_cb[PositionLog::STABLE_MEMORY_SERIALIZE_SIZE] == 1,
            );
            assert_eq!(pl_payout_status, true);
            assert_eq!(
                pl_here,
                *pl,
            );
        }
        
        let cm_data: CMData = create_and_download_state_snapshot::<CMData>(&pic, CM_MAIN, tc, 0);
        
        assert_eq!(
            cm_data.void_token_positions[0],
            VoidTokenPosition{
                position_id: i*2,
                tokens: 0,
                positor: p1,
                token_payout_lock: false,
                token_payout_data: Some(PayoutData{
                    did_transfer: false,
                    ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE
                }),
                timestamp_nanos: cm_data.void_token_positions[0].timestamp_nanos,
                update_storage_position_data: VPUpdateStoragePositionData{
                    lock: false,
                    status: false,
                    update_storage_position_log: token_position_position_log.clone(), 
                },    
                return_tokens_to_subaccount: None,
            }
        );
        
        assert_eq!(
            cm_data.void_cycles_positions[0],
            VoidCyclesPosition{
                position_id: i*2 + 1,
                cycles: 0,
                positor: p2,
                cycles_payout_lock: false,
                cycles_payout_data: Some(PayoutData{
                    did_transfer: false,
                    ledger_transfer_fee: BANK_TRANSFER_FEE
                }),
                timestamp_nanos: cm_data.void_cycles_positions[0].timestamp_nanos,
                update_storage_position_data: VPUpdateStoragePositionData{
                    lock: false,
                    status: false,
                    update_storage_position_log: cycles_position_position_log.clone()
                },
                return_cycles_to_subaccount: None,    
            }
        );
        
        assert_eq!(cm_data.trade_logs.len(), 0);
        assert_eq!(cm_data.token_positions.len(), 0);
        assert_eq!(cm_data.cycles_positions.len(), 0);
        assert_eq!(cm_data.void_token_positions.len(), 1);
        assert_eq!(cm_data.void_cycles_positions.len(), 1);
        
        
        pic.advance_time(Duration::from_secs(31)); // for the updatestoragepositionlog
        pic.tick();
        
        for (p, pl) in [(p1, &token_position_position_log), (p2, &cycles_position_position_log)] {
        
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
            println!("total_pls_ser_size: {total_pls_ser_size}");
            assert_eq!(
                view_positions_storage_logs.len(),
                if total_pls_ser_size < FLUSH_STORAGE_BUFFER_AT_SIZE {
                    total_pls_ser_size    
                } else {
                    total_pls_ser_size % positions_flushes_at_len   
                }
            );
            
            let view_positions_storage_logs: Vec<u8> = pic.query_call(
                tc,
                Principal::anonymous(),
                "view_user_positions_logs",
                candid::encode_one(
                    ViewStorageLogsQuest::<Principal>{
                        opt_start_before_id: None,
                        index_key: Some(p)
                    }
                ).unwrap(),
            ).unwrap().unwrap();
            
            let logs_len = view_positions_storage_logs.len() / PositionLog::STABLE_MEMORY_SERIALIZE_SIZE;
            for i in 0..logs_len {
                let pl_here = pl_backwards(&view_positions_storage_logs[i*PositionLog::STABLE_MEMORY_SERIALIZE_SIZE..i*PositionLog::STABLE_MEMORY_SERIALIZE_SIZE+PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]);
                assert!(pl_here.position_termination.is_some());
            }    
        }
        
        pic.advance_time(Duration::from_secs(31));
        pic.tick();   
    }
}



fn create_and_download_state_snapshot<T: candid::CandidType + for<'a> Deserialize<'a>>(pic: &PocketIc, caller: Principal, canister: Principal, memory_id: u8) -> T {
    let (snapshot_len,): (u64,) = call_candid_as(&pic, canister, RawEffectivePrincipal::None, caller, "controller_create_state_snapshot", (memory_id,)).unwrap();
    let mut v = Vec::<u8>::new();
    let mut i = 0;
    let chunk_size_bytes = 512 * KiB * 3; 
    while (v.len() as u64) < snapshot_len {
        let (chunk,): (Vec<u8>,) = query_candid_as(&pic, canister, caller, "controller_download_state_snapshot", 
            (memory_id, v.len() as u64, std::cmp::min(chunk_size_bytes as u64, snapshot_len - v.len() as u64))
        ).unwrap(); 
        i = i + chunk.len();
        v.extend(chunk);
    }  
    assert_eq!(v.len(), snapshot_len as usize);
    candid::decode_one(&v).unwrap()    
}

fn pl_backwards(b: &[u8]) -> PositionLog {
    PositionLog::stable_memory_serialize_backwards(b)
}

fn tl_backwards(b: &[u8]) -> TradeLog {
    TradeLog::stable_memory_serialize_backwards(b)
}

fn create_token_position(pic: &PocketIc, user: Principal, tc: Principal, trade_tokens: Tokens, trade_rate: CyclesPerToken) -> u128/*position-id*/ {
    let _block = call_candid_as_::<_, (Result<u128, TransferError>,)>(
        &pic,
        ICP_LEDGER,
        user,
        "icrc1_transfer",
        (TransferArg{
            from_subaccount: None,
            to: Account{
                owner: tc,
                subaccount: Some(principal_token_subaccount(&user))
            },
            fee: Some(ICP_LEDGER_TRANSFER_FEE.into()),
            created_at_time: None,
            memo: None,
            amount: (trade_tokens + ICP_LEDGER_TRANSFER_FEE/*for the tc-collect-posit-transfer*/).into(),
        },)
    ).unwrap().0.unwrap();
    
    call_candid_as_::<_, (TradeResult,)>(&pic, tc, user, "trade_tokens", (
        TradeTokensQuest{
            tokens: trade_tokens,
            cycles_per_token_rate: trade_rate,
            posit_transfer_ledger_fee: Some(ICP_LEDGER_TRANSFER_FEE),
            return_tokens_to_subaccount: None,
            payout_cycles_to_subaccount: None,
        },
    )).unwrap().0.unwrap().position_id
}

fn create_cycles_position(pic: &PocketIc, user: Principal, tc: Principal, trade_cycles: Cycles, trade_rate: CyclesPerToken) -> u128/*position-id*/ {
    let _block = call_candid_as_::<_, (Result<u128, TransferError>,)>(
        &pic,
        BANK,
        user,
        "icrc1_transfer",
        (TransferArg{
            from_subaccount: None,
            to: Account{
                owner: tc,
                subaccount: Some(principal_token_subaccount(&user))
            },
            fee: Some(BANK_TRANSFER_FEE.into()),
            created_at_time: None,
            memo: None,
            amount: (trade_cycles + BANK_TRANSFER_FEE/*for the tc-collect-posit-transfer*/).into(),
        },)
    ).unwrap().0.unwrap();
    
    call_candid_as_::<_, (TradeResult,)>(&pic, tc, user, "trade_cycles", (
        TradeCyclesQuest{
            cycles: trade_cycles,
            cycles_per_token_rate: trade_rate,
            posit_transfer_ledger_fee: Some(BANK_TRANSFER_FEE),
            return_cycles_to_subaccount: None,
            payout_tokens_to_subaccount: None,
        },
    )).unwrap().0.unwrap().position_id    
}