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
const WASMS_DIR: &str = "../rust/target/wasm32-unknown-unknown/release/";
const CB_START_CYCLES_BALANCE: Cycles = 500_000 * TRILLION;

#[test]
fn cm_test() {
    let pic = PocketIcBuilder::new()
        .with_nns_subnet()
        .with_fiduciary_subnet()
        .build();
    let _nns_subnet = pic.topology().get_nns().unwrap();
    let fid_subnet = pic.topology().get_fiduciary().unwrap();
    
    // create cb
    let cb_wasm: Vec<u8> = std::fs::read(WASMS_DIR.to_owned() + "cycles_bank.wasm").unwrap();
    let cb: Principal = pic.create_canister_on_subnet(None, None, fid_subnet);
    println!("cb: {cb}");
    pic.add_cycles(cb, 1_000_000 * TRILLION);
    pic.install_canister(
        cb, 
        cb_wasm, 
        candid::encode_one(
            CyclesBankInit{
                user_id: Principal::from_slice(&[4]),
                cts_id: Principal::from_slice(&[]),
                cbsm_id: Principal::from_slice(&[]),
                storage_size_mib: 310,                         
                lifetime_termination_timestamp_seconds: 60*60*24*365*100,
                start_with_user_cycles_balance: CB_START_CYCLES_BALANCE,
            }
        ).unwrap(), 
        None
    );
    
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
                        owner: cb,
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
    let cm_main_wasm: Vec<u8> = std::fs::read("../rust/target/wasm32-unknown-unknown/release/cm_main.wasm").unwrap();
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
    
    let tc_cb_subaccount = Account{
        owner: tc,
        subaccount: Some(principal_token_subaccount(&cb))
    };
    
    let tc_token_positions_subaccount = Account{
        owner: tc,
        subaccount: Some([5u8; 32]),
    };
    
    // create cm-token-position
    let trade_tokens: Tokens = 20_000;
    
    let (transfer_cb_sponse,): (CallResult<Vec<u8>>,) = call_candid(
        &pic,
        cb,
        RawEffectivePrincipal::None,
        "transfer_icrc1",
        (
            ledger,
            candid::encode_one(
                TransferArg{
                    from_subaccount: None,
                    to: tc_cb_subaccount,
                    fee: Some(LEDGER_TRANSFER_FEE.into()),
                    created_at_time: None,
                    memo: None,
                    amount: (trade_tokens + LEDGER_TRANSFER_FEE).into(),
                }
            ).unwrap()
        )
    ).unwrap();
    let block = candid::decode_one::<Result<Nat, TransferError>>(&transfer_cb_sponse.unwrap()).unwrap().unwrap();
    println!("transfer at block: {block}");
    
    let trade_tokens_rate: CyclesPerToken = 1 * TRILLION / trade_tokens;
    
    let (cb_trade_tokens_result,): (CBTradeTokensResult,) = call_candid(
        &pic,
        cb,
        RawEffectivePrincipal::None,
        "cm_trade_tokens",
        (
            tc_id_and_ledger_id, 
            SellTokensQuest{
                tokens: trade_tokens,
                cycles_per_token_rate: trade_tokens_rate,
                posit_transfer_ledger_fee: Some(LEDGER_TRANSFER_FEE),
            }
        )
    ).unwrap();
    
    let token_position_id: u128 = cb_trade_tokens_result.unwrap().unwrap().position_id;
    println!("token_position_id: {token_position_id}");
    
    assert!(icrc1_balance(&pic, ledger, &tc_cb_subaccount).unwrap() == 0);
    assert!(icrc1_balance(&pic, ledger, &tc_token_positions_subaccount).unwrap() == trade_tokens);
    assert!(icrc1_balance(&pic, ledger, &Account{owner: cb, subaccount: None}).unwrap() == CB_START_TOKEN_BALANCE - trade_tokens - (LEDGER_TRANSFER_FEE * 2));
    
    // download_state_snapshot, make sure cm-tc has the token-position in its token_positions list.
    //let snapshot_len:u64 = call_candid()
            
            
    // make a trade. 
    let trade_cycles: Cycles = trade_tokens * trade_tokens_rate; 
    let (cb_trade_cycles_result,): (CBTradeCyclesResult,) = call_candid(
        &pic,
        cb,
        RawEffectivePrincipal::None,
        "cm_trade_cycles",
        (
            tc_id_and_ledger_id, 
            BuyTokensQuest{
                cycles: trade_cycles,
                cycles_per_token_rate: trade_tokens_rate,
            }
        )
    ).unwrap();
    let cycles_position_id = cb_trade_cycles_result.unwrap().unwrap().position_id;
    println!("cycles_position_id: {cycles_position_id}");
    
    let cycles_payout_fee = trade_cycles / 10_000 * 50;    
    let tokens_payout_fee = cycles_payout_fee / trade_tokens_rate;
    assert_eq!(
        icrc1_balance(&pic, ledger, &tc_token_positions_subaccount).unwrap(), 
        tokens_payout_fee
    );
    assert_eq!(
        icrc1_balance(&pic, ledger, &Account{owner: cb, subaccount: None}).unwrap(), 
        CB_START_TOKEN_BALANCE - (LEDGER_TRANSFER_FEE * 2) - tokens_payout_fee - LEDGER_TRANSFER_FEE
    );
    assert_eq!(
        cb_cycles_balance(&pic, cb),
        CB_START_CYCLES_BALANCE - cycles_payout_fee
    );   
    
    let void_positions_pending_b: Vec<u8> = pic.query_call(
        tc,
        Principal::anonymous(),
        "view_void_positions_pending",
        candid::encode_one(
            ViewStorageLogsQuest{
                opt_start_before_id: None,
                index_key: Some(cb)
            }
        ).unwrap(),
    ).unwrap().unwrap();
    
    assert_eq!(
        void_positions_pending_b.len(),
        (position_log::STABLE_MEMORY_SERIALIZE_SIZE + 1) * 2,
    );
    
    let (pl_1, pl_1_payout_status): (PositionLog, bool) = (
        pl_backwards(&void_positions_pending_b[0..position_log::STABLE_MEMORY_SERIALIZE_SIZE]),
        void_positions_pending_b[position_log::STABLE_MEMORY_SERIALIZE_SIZE] == 1,
    );
    
    assert_eq!(pl_1_payout_status, true);
    assert_eq!(
        pl_1,
        PositionLog{
            id: 0,
            positor: cb,
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
        }
    );
    
     let (pl_2, pl_2_payout_status): (PositionLog, bool) = (
        pl_backwards(&void_positions_pending_b[
            position_log::STABLE_MEMORY_SERIALIZE_SIZE + 1
            ..
            position_log::STABLE_MEMORY_SERIALIZE_SIZE + 1 + position_log::STABLE_MEMORY_SERIALIZE_SIZE
        ]),
        void_positions_pending_b[(position_log::STABLE_MEMORY_SERIALIZE_SIZE * 2) + 1] == 1,
    );
    
    assert_eq!(pl_2_payout_status, true);
    assert_eq!(
        pl_2,
        PositionLog{
            id: 1,
            positor: cb,
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
        }
    );



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




fn cb_cycles_balance(pic: &PocketIc, cb: Principal) -> Cycles {
    let (cb_cycles_balance,): (Cycles,) = call_candid(&pic, cb, RawEffectivePrincipal::None, "cycles_balance", ()).unwrap(); 
    cb_cycles_balance    
}

fn icrc1_balance(pic: &PocketIc, ledger: Principal, countid: &Account) -> Result<u128, CallError> {
    call_candid(
        pic,
        ledger,
        RawEffectivePrincipal::None,
        "icrc1_balance_of",
        (countid,),
    ).map(|t: (u128,)| t.0)
}





