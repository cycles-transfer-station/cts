use candid::{
    CandidType,
    Deserialize,
    Principal,
};

pub mod bank;
pub mod cm;
pub mod fueler;


pub type Cycles = u128;
pub type CallError = (u32, String);


#[derive(CandidType, Deserialize)]
pub struct CallCanisterQuest {
    pub callee: Principal,
    pub method_name: String,
    pub arg_raw: Vec<u8>,
    pub cycles: u128
}


pub mod canister_code {
    use super::{CandidType, Deserialize};
    use serde::Serialize;
    
    #[derive(CandidType, Serialize, Deserialize, Clone)]
    pub struct CanisterCode {
        #[serde(with = "serde_bytes")]
        module: Vec<u8>,
        module_hash: [u8; 32]
    }

    impl CanisterCode {
        pub fn new(m: Vec<u8>) -> Self {
            Self {
                module_hash: crate::tools::sha256(&m),
                module: m
            }
        }
        pub fn empty() -> Self {
            Self {
                module_hash: [0u8; 32],
                module: Vec::new()
            }
        }
        pub fn module(&self) -> &Vec<u8> {
            &self.module
        }
        pub fn module_hash(&self) -> &[u8; 32] {
            &self.module_hash
        }
        pub fn module_hash_hex(&self) -> String {
            hex::encode(self.module_hash())
        }
        pub fn verify_module_hash(&self) -> Result<(), ()> {
            if *(self.module_hash()) != crate::tools::sha256(self.module()) {
                Err(())
            } else {
                Ok(())
            }
        }
    }
}
pub use canister_code::CanisterCode;

pub mod http_request{
    use super::*;
    use serde_bytes::ByteBuf;
    use candid::Nat;
    
    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct HttpRequest {
        pub method: String,
        pub url: String,
        pub headers: Vec<(String, String)>,
        #[serde(with = "serde_bytes")]
        pub body: Vec<u8>,
    }
    
    #[derive(Clone, Debug, CandidType)]
    pub struct HttpResponse<'a> {
        pub status_code: u16,
        pub headers: Vec<(&'a str, &'a str)>,
        pub body: &'a ByteBuf,
        pub streaming_strategy: Option<StreamStrategy<'a>>,
    }
    

    candid::define_function!(pub StreamCallback : (StreamCallbackTokenBackwards) -> (StreamCallbackHttpResponse) query);
    
    #[derive(Clone, Debug, CandidType)]
    pub enum StreamStrategy<'a> {
        Callback { callback: StreamCallback, token: StreamCallbackToken<'a>},
    }
    
    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct StreamCallbackToken<'a> {
        pub key: &'a str,
        pub content_encoding: &'a str,
        pub index: Nat,
        // We don't care about the sha, we just want to be backward compatible.
        pub sha256: Option<[u8; 32]>,
    }
    
    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct StreamCallbackTokenBackwards {
        pub key: String,
        pub content_encoding: String,
        pub index: Nat,
        // We don't care about the sha, we just want to be backward compatible.
        pub sha256: Option<[u8; 32]>,
    }
    
    #[derive(Clone, Debug, CandidType)]
    pub struct StreamCallbackHttpResponse<'a> {
        pub body: &'a ByteBuf,
        pub token: Option<StreamCallbackToken<'a>>,
    }

}


pub mod cts {
    use candid::{CandidType, Deserialize, Principal};
    use std::collections::HashSet;
    
    #[derive(CandidType, Deserialize)]
    pub struct CTSInit {
        pub batch_creators: Option<HashSet<Principal>>,
    }
}

pub mod top_level_upgrader {
    use candid::{CandidType, Deserialize, Principal};
    
    #[derive(CandidType, Deserialize)]
    pub struct UpgradeTopLevelCanisterQuest{
        pub canister_id: Principal,
        pub cc: crate::types::CanisterCode, 
        pub post_upgrade_quest: Vec<u8>,
        pub take_canister_snapshot: bool,
    }

}
