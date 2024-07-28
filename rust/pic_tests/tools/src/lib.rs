use pocket_ic::{*, common::rest::RawEffectivePrincipal};
use candid::{Nat, Principal, CandidType, Deserialize};
use std::collections::{HashSet, HashMap};
use cts_lib::{
    consts::{TRILLION, KiB, NANOS_IN_A_SECOND, SECONDS_IN_A_DAY, SECONDS_IN_A_HOUR},
    tools::principal_token_subaccount,
    types::{
        cm::cm_main::*,
        Cycles,
        CanisterCode,
        fueler::{self, FuelerData},
        
    },
};
use icrc_ledger_types::icrc1::{account::Account, transfer::{TransferArg, TransferError, BlockIndex}};


pub mod bank; 
pub mod tc;

pub const ICP_LEDGER_TRANSFER_FEE: u128 = 10_000;
pub const CMC_RATE: u128 = 55555;
pub const ICP_MINTER: Principal = Principal::from_slice(b"icp-minter");
pub const CMC: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,4,1,1]);
pub const NNS_GOVERNANCE: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,1,1,1]);
pub const ICP_LEDGER: Principal = Principal::from_slice(&[0,0,0,0,0,0,0,2,1,1]);
pub const CTS_CONTROLLER: Principal = SNS_ROOT; // Principal::from_slice(&[0,1,2,3,4,5,6,7,8,9]);
pub const CTS: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 0, 110, 1, 1]);
pub const CM_MAIN: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 0, 111, 1, 1]);
pub const BANK: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 0, 170, 1, 1]);
pub const FUELER: Principal = Principal::from_slice(&[0,0,0,0,2,48,0,191,1,1]); // update this when mainnet canister live.
pub const SNS_ROOT: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 0, 0, 218, 1, 1]) ; // ibahq-taaaa-aaaaq-aadna-cai
pub const SNS_GOVERNANCE: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 0, 0, 219, 1, 1]); // igbbe-6yaaa-aaaaq-aadnq-cai
pub const SNS_LEDGER: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 0, 0, 220, 1, 1]);
pub const SNS_LEDGER_INDEX: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 0, 0, 222, 1, 1]);
pub const SNS_SWAP: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 0, 0, 221, 1, 1]);

pub const START_WITH_FUEL: u128 = fueler::FUEL_TOPUP_TRIGGER_THRESHOLD - 1;

use std::path::PathBuf;


/*
fn workspace_dir() -> PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}
*/
pub fn workspace_dir() -> PathBuf {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d = d.parent().unwrap().parent().unwrap().to_path_buf();
    d
}

pub fn wasms_dir() -> PathBuf {
    let mut d = workspace_dir();
    d.push("target/wasm32-unknown-unknown/debug");
    d
}

pub fn pic_get_time_nanos(pic: &PocketIc) -> u128 {
    pic.get_time().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
}

pub fn icrc1_transfer(pic: &PocketIc, ledger: Principal, owner: Principal, q: TransferArg) -> Result<BlockIndex, TransferError> {
    call_candid_as::<_, (Result<BlockIndex, TransferError>,)>(pic, ledger, RawEffectivePrincipal::None, owner, "icrc1_transfer", (q,)).unwrap().0
}

pub fn icrc1_balance(pic: &PocketIc, ledger: Principal, countid: &Account) -> u128 {
    call_candid(
        pic,
        ledger,
        RawEffectivePrincipal::None,
        "icrc1_balance_of",
        (countid,),
    ).map(|t: (u128,)| t.0).unwrap()
}

pub fn mint_icp(pic: &PocketIc, to: &Account, amount: u128) {
    let (mint_icp_r,): (Result<Nat, TransferError>,) = call_candid_as_(
        pic,
        ICP_LEDGER,
        ICP_MINTER,            
        "icrc1_transfer",
        (TransferArg{
            from_subaccount: None,
            to: to.clone(),
            fee: None,
            created_at_time: None,
            memo: None,
            amount: amount.into(),
        },)
    ).unwrap();
    mint_icp_r.unwrap();
}

pub fn create_and_download_state_snapshot<T: candid::CandidType + for<'a> Deserialize<'a>>(pic: &PocketIc, caller: Principal, canister: Principal, memory_id: u8) -> T {
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

pub fn call_candid_as_<Input, Output>(
    env: &PocketIc,
    canister_id: Principal,
    sender: Principal,
    method: &str,
    input: Input
) -> Result<Output, pocket_ic::CallError>
where
    Input: candid::utils::ArgumentEncoder,
    Output: for<'a> candid::utils::ArgumentDecoder<'a>,
{
    call_candid_as(env, canister_id, RawEffectivePrincipal::None, sender, method, input)
}

pub fn call_candid_<Input, Output>(
    env: &PocketIc,
    canister_id: Principal,
    method: &str,
    input: Input
) -> Result<Output, pocket_ic::CallError>
where
    Input: candid::utils::ArgumentEncoder,
    Output: for<'a> candid::utils::ArgumentDecoder<'a>,
{
    call_candid(env, canister_id, RawEffectivePrincipal::None, method, input)
}

pub trait WasmResultUnwrap {
    fn unwrap(self) -> Vec<u8>;
}
impl WasmResultUnwrap for WasmResult {
    fn unwrap(self) -> Vec<u8> {
        match self {
            WasmResult::Reply(b) => b,
            WasmResult::Reject(s) => panic!("{}", s),
        }
    }
}


pub fn range_radius(n: u128, radius: u128) -> std::ops::Range<u128> {
    (n - radius)..(n + radius)
}


// private
fn create_ledger_(pic: &PocketIc, symbol: &str, name: &str, opt_canister_id: Option<Principal>) -> Principal {

    let canister = if let Some(canister_id) = opt_canister_id {
        canister_id
    } else {
        pic.create_canister()
    };

    pic.add_cycles(canister, 1_000 * TRILLION);

    #[derive(CandidType, Deserialize)]
    enum IcpLedgerPayload {
        Init(IcpLedgerInitArgs)
    }
    #[derive(CandidType, Deserialize)]
    struct IcpLedgerFeatureFlags {
        icrc2: bool
    }
    #[derive(CandidType, Deserialize)]
    struct IcpLedgerInitArgs {
        minting_account: String,
        icrc1_minting_account: Option<Account>,
        initial_values: HashMap<String, ic_ledger_types::Tokens>,
        send_whitelist: HashSet<Principal>,
        transfer_fee: Option<ic_ledger_types::Tokens>,
        token_symbol: Option<String>,
        token_name: Option<String>,
        feature_flags: Option<IcpLedgerFeatureFlags>,
    }
    let ledger_wasm = std::fs::read(workspace_dir().join("pic_tests/pre-built-wasms/ledger-canister-o-98eb213581b239c3829eee7076bea74acad9937b.wasm.gz")).unwrap();
    pic.install_canister(
        canister,
        ledger_wasm,
        candid::encode_one(
            IcpLedgerPayload::Init(
                IcpLedgerInitArgs{
                    minting_account: ic_ledger_types::AccountIdentifier::new(&ICP_MINTER, &ic_ledger_types::DEFAULT_SUBACCOUNT).to_hex(),
                    icrc1_minting_account: Some(Account{owner: ICP_MINTER, subaccount: None}),
                    initial_values: HashMap::from([
                        (
                            "5b315d2f6702cb3a27d826161797d7b2c2e131cd312aece51d4d5574d1247087".to_string(),
                            ic_ledger_types::Tokens::from_e8s(10_000_000_000_000_000)
                        ),
                    ]),
                    send_whitelist: HashSet::new(),
                    transfer_fee: Some(ic_ledger_types::Tokens::from_e8s(ICP_LEDGER_TRANSFER_FEE as u64)),
                    token_symbol: Some(symbol.to_string()),
                    token_name: Some(name.to_string()),
                    feature_flags: Some(IcpLedgerFeatureFlags{ icrc2: true }),
                }
            )
        ).unwrap(),
        None
    );
    canister
}



pub fn set_up() -> PocketIc {
    
    // set pic binary location if not set
    const POCKET_IC_BIN_VARNAME: &'static str = "POCKET_IC_BIN";
    if std::env::var(POCKET_IC_BIN_VARNAME).is_err() {
        println!("setting {} environment variable", POCKET_IC_BIN_VARNAME);
        std::env::set_var(POCKET_IC_BIN_VARNAME, workspace_dir().join("pic_tests/pocket-ic"));
    }
    
    let pic = PocketIcBuilder::new()
        .with_nns_subnet()
        .with_fiduciary_subnet()
        .with_sns_subnet()
        .build();

    // ICP-LEDGER
    let icp_ledger = pic.create_canister_with_id(None, None, ICP_LEDGER).unwrap();
    create_ledger_(&pic, "ICP", "Internet-Computer", Some(icp_ledger));
    
    // CMC
    let nns_governance = NNS_GOVERNANCE;
    let cmc_wasm = std::fs::read(workspace_dir().join("pic_tests/pre-built-wasms/cmc-o-14e0b0adf6632a6225cb1b0a22d4bafce75eb81e.wasm.gz")).unwrap();
    let cmc = pic.create_canister_with_id(None, None, CMC).unwrap();
    pic.add_cycles(cmc, 1_000 * TRILLION);    
    pic.install_canister(
        cmc, 
        cmc_wasm, 
        candid::encode_one(
            {
                #[derive(CandidType, Deserialize)]
                struct Ia {
                    ledger_canister_id: Option<Principal>,
                    governance_canister_id: Option<Principal>,
                    minting_account_id: Option<String>,
                    last_purged_notification: Option<u64>,
                }
                Ia{
                    ledger_canister_id: Some(icp_ledger),
                    governance_canister_id: Some(nns_governance),
                    minting_account_id: Some(ic_ledger_types::AccountIdentifier::new(&ICP_MINTER, &ic_ledger_types::DEFAULT_SUBACCOUNT).to_hex()),
                    last_purged_notification: Some(0),
                }
            }
        ).unwrap(), 
        None
    );
       
    let cmc_rate: u128 = CMC_RATE;
    #[derive(CandidType, Deserialize)]
    struct UpdateIcpXdrConversionRatePayload {
        data_source: String,
        timestamp_seconds: u64,
        xdr_permyriad_per_icp: u64,
    }
    let (r,): (Result<(), String>,) = call_candid_as(
        &pic,
        cmc,
        RawEffectivePrincipal::None,
        nns_governance,
        "set_icp_xdr_conversion_rate",
        (UpdateIcpXdrConversionRatePayload {
            data_source: "".to_string(),
            timestamp_seconds: u64::MAX, //pic.get_time().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 5*60,
            xdr_permyriad_per_icp: cmc_rate as u64,
        },)
    ).unwrap();
    r.unwrap();
    
    // SNS-ROOT
    pic.create_canister_with_id(Some(SNS_GOVERNANCE), None, SNS_ROOT).unwrap();
    pic.add_cycles(SNS_ROOT, START_WITH_FUEL);
    let sns_root_wasm = std::fs::read(workspace_dir().join("pic_tests/pre-built-wasms/sns-root-canister-e790c6636115482db53ca3daa2f1900202ab04cf.wasm.gz")).unwrap();
    pic.install_canister(
        SNS_ROOT,
        sns_root_wasm, 
        candid::encode_one(
            outsiders::sns_root::SnsRootCanister {
                dapp_canister_ids: vec![CTS, BANK, CM_MAIN, FUELER],
                testflight: false,
                latest_ledger_archive_poll_timestamp_seconds: None,
                archive_canister_ids: vec![],
                governance_canister_id: Some(SNS_GOVERNANCE),
                index_canister_id: Some(SNS_LEDGER_INDEX),
                swap_canister_id: Some(SNS_SWAP),
                ledger_canister_id: Some(SNS_LEDGER),
            }
        ).unwrap(), 
        Some(SNS_GOVERNANCE), 
    );
    
    // SNS_GOVERANANCE
    pic.create_canister_with_id(Some(SNS_ROOT), None, SNS_GOVERNANCE).unwrap();
    pic.add_cycles(SNS_GOVERNANCE, START_WITH_FUEL);
    
    // SNS_SWAP
    pic.create_canister_with_id(Some(SNS_ROOT), None, SNS_SWAP).unwrap();
    pic.add_cycles(SNS_SWAP, START_WITH_FUEL);
    let sns_swap_wasm = std::fs::read(workspace_dir().join("pic_tests/pre-built-wasms/sns-swap-canister-b39f782ae9e976f6f25c8f1d75b977bd22c81507.wasm.gz")).unwrap();
    pic.install_canister(
        SNS_SWAP,
        sns_swap_wasm, 
        candid::encode_one(
            outsiders::sns_swap::Init {
                nns_proposal_id: Some(1234),
                sns_root_canister_id: SNS_ROOT.to_text(),
                neurons_fund_participation: None,
                min_participant_icp_e8s: Some(5_00000000),
                neuron_basket_construction_parameters: Some(
                    outsiders::sns_swap::NeuronBasketConstructionParameters {
                        dissolve_delay_interval_seconds: (SECONDS_IN_A_DAY * 365 + SECONDS_IN_A_HOUR * 6) as u64,
                        count: 4,
                    }
                ),
                fallback_controller_principal_ids: vec![SNS_ROOT.to_text()], // one is required
                max_icp_e8s: None,
                neuron_minimum_stake_e8s: Some(100_00000000),
                confirmation_text: None,
                swap_start_timestamp_seconds: None,
                swap_due_timestamp_seconds: Some((pic_get_time_nanos(&pic) / NANOS_IN_A_SECOND + SECONDS_IN_A_DAY * 14) as u64),
                min_participants: Some(5),
                sns_token_e8s: Some(10_000_000_00000000),
                nns_governance_canister_id: NNS_GOVERNANCE.to_text(),
                transaction_fee_e8s: Some(100000),
                icp_ledger_canister_id: ICP_LEDGER.to_text(),
                sns_ledger_canister_id: SNS_LEDGER.to_text(),
                neurons_fund_participation_constraints: None,
                neurons_fund_participants: None,
                should_auto_finalize: Some(false),
                max_participant_icp_e8s: Some(10_00000000),
                sns_governance_canister_id: SNS_GOVERNANCE.to_text(),
                min_direct_participation_icp_e8s: Some(10_000_00000000),
                restricted_countries: None,
                min_icp_e8s: None,
                max_direct_participation_icp_e8s: Some(50_000_00000000),
            }
        ).unwrap(), 
        Some(SNS_ROOT), 
    );
    
    
    // SNS_LEDGER
    pic.create_canister_with_id(Some(SNS_ROOT), None, SNS_LEDGER).unwrap();
    pic.add_cycles(SNS_LEDGER, START_WITH_FUEL);
    
    //  SNS_LEDGER_INDEX
    pic.create_canister_with_id(Some(SNS_ROOT), None, SNS_LEDGER_INDEX).unwrap();
    pic.add_cycles(SNS_LEDGER_INDEX, START_WITH_FUEL);
    
    // CTS
    pic.create_canister_with_id(Some(SNS_ROOT), None, CTS).unwrap();
    pic.add_cycles(CTS, START_WITH_FUEL);
    
    // BANK
    pic.create_canister_with_id(Some(SNS_ROOT), None, BANK).unwrap();
    pic.add_cycles(BANK, START_WITH_FUEL);
    pic.install_canister(
        BANK, 
        std::fs::read(wasms_dir().join("bank.wasm")).unwrap(), 
        candid::encode_args(()).unwrap(), 
        Some(SNS_ROOT), 
    );

    // CM_MAIN
    pic.create_canister_with_id(Some(SNS_ROOT), None, CM_MAIN).unwrap();
    pic.add_cycles(CM_MAIN, START_WITH_FUEL);
    pic.install_canister(
        CM_MAIN, 
        std::fs::read(wasms_dir().join("cm_main.wasm")).unwrap(), 
        candid::encode_one(CMMainInit {
            cts_id: CTS,
            cycles_bank_id: BANK,
        }).unwrap(), 
        Some(SNS_ROOT), 
    );
    
    pic
}

// icp tc
pub fn set_up_tc(pic: &PocketIc) -> Principal {
    for (wasm_path, market_canister_type) in [
        ("cm_tc.wasm", MarketCanisterType::TradeContract),
        ("cm_positions_storage.wasm", MarketCanisterType::PositionsStorage),
        ("cm_trades_storage.wasm", MarketCanisterType::TradesStorage),
    ] {
        let cc = CanisterCode::new(std::fs::read(wasms_dir().join(wasm_path)).unwrap());
        call_candid_as::<_, ()>(&pic, CM_MAIN, RawEffectivePrincipal::None, SNS_GOVERNANCE, "controller_upload_canister_code", (cc, market_canister_type)).unwrap();
    }

    pic.add_cycles(CM_MAIN, NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES);

    let tc = call_candid_as::<_, (Result<ControllerCreateIcrc1TokenTradeContractSuccess, ControllerCreateIcrc1TokenTradeContractError>,)>(
        &pic, CM_MAIN, RawEffectivePrincipal::None, SNS_GOVERNANCE, "controller_create_trade_contract", (
            ControllerCreateIcrc1TokenTradeContractQuest {
                icrc1_ledger_id: ICP_LEDGER,
                icrc1_ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE,
            },
        )
    ).unwrap().0.unwrap().trade_contract_canister_id;
    println!("tc: {}", tc);
    tc
}

pub fn set_up_new_ledger_and_tc(pic: &PocketIc) -> (Principal, Principal)/*(ledger, tc)*/ {

    let tc_i: usize = call_candid::<(), (Vec<(TradeContractIdAndLedgerId, TradeContractData)>,)>(
        &pic, CM_MAIN, RawEffectivePrincipal::None, "view_icrc1_token_trade_contracts", ()
    ).unwrap().0.len();

    let ledger = create_ledger_(pic, &format!("TKN{}", tc_i), &format!("Token{}", tc_i), None);

    pic.add_cycles(CM_MAIN, NEW_ICRC1TOKEN_TRADE_CONTRACT_CYCLES);

    let tc = call_candid_as_::<_, (Result<ControllerCreateIcrc1TokenTradeContractSuccess, ControllerCreateIcrc1TokenTradeContractError>,)>(
        &pic, CM_MAIN, SNS_GOVERNANCE, "controller_create_trade_contract", (
            ControllerCreateIcrc1TokenTradeContractQuest {
                icrc1_ledger_id: ledger,
                icrc1_ledger_transfer_fee: ICP_LEDGER_TRANSFER_FEE,
            },
        )
    ).unwrap().0.unwrap().trade_contract_canister_id;
    println!("ledger: {}, tc: {}", ledger, tc);
    (ledger, tc)
}


pub fn set_up_fueler(pic: &PocketIc) -> Principal {
    pic.create_canister_with_id(Some(SNS_ROOT), None, FUELER).unwrap();
    pic.add_cycles(FUELER, START_WITH_FUEL);
    pic.install_canister(
        FUELER,
        std::fs::read(wasms_dir().join("fueler.wasm")).unwrap(),
        candid::encode_one(
            FuelerData{
                sns_root: SNS_ROOT,
                cm_main: CM_MAIN,
                cts_cycles_bank: BANK,
            }
        ).unwrap(),
        Some(SNS_ROOT)
    );
    FUELER
}


pub fn set_up_canister_caller(pic: &PocketIc) -> Principal {
    let canister_caller: Principal = pic.create_canister();
    let canister_caller_wasm: Vec<u8> = std::fs::read(wasms_dir().join("canister_caller.wasm")).unwrap();
    pic.add_cycles(canister_caller, 1_000_000_000 * TRILLION);
    pic.install_canister(
        canister_caller, 
        canister_caller_wasm, 
        candid::encode_args(()).unwrap(),
        None,
    );
    canister_caller
}

