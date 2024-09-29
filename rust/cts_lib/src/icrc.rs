use crate::{
    types::CallError,
    tools::call_error_as_u32_and_string,
};
use ic_cdk::call;
use candid::{CandidType, Deserialize, Principal};
use serde_bytes::{ByteBuf, ByteArray};

pub use icrc_ledger_types::{
    icrc1::{
        transfer::{
            Memo as IcrcMemo,
            TransferError as Icrc1TransferError,
        }
    },
    icrc::generic_metadata_value::MetadataValue as IcrcMetadataValue,
};

pub mod icrc3;


pub type IcrcSub = ByteArray<32>;
pub type IcrcSubaccount = IcrcSub;    
pub const DEFAULT_SUBACCOUNT: &ByteArray<32> = &ByteArray::new([0u8; 32]);
pub const ICRC_DEFAULT_SUBACCOUNT: &ByteArray<32> = DEFAULT_SUBACCOUNT;

#[derive(serde::Serialize, CandidType, Deserialize, Clone, Debug, Copy)]
pub struct IcrcId {
    pub owner: Principal,
    pub subaccount: Option<IcrcSub>,
}

impl IcrcId {
    #[inline]
    pub fn effective_subaccount(&self) -> &IcrcSubaccount {
        self.subaccount.as_ref().unwrap_or(DEFAULT_SUBACCOUNT)
    }
}

impl PartialEq for IcrcId {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && self.effective_subaccount() == other.effective_subaccount()
    }
}

impl Eq for IcrcId {}

impl std::cmp::PartialOrd for IcrcId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for IcrcId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.owner.cmp(&other.owner).then_with(|| {
            self.effective_subaccount()
                .cmp(other.effective_subaccount())
        })
    }
}

impl std::hash::Hash for IcrcId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.owner.hash(state);
        self.effective_subaccount().hash(state);
    }
}

impl From<icrc_ledger_types::icrc1::account::Account> for IcrcId {
    fn from(q: icrc_ledger_types::icrc1::account::Account) -> Self {
        Self {
            owner: q.owner,
            subaccount: q.subaccount.map(ByteArray::new),
        }
    }
}

impl From<IcrcId> for icrc_ledger_types::icrc1::account::Account {
    fn from(q: IcrcId) -> Self {
        Self {
            owner: q.owner,
            subaccount: q.subaccount.as_deref().copied()
        }
    }
}

#[test]
fn test_icrc_id_serialize() {
    use icrc_ledger_types::icrc1::account::Account;    
    let p = Principal::from_slice(&[4u8; 29][..]);
    let icrc_id = IcrcId{ owner: p, subaccount: Some(ByteArray::new([7u8; 32])) };
    let ser = candid::encode_one(icrc_id).unwrap();
    let account: Account = candid::decode_one::<Account>(&ser).unwrap();
    assert_eq!(icrc_id.owner, account.owner);
    assert_eq!(icrc_id.subaccount.as_deref(), account.subaccount.as_ref());
}

#[test]
fn test_icrc_id_serialize_backwards() {
    use icrc_ledger_types::icrc1::account::Account;    
    let p = Principal::from_slice(&[78u8; 29][..]);
    let account = Account{ owner: p, subaccount: Some([92u8; 32]) };
    let ser = candid::encode_one(account).unwrap();
    let icrc_id: IcrcId = candid::decode_one::<IcrcId>(&ser).unwrap();
    assert_eq!(icrc_id.owner, account.owner);
    assert_eq!(icrc_id.subaccount.as_deref(), account.subaccount.as_ref());
}


#[derive(CandidType, serde::Serialize, Deserialize)]
pub struct Icrc1TransferQuest {
    pub to: IcrcId,
    pub fee: Option<u128>,
    pub memo: Option<ByteBuf>,
    pub from_subaccount: Option<IcrcSub>,
    pub created_at_time: Option<u64>,
    pub amount: u128,
}

pub use u128 as BlockId;
pub use u128 as Tokens;


pub async fn icrc1_transfer(icrc1_ledger_id: Principal, q: Icrc1TransferQuest) -> Result<Result<BlockId, Icrc1TransferError>, CallError> {
    call(
        icrc1_ledger_id,
        "icrc1_transfer",
        (q,),
    ).await
    .map_err(call_error_as_u32_and_string)
    .map(|(ir,): (Result<candid::Nat, Icrc1TransferError>,)| ir.map(|nat| nat.0.try_into().unwrap_or(0)))
}

pub async fn icrc1_balance_of(icrc1_ledger_id: Principal, count_id: IcrcId) -> Result<Tokens, (u32, String)> {
    call(
        icrc1_ledger_id,
        "icrc1_balance_of",
        (count_id,),
    ).await.map_err(|e| (e.0 as u32, e.1)).map(|(s,)| s)
}
