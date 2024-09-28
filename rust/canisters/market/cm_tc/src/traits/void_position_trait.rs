use cts_lib::{
    icrc::{
        IcrcSubaccount,
    },
    types::{
        cm::tc::{
            PositionId,
            VoidCyclesPosition,
            VoidTokenPosition,
            VPUpdateStoragePositionData,
            PayoutData,
        }
    },
};
use candid::Principal;



pub trait VoidPositionTrait: Clone {
    fn position_id(&self) -> PositionId;
    fn positor(&self) -> Principal;
    fn quantity(&self) -> u128;
    fn payout_data(&self) -> &Option<PayoutData>;
    fn payout_data_mut(&mut self) -> &mut Option<PayoutData>;
    fn payout_lock(&mut self) -> &mut bool;
    fn can_remove(&self) -> bool;
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData;
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData;
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount>;
}


impl VoidPositionTrait for VoidCyclesPosition {
    fn position_id(&self) -> PositionId {
        self.position_id
    }
    fn positor(&self) -> Principal {
        self.positor
    }
    fn quantity(&self) -> u128 {
        self.cycles
    }
    fn payout_data(&self) -> &Option<PayoutData> {
        &self.cycles_payout_data
    }
    fn payout_data_mut(&mut self) -> &mut Option<PayoutData> {
        &mut self.cycles_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.cycles_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.cycles_payout_data.is_some() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData {
        &self.update_storage_position_data
    }
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData {
        &mut self.update_storage_position_data
    }
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.return_cycles_to_subaccount.clone()
    }
}

// --------

impl VoidPositionTrait for VoidTokenPosition {
    fn position_id(&self) -> PositionId {
        self.position_id
    }
    fn positor(&self) -> Principal {
        self.positor
    }
    fn quantity(&self) -> u128 {
        self.tokens
    }
    fn payout_data(&self) -> &Option<PayoutData> {
        &self.token_payout_data
    }
    fn payout_data_mut(&mut self) -> &mut Option<PayoutData> {
        &mut self.token_payout_data
    }
    fn payout_lock(&mut self) -> &mut bool {
        &mut self.token_payout_lock
    }
    fn can_remove(&self) -> bool {
        self.token_payout_data.is_some() && self.update_storage_position_data.status == true
    }
    fn update_storage_position_data(&self) -> &VPUpdateStoragePositionData {
        &self.update_storage_position_data
    }
    fn update_storage_position_data_mut(&mut self) -> &mut VPUpdateStoragePositionData {
        &mut self.update_storage_position_data
    }
    fn return_to_subaccount(&self) -> Option<IcrcSubaccount> {
        self.return_tokens_to_subaccount.clone()
    }
}
