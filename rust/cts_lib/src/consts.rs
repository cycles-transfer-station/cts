use crate::{
    ic_ledger_types::{
        IcpMemo
    },
    types::{
        Cycles
    }
};
use candid::Principal;
 
   
#[allow(non_upper_case_globals)]
pub const KiB: usize = 1024;
#[allow(non_upper_case_globals)]
pub const MiB: usize = KiB * 1024;
#[allow(non_upper_case_globals)]
pub const GiB: usize = MiB * 1024;

pub const NANOS_IN_A_SECOND: u128 = 1_000_000_000;
pub const SECONDS_IN_A_MINUTE: u128 = 60; 
pub const SECONDS_IN_A_HOUR: u128 = SECONDS_IN_A_MINUTE * 60;
pub const SECONDS_IN_A_DAY: u128 = SECONDS_IN_A_HOUR * 24;
pub const MINUTES_IN_A_HOUR: u128 = 60;

pub const WASM_PAGE_SIZE_BYTES: usize = 65536; // 2^16 // 64KiB


pub const MANAGEMENT_CANISTER_ID: Principal = Principal::management_canister();

pub const BILLION: u64 = 1_000_000_000;
pub const TRILLION: u128 = 1_000_000_000_000;

pub const CYCLES_PER_XDR: Cycles = TRILLION; // 1T cycles = 1 XDR

pub const NETWORK_CANISTER_CREATION_FEE_CYCLES_13_NODE_SUBNET: Cycles = 100_000_000_000;

#[allow(non_upper_case_globals)]
pub const NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES_13_NODE_SUBNET: Cycles = 127_000;             // 4 SDR per GiB per year => 4e12 Cycles per year

pub const FIDUCIARY_SUBNET_SIZE: u128 = 28;

const fn fiduciary_subnet_cost(thirteen_node_subnet_cost: Cycles) -> Cycles {
    thirteen_node_subnet_cost * FIDUCIARY_SUBNET_SIZE / 13 
} 

pub const NETWORK_CANISTER_CREATION_FEE_CYCLES/*_FIDUCIARY_SUBNET*/: Cycles = fiduciary_subnet_cost(NETWORK_CANISTER_CREATION_FEE_CYCLES_13_NODE_SUBNET);

#[allow(non_upper_case_globals)]
pub const NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES/*_FIDUCIARY_SUBNET*/: Cycles = fiduciary_subnet_cost(NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES_13_NODE_SUBNET);




pub const fn cb_storage_size_mib_as_cb_network_memory_allocation_mib(storage_size_mib: u128) -> u128 {
    storage_size_mib * 3 + 10
}






pub const ICP_LEDGER_CREATE_CANISTER_MEMO: IcpMemo = IcpMemo(0x41455243); // == 'CREA'
pub const ICP_LEDGER_TOP_UP_CANISTER_MEMO: IcpMemo = IcpMemo(0x50555054); // == 'TPUP'



// -----------------------------------------------------------------

pub const MAINNET_SNS_ROOT: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 0, 0, 218, 1, 1]) ; // ibahq-taaaa-aaaaq-aadna-cai
pub const MAINNET_SNS_GOVERNANCE: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 0, 0, 219, 1, 1]); // igbbe-6yaaa-aaaaq-aadnq-cai

pub const MAINNET_CTS: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 0, 110, 1, 1]);
pub const MAINNET_CM_MAIN: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 0, 111, 1, 1]);
pub const MAINNET_BANK: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 0, 170, 1, 1]);
pub const MAINNET_FUELER: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 1, 177, 1, 1]); // dvpyg-3qaaa-aaaar-qagyq-cai
pub const MAINNET_TOP_LEVEL_UPGRADER: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 2, 8, 1, 1]); // yvs6s-hyaaa-aaaar-qaiea-cai


#[test]
fn test_fueler_principal() {
    println!("{:?}", Principal::from_text("dvpyg-3qaaa-aaaar-qagyq-cai").unwrap().as_slice());
}

pub const MAINNET_TOP_LEVEL_CANISTERS: [Principal; 3] = [MAINNET_CTS, MAINNET_CM_MAIN, MAINNET_BANK];



pub mod livetest {
    use candid::Principal;
    pub const LIVETEST_CTS: Principal = Principal::from_slice(&[0, 0, 0, 0, 1, 144, 8, 138, 1, 1]); // x3ncx-liaaa-aaaam-qbcfa-cai
    pub const LIVETEST_CM_MAIN: Principal = Principal::from_slice(&[0, 0, 0, 0, 1, 144, 8, 139, 1, 1]); // x4med-gqaaa-aaaam-qbcfq-cai
    pub const LIVETEST_CONTROLLER: Principal = Principal::from_slice(&[107, 243, 181, 41, 148, 42, 142, 159, 184, 86, 182, 232, 230, 52, 194, 29, 106, 241, 24, 87, 145, 127, 235, 49, 150, 90, 97, 189, 2]); // 35bfm-o3l6o-2stfb-kr2p3-qvvw5-dtdjq-q5nly-rqv4r-p7vtd-fs2mg-6qe // for the livetest canisters that at this time don't have an sns set up'
}
