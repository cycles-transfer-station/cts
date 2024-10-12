use candid::{Principal, CandidType, Deserialize};
use crate::{
    types::Cycles,
    icrc::Tokens,
    tools::cycles_per_token_rate_as_f64,
};

use super::tc::{PositionId, PositionKind, storage_logs::position_log::{PositionLog, PositionTerminationCause,}};


#[derive(Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
#[serde(try_from = "candid::types::reference::Func")]
pub struct GetPositionCallback{
    pub storage_canister_id: Principal,
}
impl GetPositionCallback {
    pub const METHOD_NAME: &'static str = "get_position";
    pub fn new(storage_canister_id: Principal) -> Self {
        Self { storage_canister_id }
    }
}
impl PartialOrd for GetPositionCallback {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for GetPositionCallback {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.storage_canister_id.cmp(&other.storage_canister_id)
    }
}
impl From<GetPositionCallback> for candid::types::reference::Func {
    fn from(callback: GetPositionCallback) -> Self {
        Self {
            principal: callback.storage_canister_id,
            method: GetPositionCallback::METHOD_NAME.to_string(),
        }
    }
}
impl TryFrom<candid::types::reference::Func> for GetPositionCallback {
    type Error = String;
    fn try_from(func: candid::types::reference::Func) -> Result<Self, Self::Error> {
        if &func.method != GetPositionCallback::METHOD_NAME {
            return Err(format!("Error trying to deserialize from a candid-callback-type with the method name: {}. The GetPositionCallback must have the method name: {}.", &func.method, GetPositionCallback::METHOD_NAME));
        }
        Ok(Self {
            storage_canister_id: func.principal
        })
    }
}
impl CandidType for GetPositionCallback {
    fn _ty() -> candid::types::Type {
        candid::func!((PositionId) -> (GetPositionSponse) query) // compatible with multi-level storage canister callbacks.
    }
    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        candid::types::reference::Func::from(self.clone()).idl_serialize(serializer)
    }
}

#[derive(CandidType, Deserialize, PartialEq, Debug)]
pub enum GetPositionSponse {
    Here(MoreReadablePositionLog),
    Storage(GetPositionCallback)
}

#[derive(CandidType, Deserialize, PartialEq, Debug)]
pub struct MoreReadablePositionLog {
    pub id: PositionId,
    pub positor: Principal,
    pub position_kind: MoreReadablePositionKind,
    /// If the position_kind is Cycles, then this value is CYCLES, which is the base token of the trading pair. If the position_kind is Tokens, then this value is the quote token of the trading pair.  
    pub original_position_quantity: u128,
    pub original_position_rate: f64,
    /// The amount of CYCLES that have traded through this position.
    pub cycles_trade_volume: Cycles,
    /// The amount of the quote token that has traded through this position.
    pub tokens_trade_volume: Tokens,
    pub average_trade_rate: f64,
    /// This value is the market fees taken out of the trades payouts amounts.
    /// If the position is to trade CYCLES for $SOMETOKEN, then the market_fees_paid is the amount of $SOMETOKEN that the market takes from the trade_tokens_volume before it pays out the trade.
    /// If the position is to trade $SOMETOKEN for CYCLES, then the market_fees_paid is the amount of CYCLES that the market takes from the trade_cycles_volume before it pays out the trade.    
    pub market_fees_paid: u128,
    pub creation_timestamp_nanos: u128,
    pub position_termination: Option<MoreReadablePositionTerminationData>,
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Debug)]
pub enum MoreReadablePositionKind{
    Cycles,
    Tokens
}
impl From<PositionKind> for MoreReadablePositionKind {
    fn from(pk: PositionKind) -> Self {
        match pk {
            PositionKind::Cycles => Self::Cycles,
            PositionKind::Token => Self::Tokens,
        }
    }
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Debug)]
pub struct MoreReadablePositionTerminationData {
    pub timestamp_nanos: u128,
    pub cause: PositionTerminationCause,
    pub position_leftover_transfer_status: Option<MoreReadablePositionLeftoverTransferData>
}

#[derive(CandidType, Deserialize, PartialEq, Eq, Debug)]
pub enum MoreReadablePositionLeftoverTransferData{
    DustCollection,
    Transfer{ amount: u128, ledger_transfer_fee: u128 }
}


impl MoreReadablePositionLog {
    pub fn from_position_log(pl: PositionLog, leftover_transfer_status: bool, token_decimal_places: u8) -> Self {
        Self {
            id: pl.id,
            positor: pl.positor,
            position_kind: MoreReadablePositionKind::from(pl.position_kind),
            original_position_quantity: pl.quest.quantity,
            original_position_rate: cycles_per_token_rate_as_f64(pl.quest.cycles_per_token_rate, token_decimal_places),
            cycles_trade_volume: {
                match pl.position_kind {
                    PositionKind::Cycles => {
                        pl.quest.quantity - pl.mainder_position_quantity
                    }
                    PositionKind::Token => {
                        pl.fill_quantity
                    }
                }
            },
            tokens_trade_volume: {
                match pl.position_kind {
                    PositionKind::Cycles => {
                        pl.fill_quantity
                    }
                    PositionKind::Token => {
                        pl.quest.quantity - pl.mainder_position_quantity
                    }
                }
            },
            average_trade_rate: cycles_per_token_rate_as_f64(pl.fill_average_rate, token_decimal_places),
            market_fees_paid: pl.payouts_fees_sum,
            creation_timestamp_nanos: pl.creation_timestamp_nanos,
            position_termination: pl.position_termination.map(|ptd| MoreReadablePositionTerminationData{
                timestamp_nanos: ptd.timestamp_nanos,
                cause: ptd.cause,
                position_leftover_transfer_status: {
                    match leftover_transfer_status {
                        false => None,
                        true => Some(
                            match pl.void_position_payout_dust_collection {
                                true => MoreReadablePositionLeftoverTransferData::DustCollection,
                                false => MoreReadablePositionLeftoverTransferData::Transfer{ 
                                    amount: pl.mainder_position_quantity - pl.void_position_payout_ledger_transfer_fee as u128,
                                    ledger_transfer_fee: pl.void_position_payout_ledger_transfer_fee as u128,
                                }
                            } 
                        )
                    } 
                }
            })
        }
    }
}