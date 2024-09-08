use std::collections::BTreeMap;
use serde_bytes::{Bytes, ByteBuf};
use candid::{CandidType, Deserialize};


pub type Icrc3Map<'a> = BTreeMap<&'a str, Icrc3Value<'a>>;

#[derive(CandidType, serde::Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Icrc3Value<'a> {
    Blob(&'a Bytes),
    Text(&'a str),
    Nat(u128),
    Int(i128),
    Array(Vec<Icrc3Value<'a>>),
    Map(Icrc3Map<'a>),
}

impl<'a> From<&'a Icrc3Value<'a>> for icrc_ledger_types::icrc::generic_value::ICRC3Value {
    fn from(v: &'a Icrc3Value<'a>) -> Self {
        match v {
            Icrc3Value::Blob(b) => Self::Blob(ByteBuf::from((*b).to_vec())),
            Icrc3Value::Text(t) => Self::Text(t.to_string()),
            Icrc3Value::Nat(n) => Self::Nat(n.clone().into()),
            Icrc3Value::Int(i) => Self::Int(i.clone().into()),
            Icrc3Value::Array(a) => Self::Array(a.into_iter().map(Self::from).collect()),
            Icrc3Value::Map(m) => {
                Self::Map(m.into_iter().map(|(k, v)| (k.to_string(), Self::from(v))).collect())
            }
        }
    }
}

impl<'a> Icrc3Value<'a> {
    // will clone everything. only for use once per block at the block-creation-time
    pub fn hash(&self) -> [u8; 32] {
        icrc_ledger_types::icrc::generic_value::ICRC3Value::from(self).hash()
    }
}



#[test]
fn test_icrc3_value_hash() {
    assert_eq!(
        hex::encode(Icrc3Value::Nat(42).hash()),
        "684888c0ebb17f374298b65ee2807526c066094c701bcc7ebbe1c1095f494fc1".to_string(),
    );
    
    assert_eq!(
        hex::encode(Icrc3Value::Int(-42).hash()),
        "de5a6f78116eca62d7fc5ce159d23ae6b889b365a1739ad2cf36f925a140d0cc".to_string(),
    );
    
    assert_eq!(
        hex::encode(Icrc3Value::Text("Hello, World!").hash()),
        "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f".to_string(),
    );
    
    assert_eq!(
        hex::encode(Icrc3Value::Blob(Bytes::new(b"\x01\x02\x03\x04")).hash()),
        "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a".to_string(),
    );
    
    assert_eq!(
        hex::encode(
            Icrc3Value::Array(vec![Icrc3Value::Nat(3), Icrc3Value::Text("foo"), Icrc3Value::Blob(Bytes::new(b"\x05\x06"))])
            .hash()
        ),
        "514a04011caa503990d446b7dec5d79e19c221ae607fb08b2848c67734d468d6".to_string(),
    );
    
    assert_eq!(
        hex::encode(
            Icrc3Value::Map(Icrc3Map::from_iter([
                ("from", Icrc3Value::Blob(Bytes::new(b"\x00\xab\xcd\xef\x00\x12\x34\x00\x56\x78\x9a\x00\xbc\xde\xf0\x00\x01\x23\x45\x67\x89\x00\xab\xcd\xef\x01"))),
                ("to", Icrc3Value::Blob(Bytes::new(b"\x00\xab\x0d\xef\x00\x12\x34\x00\x56\x78\x9a\x00\xbc\xde\xf0\x00\x01\x23\x45\x67\x89\x00\xab\xcd\xef\x01"))),
                ("amount", Icrc3Value::Nat(42)),
                ("created_at", Icrc3Value::Nat(1699218263)),
                ("memo", Icrc3Value::Nat(0))
            ]))
            .hash()
        ),
        "c56ece650e1de4269c5bdeff7875949e3e2033f85b2d193c2ff4f7f78bdcfc75".to_string(),
    );
}
