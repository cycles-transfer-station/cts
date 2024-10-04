// https://github.com/Neutrinomic/wg_defi/tree/main/icrc-45

use serde_bytes::ByteBuf;
use candid::{Principal, CandidType, Deserialize};

pub const INTERNET_COMPUTER_PLATFORM_ID: PlatformId = 1;
pub const DEFAULT_DEPTH_LIMIT: usize = 100;

pub type PlatformId = u64; 
pub type PlatformPath = ByteBuf; // For the IC that is a Principal // Can be anything, a principal, a symbol like BTC, ethereum address, text, etc.
pub type DataSource = Principal; // Location from which we can get icrc_38_pair_data.
pub type ListPairsResponse = Vec<PairInfo>;
pub type PairResponseOk = Vec<PairData>;
pub type PairResponse = Result<PairResponseOk, PairResponseErr>;
pub type Level = u8;
pub type Rate = f64;
pub type Amount = u128;

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct TokenId {
    pub platform: PlatformId, 
    pub path: PlatformPath,
} 

#[derive(CandidType, Deserialize, PartialEq, Eq, Clone)]
pub struct PairId {
    pub base: TokenId,
    pub quote: TokenId,
}

#[derive(CandidType, Deserialize)]
pub struct PairInfo {
    pub data: DataSource,
    pub id: PairId,
}

#[derive(CandidType, Deserialize)]
pub struct DepthRequest{
    pub limit: u32, 
    pub level: Level,
}

#[derive(CandidType, Deserialize)]
pub struct PairRequest {
    pub pairs: Vec<PairId>, 
    pub depth: Option<DepthRequest>,
} 

#[derive(CandidType, Deserialize)]
pub enum PairResponseErr {
    NotFound(PairId),
    InvalidDepthLevel(Level),
    InvalidDepthLimit(u32),
}

#[derive(CandidType, Deserialize)]
pub struct TokenData {
    pub volume24: Amount,
    pub volume_total: Amount,
}

#[allow(non_snake_case)]
#[derive(CandidType, Deserialize)]
pub struct PairData {
    pub id: PairId,
    pub base: TokenData,
    pub quote: TokenData,
    pub volume24_USD: Option<Amount>, // (optional) Always 6 decimals
    pub volume_total_USD: Option<Amount>, // (optional) Always 6 decimals
    pub last: Rate, // Last trade rate
    pub last_timestamp: u64, // Last trade timestamp in nanoseconds
    pub bids: Vec<(Rate, Amount)>, // descending ordered by rate
    pub asks: Vec<(Rate, Amount)>, // ascending ordered by rate
    pub updated_timestamp: u64, // Last updated timestamp in nanoseconds
}
