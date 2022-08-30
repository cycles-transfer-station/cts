use crate::{
    ic_cdk::export::Principal,
    ic_ledger_types::{
        IcpMemo
    },
    types::{
        Cycles
    }
};
 

pub const KiB: usize = 1024;
pub const MiB: usize = KiB * 1024;
pub const GiB: usize = MiB * 1024;

pub const WASM_PAGE_SIZE_BYTES: usize = 65536; // 2^16 // 64KiB


pub const MANAGEMENT_CANISTER_ID: Principal = Principal::management_canister();

pub const CYCLES_PER_XDR: Cycles = 1_000_000_000_000u128; // 1T cycles = 1 XDR

pub const NETWORK_CANISTER_CREATION_FEE_CYCLES                  : Cycles = 100_000_000_000;
//pub const NETWORK_COMPUTE_PERCENT_ALLOCATED_PER_SECOND_FEE_CYCLES:Cycles= 100_000;
pub const NETWORK_UPDATE_MESSAGE_EXECUTION_FEE_CYCLES           : Cycles = 590_000;
pub const NETWORK_TEN_UPDATE_INSTRUCTIONS_EXECUTION_FEE_CYCLES  : Cycles = 4;
pub const NETWORK_XNET_CALL_FEE_CYCLES                          : Cycles = 260_000;             // For every inter-canister call performed (includes the cost for sending the request and receiving the response)
pub const NETWORK_XNET_BYTE_TRANSMISSION_FEE_CYCLES             : Cycles = 1_000;               // For every byte sent in an inter-canister call (for bytes sent in the request and response)
pub const NETWORK_INGRESS_MESSAGE_CEPTION_FEE_CYCLES            : Cycles = 1_200_000;
pub const NETWORK_INGRESS_BYTE_CEPTION_FEE_CYCLES               : Cycles = 2_000;               // what about bytes sent back as a sponse?
pub const NETWORK_GiB_STORAGE_PER_SECOND_FEE_CYCLES             : Cycles = 127_000;             // 4 SDR per GiB per year => 4e12 Cycles per year





pub const ICP_LEDGER_CREATE_CANISTER_MEMO: IcpMemo = IcpMemo(0x41455243); // == 'CREA'
pub const ICP_LEDGER_TOP_UP_CANISTER_MEMO: IcpMemo = IcpMemo(0x50555054); // == 'TPUP'



// -----------------------------------------------------------------





pub const ICP_CTS_TAKE_FEE_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-TFEE"));







