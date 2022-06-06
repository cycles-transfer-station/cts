use ic_cdk::export::Principal;







pub const MANAGEMENT_CANISTER_ID: Principal = Principal::management_canister();

pub const ICP_PAYOUT_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-POUT"));
pub const ICP_FEE_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-TFEE"));







