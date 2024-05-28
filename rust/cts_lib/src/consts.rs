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





pub const CTS_PURCHASE_CYCLES_BANK_COLLECT_PAYMENT_ICP_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-PCBC"));



//pub const CTS_LOCAL_ID: &'static [u8; 10] = b"cts_local_";
pub const CTS_ID: Principal = Principal::from_slice(&[0,0,0,0,2,48,0,110,1,1]); // em3jm-bqaaa-aaaar-qabxa-cai


