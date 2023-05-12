

use crate::*;


pub type VoidCyclesPositionId = PositionId;
pub type VoidTokenPositionId = PositionId;
pub type CyclesPositionPurchaseId = PurchaseId;
pub type TokenPositionPurchaseId = PurchaseId;


#[derive(CandidType, Deserialize)]
pub struct CyclesPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub cycles: Cycles,
    pub minimum_purchase: Cycles,
    pub cycles_per_token_rate: CyclesPerToken,
    pub timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
pub struct TokenPosition {
    pub id: PositionId,   
    pub positor: Principal,
    pub tokens: Tokens,
    pub minimum_purchase: Tokens,
    pub cycles_per_token_rate: CyclesPerToken,
    pub timestamp_nanos: u128,
}




#[derive(Clone, CandidType, Deserialize)]
pub struct CyclesPayoutData {
    pub cmcaller_cycles_payout_call_success_timestamp_nanos: Option<u128>,
    pub cmcaller_cycles_payout_callback_complete: Option<(CyclesTransferRefund, Option<(u32, String)>)>,
    pub management_canister_posit_cycles_call_success: bool // this is use for when the payout-cycles-transfer-refund != 0, call the management_canister-deposit_cycles(payout-cycles-transfer-refund)
}
impl CyclesPayoutData {
    pub fn new() -> Self {
        Self {
            cmcaller_cycles_payout_call_success_timestamp_nanos: None,
            cmcaller_cycles_payout_callback_complete: None,
            management_canister_posit_cycles_call_success: false
        }
    }
    pub fn is_waiting_for_the_cycles_transferrer_transfer_cycles_callback(&self) -> bool {
        self.cmcaller_cycles_payout_call_success_timestamp_nanos.is_some() 
        && self.cmcaller_cycles_payout_callback_complete.is_none()
    }
    pub fn is_complete(&self) -> bool {
        if let Some((cycles_transfer_refund, _)) = self.cmcaller_cycles_payout_callback_complete {
            if cycles_transfer_refund == 0 || self.management_canister_posit_cycles_call_success == true {
                return true;
            }
        }
        false
    }
}


pub trait CyclesPayoutDataTrait {
    fn cycles_payout_data(&self) -> CyclesPayoutData;
    fn cycles_payout_lock(&self) -> bool;
    fn cycles_payout_payee(&self) -> Principal;
    fn cycles_payout_payee_method(&self) -> &'static str;
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError>;
    fn cycles(&self) -> Cycles;
    fn cm_call_id(&self) -> u128;
    fn cm_call_callback_method(&self) -> &'static str;
}



#[derive(Clone, CandidType, Deserialize)]
pub struct TokenTransferBlockHeightAndTimestampNanos {
    pub block_height: Option<BlockId>, // if None that means there was no transfer in this token-payout. it is an unlock of the funds within the same-token-id.
    pub timestamp_nanos: u128,
}
#[derive(Clone, CandidType, Deserialize)]
pub struct TokenPayoutData {
    pub token_transfer: Option<TokenTransferBlockHeightAndTimestampNanos>,
    pub cm_message_call_success_timestamp_nanos: Option<u128>,
    pub cm_message_callback_complete: Option<Option<(u32, String)>>, // first option for the callback-completion, second option for the possible-positor-message-call-error
}
impl TokenPayoutData {
    // no new fn because a void_token_position.token_payout_data must start with the token_transfer = Some(TokenTransferBlockHeightAndTimestampNanos)
    pub fn is_waiting_for_the_cmcaller_callback(&self) -> bool {
        self.cm_message_call_success_timestamp_nanos.is_some() 
        && self.cm_message_callback_complete.is_none()
    }
    pub fn is_complete(&self) -> bool {
        if self.cm_message_callback_complete.is_some() {
            // maybe add a check for if there is an error sending the cm_message and re-try on some cases 
            return true;
        }
        false
    }
}

pub trait TokenPayoutDataTrait {
    fn token_payout_data(&self) -> TokenPayoutData;
    fn token_payout_lock(&self) -> bool;
    fn token_payout_payee(&self) -> Principal;
    fn token_payout_payor(&self) -> Principal;
    fn token_payout_payee_method(&self) -> &'static str;
    fn token_payout_payee_method_quest_bytes(&self, token_payout_data_token_transfer: TokenTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError>; 
    fn tokens(&self) -> Tokens;
    fn token_transfer_memo(&self) -> Option<IcrcMemo>;
    fn token_transfer_fee(&self) -> Tokens;
    fn cm_call_id(&self) -> u128;
    fn cm_call_callback_method(&self) -> &'static str; 
}



// -----------------



#[derive(Clone, CandidType, Deserialize)]
pub struct TradeLog {
    pub position_id: PositionId,
    pub id: PurchaseId,
    pub positor: Principal, //maker
    pub purchaser: Principal, //taker
    pub tokens: Tokens,
    pub cycles: Cycles,
    pub rate: CyclesPerTokenRate,
    //but then how do we know whether the position is a cycles-position or a token-position & whether this is a cycles-position-purchase or a token-position-purchase?
    pub position_kind: PositionKind,
    pub timestamp_nanos: u128,
    pub cycles_payout_lock: bool,
    pub token_payout_lock: bool,
    pub cycles_payout_data: CyclesPayoutData,
    pub token_payout_data: TokenPayoutData
}

pub enum PositionKind {
    Cycles,
    Token
}

impl CyclesPayoutDataTrait for TradeLog {
    fn cycles_payout_data(&self) -> CyclesPayoutData { self.cycles_payout_data.clone() }
    fn cycles_payout_lock(&self) -> bool { self.cycles_payout_lock }
    fn cycles_payout_payee(&self) -> Principal { 
        match self.position_kind { 
            PositionKind::Cycles => self.purchaser,
            PositionKind::Token => self.positor,
        }
    }
    fn cycles_payout_payee_method(&self) -> &'static str { 
        match self.position_kind { 
            PositionKind::Cycles => CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_PURCHASER,
            PositionKind::Token => CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_POSITOR,
        } 
    }
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
        match self.position_kind { 
            PositionKind::Cycles => {
                encode_one(
                    CMCyclesPositionPurchasePurchaserMessageQuest {
                        cycles_position_id: self.cycles_position_id,
                        cycles_position_positor: self.cycles_position_positor,
                        cycles_position_cycles_per_token_rate: self.cycles_position_cycles_per_token_rate,
                        purchase_id: self.id,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                        token_payment: cycles_transform_tokens(self.cycles, self.cycles_position_cycles_per_token_rate),
                    }
                ) 
            }
            PositionKind::Token => {
                encode_one(
                    CMTokenPositionPurchasePositorMessageQuest{
                        token_position_id: self.token_position_id,
                        token_position_cycles_per_token_rate: self.token_position_cycles_per_token_rate,
                        purchase_id: self.id,
                        purchaser: self.purchaser,
                        token_purchase: self.tokens,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                    }
                )
            }
        }
    }
    fn cycles(&self) -> Cycles { self.cycles }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { 
        match self.position_kind { 
            PositionKind::Cycles => CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_PURCHASER,
            PositionKind::Token => CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_POSITOR
        }
    }
}
impl TokenPayoutDataTrait for TradeLog {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
    fn token_payout_lock(&self) -> bool { self.token_payout_lock }
    fn token_payout_payee(&self) -> Principal { 
        match self.position_kind { 
            PositionKind::Cycles => self.positor,
            PositionKind::Token => self.purchaser,
        }
    }
    fn token_payout_payor(&self) -> Principal { 
        match self.position_kind { 
            PositionKind::Cycles => self.purchaser,
            PositionKind::Token => self.positor,
        }
    }
    fn token_payout_payee_method(&self) -> &'static str { 
        match self.position_kind { 
            PositionKind::Cycles => CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR,
            PositionKind::Token => CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_PURCHASER,
    }
    fn token_payout_payee_method_quest_bytes(&self, token_payout_data_token_transfer: TokenTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError> {
        match self.position_kind { 
            PositionKind::Cycles => {
                encode_one(
                    CMCyclesPositionPurchasePositorMessageQuest {
                        cycles_position_id: self.cycles_position_id,
                        purchase_id: self.id,
                        purchaser: self.purchaser,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                        cycles_purchase: self.cycles,
                        cycles_position_cycles_per_token_rate: self.cycles_position_cycles_per_token_rate,
                        token_payment: self.tokens(),
                        token_transfer_block_height: token_payout_data_token_transfer.block_height.unwrap(), 
                        token_transfer_timestamp_nanos: token_payout_data_token_transfer.timestamp_nanos,
                    }    
                )
            }
            PositionKind::Token => {
                encode_one(
                    CMTokenPositionPurchasePurchaserMessageQuest {
                        token_position_id: self.token_position_id,
                        purchase_id: self.id, 
                        positor: self.token_position_positor,
                        purchase_timestamp_nanos: self.timestamp_nanos,
                        token_purchase: self.tokens(),
                        token_position_cycles_per_token_rate: self.token_position_cycles_per_token_rate,
                        cycles_payment: tokens_transform_cycles(self.tokens, self.token_position_cycles_per_token_rate),
                        token_transfer_block_height: token_payout_data_token_transfer.block_height.unwrap(),
                        token_transfer_timestamp_nanos: token_payout_data_token_transfer.timestamp_nanos,
                    }
                )
            }
        }
    } 
    fn tokens(&self) -> Tokens { self.tokens }
    fn token_transfer_memo(&self) -> Option<IcrcMemo> { 
        match self.position_kind { 
            PositionKind::Cycles => Some(IcrcMemo(ByteBuf::from(*CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO))),
            PositionKind::Token => Some(IcrcMemo(ByteBuf::from(*TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO)))
        }
    }
    fn token_transfer_fee(&self) -> Tokens { localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { 
        match self.position_kind { 
            PositionKind::Cycles => CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_POSITOR, 
            PositionKind::Token => CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_PURCHASER
        }
    } 
}


// --------



#[derive(Clone, CandidType, Deserialize)]
pub struct VoidCyclesPosition {
    pub position_id: PositionId,
    pub positor: Principal,
    pub cycles: Cycles,
    pub cycles_payout_lock: bool,  // lock for the payout
    pub cycles_payout_data: CyclesPayoutData,
    pub timestamp_nanos: u128
}

impl CyclesPayoutDataTrait for VoidCyclesPosition {
    fn cycles_payout_data(&self) -> CyclesPayoutData {
        self.cycles_payout_data.clone()
    }
    fn cycles_payout_lock(&self) -> bool { self.cycles_payout_lock }
    fn cycles_payout_payee(&self) -> Principal {
        self.positor
    }
    fn cycles_payout_payee_method(&self) -> &'static str {
        CM_MESSAGE_METHOD_VOID_CYCLES_POSITION_POSITOR
    } 
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMVoidCyclesPositionPositorMessageQuest{
                position_id: self.position_id,
                timestamp_nanos: self.timestamp_nanos,
            }
        )
    }
    fn cycles(&self) -> Cycles {
        self.cycles
    }
    fn cm_call_id(&self) -> u128 {
        self.position_id
    }
    fn cm_call_callback_method(&self) -> &'static str {
        CMCALLER_CALLBACK_VOID_CYCLES_POSITION_POSITOR
    }
}



// --------



#[derive(CandidType, Deserialize, Clone)]
pub struct VoidTokenPosition {
    pub position_id: PositionId,
    pub tokens: Tokens,
    pub positor: Principal,
    pub token_payout_lock: bool,  // lock for the payout
    pub token_payout_data: TokenPayoutData,
    pub timestamp_nanos: u128
}

impl TokenPayoutDataTrait for VoidTokenPosition {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
    fn token_payout_lock(&self) -> bool { self.token_payout_lock }
    fn token_payout_payee(&self) -> Principal { self.positor }
    fn token_payout_payor(&self) -> Principal { self.positor }
    fn token_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_VOID_TOKEN_POSITION_POSITOR }
    fn token_payout_payee_method_quest_bytes(&self, _token_payout_data_token_transfer: TokenTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMVoidTokenPositionPositorMessageQuest {
                position_id: self.position_id,
                void_tokens: self.tokens(),
                timestamp_nanos: self.timestamp_nanos
            }
        )
    }
    fn tokens(&self) -> Tokens { self.tokens }
    fn token_transfer_memo(&self) -> Option<IcrcMemo> { trap("void-token-position does not call the ledger."); }
    fn token_transfer_fee(&self) -> Tokens { trap("void-token-position does not call the ledger."); }
    fn cm_call_id(&self) -> u128 { self.position_id }  
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_VOID_TOKEN_POSITION_POSITOR } 
}


