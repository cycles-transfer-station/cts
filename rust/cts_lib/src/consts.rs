use crate::{
    ic_cdk::export::Principal,
    ic_ledger_types::{
        IcpMemo
    },
    types::{
        Cycles
    }
};
 
#[allow(non_upper_case_globals)]
pub const KiB: usize = 1024;
#[allow(non_upper_case_globals)]
pub const MiB: usize = KiB * 1024;
#[allow(non_upper_case_globals)]
pub const GiB: usize = MiB * 1024;

pub const NANOS_IN_A_SECOND: u128 = 1_000_000_000;
pub const SECONDS_IN_A_MINUTE: u128 = 60; 
pub const SECONDS_IN_AN_HOUR: u128 = SECONDS_IN_A_MINUTE * 60;
pub const SECONDS_IN_A_DAY: u128 = SECONDS_IN_AN_HOUR * 24;


pub const WASM_PAGE_SIZE_BYTES: usize = 65536; // 2^16 // 64KiB


pub const MANAGEMENT_CANISTER_ID: Principal = Principal::management_canister();

pub const TRILLION: u128 = 1_000_000_000_000;

pub const CYCLES_PER_XDR: Cycles = TRILLION; // 1T cycles = 1 XDR

pub const NETWORK_CANISTER_CREATION_FEE_CYCLES                  : Cycles = 100_000_000_000;
//pub const NETWORK_COMPUTE_PERCENT_ALLOCATED_PER_SECOND_FEE_CYCLES:Cycles= 100_000;
pub const NETWORK_UPDATE_MESSAGE_EXECUTION_FEE_CYCLES           : Cycles = 590_000;
pub const NETWORK_TEN_UPDATE_INSTRUCTIONS_EXECUTION_FEE_CYCLES  : Cycles = 4;
pub const NETWORK_XNET_CALL_FEE_CYCLES                          : Cycles = 260_000;             // For every inter-canister call performed (includes the cost for sending the request and receiving the response)
pub const NETWORK_XNET_BYTE_TRANSMISSION_FEE_CYCLES             : Cycles = 1_000;               // For every byte sent in an inter-canister call (for bytes sent in the request and response)
pub const NETWORK_INGRESS_MESSAGE_CEPTION_FEE_CYCLES            : Cycles = 1_200_000;
pub const NETWORK_INGRESS_BYTE_CEPTION_FEE_CYCLES               : Cycles = 2_000;               // what about bytes sent back as a sponse?
#[allow(non_upper_case_globals)]
pub const NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES             : Cycles = 127_000;             // 4 SDR per GiB per year => 4e12 Cycles per year





pub const fn cb_storage_size_mib_as_cb_network_memory_allocation_mib(storage_size_mib: u128) -> u128 {
    storage_size_mib * 3 + 10
}






pub const ICP_LEDGER_CREATE_CANISTER_MEMO: IcpMemo = IcpMemo(0x41455243); // == 'CREA'
pub const ICP_LEDGER_TOP_UP_CANISTER_MEMO: IcpMemo = IcpMemo(0x50555054); // == 'TPUP'



// -----------------------------------------------------------------





pub const CTS_TRANSFER_ICP_FEE_ICP_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-TRIF"));
pub const CTS_PURCHASE_CYCLES_BANK_COLLECT_PAYMENT_ICP_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-PCBC"));







