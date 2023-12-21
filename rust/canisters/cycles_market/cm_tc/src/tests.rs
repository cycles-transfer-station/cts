use crate::*;

use pocket_ic::{*, common::rest::RawEffectivePrincipal};
use ic_icrc1_ledger::{LedgerArgument, InitArgsBuilder};
use candid::{Principal, Nat};
use cts_lib::{
    ic_cdk::api::call::CallResult,
    icrc::Tokens,
    types::{
        Cycles,
        CanisterCode,
        cycles_bank::{
            *,
        },
        cycles_market::{
            *,
            cm_main::*,
            tc::{*, position_log::*},
            
        }
    },
    consts::TRILLION,
    tools::principal_token_subaccount,
};

use icrc_ledger_types::icrc1::{
    account::Account,
    transfer::{TransferArg, TransferError},
};

const LEDGER_TRANSFER_FEE: u128 = 3;
const WASMS_DIR: &str = "../../../target/wasm32-unknown-unknown/debug/";
const CB_START_CYCLES_BALANCE: Cycles = 500_000 * TRILLION;


#[test]
fn t() {
    let pic = PocketIcBuilder::new()
        .with_nns_subnet()
        .with_fiduciary_subnet()
        .build();
    let _nns_subnet = pic.topology().get_nns().unwrap();
    let fid_subnet = pic.topology().get_fiduciary().unwrap();
    
    let (p1,p2): (Principal,Principal) = (
        Principal::from_slice(&[1,1,1,1,1]),
        Principal::from_slice(&[2,2,2,2,2]),
    );
    
    // create cbs
    let cb_wasm: Vec<u8> = std::fs::read(WASMS_DIR.to_owned() + "cycles_bank.wasm").unwrap();
    let (cb1,cb2) = {
        let v = [p1,p2].into_iter().map(|p| {
            let cb: Principal = pic.create_canister_on_subnet(None, None, fid_subnet);
            println!("cb: {cb}");
            pic.add_cycles(cb, 1_000_000 * TRILLION);
            pic.install_canister(
                cb, 
                cb_wasm.clone(), 
                candid::encode_one(
                    CyclesBankInit{
                        user_id: p,
                        cts_id: Principal::from_slice(&[]),
                        cbsm_id: Principal::from_slice(&[]),
                        storage_size_mib: 310,                         
                        lifetime_termination_timestamp_seconds: 60*60*24*365*100,
                        start_with_user_cycles_balance: CB_START_CYCLES_BALANCE,
                    }
                ).unwrap(), 
                None
            );
            cb                        
        }).collect::<Vec<Principal>>();
        (v[0], v[1])
    };
    
    // create ledger
    const CB_START_TOKEN_BALANCE: Tokens = 10000000000000000;
    let icrc1_ledger_wasm = std::fs::read("ic-icrc1-ledger-o-f99495f3772d5a85d25ef5008179b49a5f12c5c2.wasm").unwrap();
    let ledger: Principal = Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap();
    let ledger = pic.create_canister_with_id(None, None, ledger).unwrap();
    println!("ledger: {ledger}");
    pic.add_cycles(ledger, 1_000 * TRILLION);    
    pic.install_canister(
        ledger, 
        icrc1_ledger_wasm, 
        candid::encode_one(
            LedgerArgument::Init(
                InitArgsBuilder::for_tests()
                .with_initial_balance(
                    Account{
                        owner: cb1,
                        subaccount: None
                    },
                    CB_START_TOKEN_BALANCE,
                )
                .with_initial_balance(
                    Account{
                        owner: cb2,
                        subaccount: None
                    },
                    CB_START_TOKEN_BALANCE,
                )
                .with_transfer_fee(LEDGER_TRANSFER_FEE)
                .build()
            )
        ).unwrap(), 
        None
    );
    
    // create cm-main
    let cm_main_wasm: Vec<u8> = std::fs::read(WASMS_DIR.to_owned() + "cm_main.wasm").unwrap();
    let cm_main: Principal = pic.create_canister_on_subnet(None, None, fid_subnet);
    println!("cm_main: {cm_main}");    
    pic.add_cycles(cm_main, 1_000 * TRILLION);
    pic.install_canister(
        cm_main, 
        cm_main_wasm, 
        candid::encode_one(
            CMMainInit{
                cts_id: Principal::from_slice(&[]),
            }
        ).unwrap(), 
        None
    );
    
    // upload wasms onto the cm-main
    for (p, mct) in [
        ("cm_tc.wasm", MarketCanisterType::TradeContract),
        ("cm_trades_storage.wasm", MarketCanisterType::TradesStorage),
        ("cm_positions_storage.wasm", MarketCanisterType::PositionsStorage)
    ] {
        let wasm = std::fs::read(WASMS_DIR.to_owned() + p).unwrap();
        let _: () = call_candid(&pic, cm_main, RawEffectivePrincipal::None, "controller_upload_canister_code", (
            CanisterCode::new(wasm),
            mct
        )).unwrap();
    } 
    
    // create tc
    let (create_tc_result,): (Result<ControllerCreateIcrc1TokenTradeContractSuccess, ControllerCreateIcrc1TokenTradeContractError>,) 
    = call_candid(&pic, cm_main, RawEffectivePrincipal::None, "controller_create_trade_contract", (
        ControllerCreateIcrc1TokenTradeContractQuest{
            icrc1_ledger_id: ledger,
            icrc1_ledger_transfer_fee: LEDGER_TRANSFER_FEE
        }
    ,)).unwrap();
    let tc = create_tc_result.unwrap().trade_contract_canister_id;
    println!("tc: {tc}");

    let tc_id_and_ledger_id = TradeContractIdAndLedgerId{
        icrc1_ledger_canister_id: ledger,
        trade_contract_canister_id: tc,    
    };
    let tc_cb1_subaccount = Account{
        owner: tc,
        subaccount: Some(principal_token_subaccount(&cb1))
    };
    let tc_cb2_subaccount = Account{
        owner: tc,
        subaccount: Some(principal_token_subaccount(&cb2))
    };
    let tc_token_positions_subaccount = Account{
        owner: tc,
        subaccount: Some([5u8; 32]),
    };
    
    let trade_tokens: Tokens = 20_000;
    let trade_tokens_rate: CyclesPerToken = 1 * TRILLION / trade_tokens;
    let trade_cycles: Cycles = trade_tokens * trade_tokens_rate; 
    let cycles_payout_fee = trade_cycles / 10_000 * 50;    
    let tokens_payout_fee = cycles_payout_fee / trade_tokens_rate;
    
    for i in 0_u128..500 {
        pic.advance_time(core::time::Duration::from_secs(60));
        println!("{i}");
        
        let token_position_id: u128 = create_token_position(&pic, p1, cb1, tc, ledger, trade_tokens, trade_tokens_rate, LEDGER_TRANSFER_FEE);
        
        assert_eq!(token_position_id, i*2);
        assert!(icrc1_balance(&pic, ledger, &tc_cb1_subaccount).unwrap() == 0);
        assert!(icrc1_balance(&pic, ledger, &tc_token_positions_subaccount).unwrap() == trade_tokens + tokens_payout_fee * i);
        assert_eq!(
            icrc1_balance(&pic, ledger, &Account{owner: cb1, subaccount: None}).unwrap(), 
            CB_START_TOKEN_BALANCE - ((trade_tokens + (LEDGER_TRANSFER_FEE * 2))*(i+1)) 
        );
        
        {
            let cm_data: CMData = create_and_download_state_snapshot::<CMData>(&pic, cm_main, tc, 0);
            assert_eq!(
                cm_data.token_positions[0],
                TokenPosition{
                    id: i * 2,
                    positor: cb1,
                    quest: SellTokensQuest{
                        tokens: trade_tokens,
                        cycles_per_token_rate: trade_tokens_rate,
                        posit_transfer_ledger_fee: Some(LEDGER_TRANSFER_FEE),
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
        
        let cycles_position_id = create_cycles_position(&pic, p2, cb2, tc, ledger, trade_cycles, trade_tokens_rate);
        assert_eq!(cycles_position_id, i*2+1);
        
        let cycles_payout_fee = trade_cycles / 10_000 * 50;    
        let tokens_payout_fee = cycles_payout_fee / trade_tokens_rate;
        assert_eq!(
            icrc1_balance(&pic, ledger, &tc_token_positions_subaccount).unwrap(), 
            tokens_payout_fee + tokens_payout_fee*i 
        );
        assert_eq!(
            icrc1_balance(&pic, ledger, &Account{owner: cb1, subaccount: None}).unwrap(), 
            CB_START_TOKEN_BALANCE - ((trade_tokens + (LEDGER_TRANSFER_FEE * 2)) * (i+1))
        );
        assert_eq!(
            icrc1_balance(&pic, ledger, &Account{owner: cb2, subaccount: None}).unwrap(), 
            CB_START_TOKEN_BALANCE + ((trade_tokens - tokens_payout_fee - LEDGER_TRANSFER_FEE)*(i+1))
        );
        assert_eq!(
            cb_cycles_balance(&pic, cb1, p1),
            CB_START_CYCLES_BALANCE + ((trade_cycles - cycles_payout_fee)*(i+1))
        );   
        assert_eq!(
            cb_cycles_balance(&pic, cb2, p2),
            CB_START_CYCLES_BALANCE - (trade_cycles*(i+1))
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
        //if cm_data.trade_logs.len() == 0 {
        //if ((i as usize)+1) * 2 * PositionLog::STABLE_MEMORY_SERIALIZE_SIZE % positions_flushes_at_len != 0 {
        if ((i as usize)+1) * TradeLog::STABLE_MEMORY_SERIALIZE_SIZE % trades_flushes_at_len != 0 {
        //if trades_log_storage_data.storage_buffer.len() != 0 {
            tl_log_backwards = tl_backwards(&view_trades_storage_logs[view_trades_storage_logs.len() - TradeLog::STABLE_MEMORY_SERIALIZE_SIZE..]);
        } else {
            // call for the trades-storage-canisters view the log on the storage canister
            //for i in 0..10 { pic.tick(); }
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
            
        /*
        } else {
            println!("cm_data.trade_logs.len() != 0");
            panic!("");
            let position_pending_trades: Vec<u8> = pic.query_call(
                tc,
                Principal::anonymous(),
                "view_position_pending_trades",
                candid::encode_one(
                    ViewStorageLogsQuest::<Principal>{
                        opt_start_before_id: None,
                        index_key: None
                    }
                ).unwrap(),
            ).unwrap().unwrap();
            assert_eq!(position_pending_trades[position_pending_trades.len()-1], 1);
            assert_eq!(position_pending_trades[position_pending_trades.len()-2], 1);       
            tl_log_backwards = tl_backwards(&position_pending_trades[position_pending_trades.len()-2-TradeLog::STABLE_MEMORY_SERIALIZE_SIZE..position_pending_trades.len()-2]);  
        }
        */
        let trade_log = TradeLog{
            position_id_matcher: i*2 + 1,
            position_id_matchee: i*2,
            id: i,
            matchee_position_positor: cb1,
            matcher_position_positor: cb2,
            tokens: trade_tokens,
            cycles: trade_cycles,
            cycles_per_token_rate: trade_tokens_rate,
            matchee_position_kind: PositionKind::Token,
            timestamp_nanos: tl_log_backwards.timestamp_nanos,
            tokens_payout_fee: tokens_payout_fee,
            cycles_payout_fee: cycles_payout_fee,
            cycles_payout_lock: false,
            token_payout_lock: false,
            cycles_payout_data: CyclesPayoutData{
                cycles_payout: Some(true)
            },
            token_payout_data: TokenPayoutData{
                token_transfer: Some(TokenTransferData{
                    did_transfer: true,
                    ledger_transfer_fee: LEDGER_TRANSFER_FEE,
                })
            }
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
            positor: cb1,
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
            void_token_position_payout_ledger_transfer_fee: LEDGER_TRANSFER_FEE as u64,    
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
            positor: cb2,
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
            void_token_position_payout_ledger_transfer_fee: 0,    
        };
        
        assert_eq!(pl_2_payout_status, true);
        assert_eq!(
            pl_2,
            cycles_position_position_log
        );
        
        for (cb, pl) in [(cb1, &token_position_position_log), (cb2, &cycles_position_position_log)] {
            let void_positions_pending_b_cb: Vec<u8> = pic.query_call(
                tc,
                Principal::anonymous(),
                "view_void_positions_pending",
                candid::encode_one(
                    ViewStorageLogsQuest::<Principal>{
                        opt_start_before_id: None,
                        index_key: Some(cb)
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
                    if total_pls_ser_size % positions_flushes_at_len == 0 {
                        0
                    } else {
                        total_pls_ser_size % positions_flushes_at_len
                    }   
                }
            );
            
            let view_positions_storage_logs: Vec<u8> = pic.query_call(
                tc,
                Principal::anonymous(),
                "view_user_positions_logs",
                candid::encode_one(
                    ViewStorageLogsQuest::<Principal>{
                        opt_start_before_id: None,
                        index_key: Some(cb)
                    }
                ).unwrap(),
            ).unwrap().unwrap();
            
            let logs_len = view_positions_storage_logs.len() / PositionLog::STABLE_MEMORY_SERIALIZE_SIZE;
            for i in 0..logs_len {
                let pl_here = pl_backwards(&view_positions_storage_logs[i*PositionLog::STABLE_MEMORY_SERIALIZE_SIZE..i*PositionLog::STABLE_MEMORY_SERIALIZE_SIZE+PositionLog::STABLE_MEMORY_SERIALIZE_SIZE]);
                if logs_len == 1 || i == logs_len - 1 { 
                    assert_eq!(
                        pl_here.position_termination,
                        None,
                    );
                } else { 
                    assert!(pl_here.position_termination.is_some());    
                }
            } 
            
        }
        
        let cm_data: CMData = create_and_download_state_snapshot::<CMData>(&pic, cm_main, tc, 0);
        
        assert_eq!(
            cm_data.void_token_positions[0],
            VoidTokenPosition{
                position_id: i*2,
                tokens: 0,
                positor: cb1,
                token_payout_lock: false,
                token_payout_data: TokenPayoutData{
                    token_transfer: Some(TokenTransferData{
                        did_transfer: false,
                        ledger_transfer_fee: LEDGER_TRANSFER_FEE
                    })
                },
                timestamp_nanos: cm_data.void_token_positions[0].timestamp_nanos,
                update_storage_position_data: VPUpdateStoragePositionData{
                    lock: false,
                    status: false,
                    update_storage_position_log: token_position_position_log.clone(), 
                },    
            }
        );
        
        assert_eq!(
            cm_data.void_cycles_positions[0],
            VoidCyclesPosition{
                position_id: i*2 + 1,
                cycles: 0,
                positor: cb2,
                cycles_payout_lock: false,
                cycles_payout_data: CyclesPayoutData{
                    cycles_payout: Some(false),
                },
                timestamp_nanos: cm_data.void_cycles_positions[0].timestamp_nanos,
                update_storage_position_data: VPUpdateStoragePositionData{
                    lock: false,
                    status: false,
                    update_storage_position_log: cycles_position_position_log.clone()
                },    
            }
        );
        
        assert_eq!(cm_data.trade_logs.len(), 0);
        assert_eq!(cm_data.token_positions.len(), 0);
        assert_eq!(cm_data.cycles_positions.len(), 0);
        assert_eq!(cm_data.void_token_positions.len(), 1);
        assert_eq!(cm_data.void_cycles_positions.len(), 1);
        
        
        
    }
    
    
    
    
    // test with the flush-storage-buffer-at-size const set as the 500-bytes
    
    
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
    PositionLog{
        id: PositionId::from_be_bytes(b[2..18].try_into().unwrap()),
        positor: cts_lib::tools::thirty_bytes_as_principal(b[18..48].try_into().unwrap()),
        quest: CreatePositionQuestLog {
            quantity: u128::from_be_bytes(b[48..64].try_into().unwrap()),
            cycles_per_token_rate: u128::from_be_bytes(b[64..80].try_into().unwrap()),
        },
        position_kind: if b[80] == 0 { PositionKind::Cycles } else { PositionKind::Token },
        mainder_position_quantity: u128::from_be_bytes(b[81..97].try_into().unwrap()), 
        fill_quantity: u128::from_be_bytes(b[97..113].try_into().unwrap()), 
        fill_average_rate: CyclesPerToken::from_be_bytes(b[113..129].try_into().unwrap()),
        payouts_fees_sum: u128::from_be_bytes(b[129..145].try_into().unwrap()),
        creation_timestamp_nanos: u64::from_be_bytes(b[145..153].try_into().unwrap()) as u128,
        position_termination: if b[153] == 1 {
            Some(PositionTerminationData{
                timestamp_nanos: u64::from_be_bytes(b[154..162].try_into().unwrap()) as u128,
                cause: match b[162] {
                    0 => PositionTerminationCause::Fill,
                    1 => PositionTerminationCause::Bump,
                    2 => PositionTerminationCause::TimePass,
                    3 => PositionTerminationCause::UserCallVoidPosition,
                    _ => panic!("unknown PositionTerminationCause serialization"),
                }
            })
        } else { None },
        void_position_payout_dust_collection: b[163] == 1,
        void_token_position_payout_ledger_transfer_fee: u64::from_be_bytes(b[164..172].try_into().unwrap()),
    }
}

fn tl_backwards(b: &[u8]) -> TradeLog {
    TradeLog{
        position_id_matcher: u128::from_be_bytes(b[191..207].try_into().unwrap()),
        position_id_matchee: u128::from_be_bytes(b[2..18].try_into().unwrap()),
        id: u128::from_be_bytes(b[18..34].try_into().unwrap()),
        matchee_position_positor: cts_lib::tools::thirty_bytes_as_principal(b[34..64].try_into().unwrap()),
        matcher_position_positor: cts_lib::tools::thirty_bytes_as_principal(b[64..94].try_into().unwrap()),
        tokens: u128::from_be_bytes(b[94..110].try_into().unwrap()),
        cycles: u128::from_be_bytes(b[110..126].try_into().unwrap()),
        cycles_per_token_rate: u128::from_be_bytes(b[126..142].try_into().unwrap()),
        matchee_position_kind: if b[142] == 0 { PositionKind::Cycles } else { PositionKind::Token },
        timestamp_nanos: u128::from_be_bytes(b[143..159].try_into().unwrap()),
        tokens_payout_fee: u128::from_be_bytes(b[159..175].try_into().unwrap()),
        cycles_payout_fee: u128::from_be_bytes(b[175..191].try_into().unwrap()),
        cycles_payout_lock: false,
        token_payout_lock: false,
        cycles_payout_data: CyclesPayoutData{
            cycles_payout: Some(b[223] == 0)
        },
        token_payout_data: TokenPayoutData{
            token_transfer: Some(TokenTransferData{
                did_transfer: b[224] == 0,
                ledger_transfer_fee: u128::from_be_bytes(b[207..223].try_into().unwrap()),
            })
        }
    }  
}


trait UnwrapWasmResult {
    fn unwrap(self) -> Vec<u8>;
}
impl UnwrapWasmResult for WasmResult {
    fn unwrap(self) -> Vec<u8> {
        match self {
            WasmResult::Reply(b) => b,
            WasmResult::Reject(s) => panic!("WasmResult::Reject({s})"),
        }
    }    
}

fn cb_cycles_balance(pic: &PocketIc, cb: Principal, user: Principal) -> Cycles {
    let (cb_cycles_balance,): (Cycles,) = call_candid_as(&pic, cb, RawEffectivePrincipal::None, user, "cycles_balance", ()).unwrap(); 
    cb_cycles_balance    
}

fn icrc1_balance(pic: &PocketIc, ledger: Principal, countid: &Account) -> Result<u128, pocket_ic::CallError> {
    call_candid(
        pic,
        ledger,
        RawEffectivePrincipal::None,
        "icrc1_balance_of",
        (countid,),
    ).map(|t: (u128,)| t.0)
}

fn create_token_position(pic: &PocketIc, user: Principal, cb: Principal, tc: Principal, ledger: Principal, trade_tokens: Tokens, trade_rate: CyclesPerToken, ledger_transfer_fee: Tokens) -> u128/*position-id*/ {
    let (transfer_cb_sponse,): (CallResult<Vec<u8>>,) = call_candid_as(
        &pic,
        cb,
        RawEffectivePrincipal::None,
        user,
        "transfer_icrc1",
        (
            ledger,
            candid::encode_one(
                TransferArg{
                    from_subaccount: None,
                    to: Account{
                        owner: tc,
                        subaccount: Some(principal_token_subaccount(&cb))
                    },
                    fee: Some(ledger_transfer_fee.into()),
                    created_at_time: None,
                    memo: None,
                    amount: (trade_tokens + ledger_transfer_fee).into(),
                }
            ).unwrap()
        )
    ).unwrap();
    let _block = candid::decode_one::<Result<Nat, TransferError>>(&transfer_cb_sponse.unwrap()).unwrap().unwrap();
    let (cb_trade_tokens_result,): (CBTradeTokensResult,) = call_candid_as(
        &pic,
        cb,
        RawEffectivePrincipal::None,
        user,
        "cm_trade_tokens",
        (
            TradeContractIdAndLedgerId{
                icrc1_ledger_canister_id: ledger,
                trade_contract_canister_id: tc,    
            }, 
            SellTokensQuest{
                tokens: trade_tokens,
                cycles_per_token_rate: trade_rate,
                posit_transfer_ledger_fee: Some(ledger_transfer_fee),
            }
        )
    ).unwrap();
    let token_position_id: u128 = cb_trade_tokens_result.unwrap().unwrap().position_id;
    token_position_id
}

fn create_cycles_position(pic: &PocketIc, user: Principal, cb: Principal, tc: Principal, ledger: Principal, trade_cycles: Cycles, trade_rate: CyclesPerToken) -> u128/*position-id*/ {
    let (cb_trade_cycles_result,): (CBTradeCyclesResult,) = call_candid_as(
        &pic,
        cb,
        RawEffectivePrincipal::None,
        user,
        "cm_trade_cycles",
        (
            TradeContractIdAndLedgerId{
                icrc1_ledger_canister_id: ledger,
                trade_contract_canister_id: tc,    
            }, 
            BuyTokensQuest{
                cycles: trade_cycles,
                cycles_per_token_rate: trade_rate,
            }
        )
    ).unwrap();
    let cycles_position_id = cb_trade_cycles_result.unwrap().unwrap().position_id;
    cycles_position_id
}


