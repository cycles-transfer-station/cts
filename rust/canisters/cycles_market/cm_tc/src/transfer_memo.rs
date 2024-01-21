use crate::PurchaseId;
use serde_bytes::ByteBuf;

const TRADE_MEMO_START: &[u8; 8] = b"CTSTRADE";
const VOID_TOKEN_POSITION_MEMO_START: &[u8; 8] = b"CTS-VTP-";

pub fn create_trade_transfer_memo(purchase_id: PurchaseId) -> ByteBuf {
    create_token_transfer_memo_(TRADE_MEMO_START, purchase_id)    
}
pub fn create_void_token_position_transfer_memo(position_id: u128) -> ByteBuf {
    create_token_transfer_memo_(VOID_TOKEN_POSITION_MEMO_START, position_id)
}
fn create_token_transfer_memo_(memo_start: &[u8; 8], id: u128) -> ByteBuf {
    let mut v = Vec::<u8>::new();
    v.extend_from_slice(memo_start);
    leb128::write::unsigned(&mut v, id as u64).unwrap(); // when position ids get close to u64::max, change for a different library compatible with a u128.
    return ByteBuf::from(v);
}