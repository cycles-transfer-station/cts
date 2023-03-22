// make limit on how many on-the-market-positions each user can have at the same time. bc each user must be a canister, it helps stop "dos attack-attempts"

use std::{
    cell::{Cell, RefCell},
    collections::{HashSet}
};
use futures::task::Poll;
use cts_lib::{
    tools::{
        localkey::{
            self,
            refcell::{with, with_mut}
        },
        user_icp_id,
        cycles_transform_tokens,
        tokens_transform_cycles,
        principal_token_subaccount,
        round_robin,
        time_nanos,
        time_nanos_u64,
        time_seconds,
        rchunk_data
    },
    consts::{
        MiB,
        WASM_PAGE_SIZE_BYTES,
        MANAGEMENT_CANISTER_ID,
        NANOS_IN_A_SECOND,
        SECONDS_IN_A_MINUTE,
        SECONDS_IN_AN_HOUR,
        SECONDS_IN_A_DAY
    },
    types::{
        Cycles,
        CyclesTransferMemo,
        CyclesTransferRefund,
        DownloadRChunkQuest,
        cycles_transferrer,
        management_canister,
        cycles_market::icrc1_token_trade_contract::*,
        cm_caller::*,
    },
    icrc::{
        IcrcId, 
        IcrcSub,
        ICRC_DEFAULT_SUBACCOUNT,
        IcrcMemo,
        Tokens,
        TokenTransferError,
        TokenTransferArg,
        BlockId,
        icrc1_transfer,
        icrc1_balance_of
    },
    ic_cdk::{
        self,
        api::{
            trap,
            caller,
            call::{
                call,
                call_with_payment128,
                call_raw128,
                reply,
                CallResult,
                msg_cycles_refunded128,
                msg_cycles_available128,
                msg_cycles_accept128,
                arg_data,
            },
            canister_balance128,
            stable::{
                stable64_write,
                stable64_size,
                stable64_read,
                stable64_grow
           }
           
        },
        export::{
            Principal,
            candid::{
                self, 
                CandidType,
                Deserialize,
                utils::{encode_one, decode_one},
                error::Error as CandidError,
            }
        }
    },
    ic_cdk_macros::{
        update,
        query,
        init,
        pre_upgrade,
        post_upgrade
    }
};
use serde_bytes::ByteBuf;



type VoidCyclesPositionId = PositionId;
type VoidTokenPositionId = PositionId;
type CyclesPositionPurchaseId = PurchaseId;
type TokenPositionPurchaseId = PurchaseId;


#[derive(CandidType, Deserialize)]
struct CyclesPosition {
    id: PositionId,   
    positor: Principal,
    cycles: Cycles,
    minimum_purchase: Cycles,
    cycles_per_token_rate: CyclesPerToken,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct TokenPosition {
    id: PositionId,   
    positor: Principal,
    tokens: Tokens,
    minimum_purchase: Tokens,
    cycles_per_token_rate: CyclesPerToken,
    timestamp_nanos: u128,
}




#[derive(Clone, CandidType, Deserialize)]
struct CyclesPayoutData {
    cmcaller_cycles_payout_call_success_timestamp_nanos: Option<u128>,
    cmcaller_cycles_payout_callback_complete: Option<(CyclesTransferRefund, Option<(u32, String)>)>,
    management_canister_posit_cycles_call_success: bool // this is use for when the payout-cycles-transfer-refund != 0, call the management_canister-deposit_cycles(payout-cycles-transfer-refund)
}
impl CyclesPayoutData {
    fn new() -> Self {
        Self {
            cmcaller_cycles_payout_call_success_timestamp_nanos: None,
            cmcaller_cycles_payout_callback_complete: None,
            management_canister_posit_cycles_call_success: false
        }
    }
    fn is_waiting_for_the_cycles_transferrer_transfer_cycles_callback(&self) -> bool {
        self.cmcaller_cycles_payout_call_success_timestamp_nanos.is_some() 
        && self.cmcaller_cycles_payout_callback_complete.is_none()
    }
    fn is_complete(&self) -> bool {
        if let Some((cycles_transfer_refund, _)) = self.cmcaller_cycles_payout_callback_complete {
            if cycles_transfer_refund == 0 || self.management_canister_posit_cycles_call_success == true {
                return true;
            }
        }
        false
    }
    fn handle_do_cycles_payout_result(&mut self, do_cycles_payout_result: DoCyclesPayoutResult) {
        if let Ok(do_cycles_payout_sponse) = do_cycles_payout_result {  
            match do_cycles_payout_sponse {
                DoCyclesPayoutSponse::CMCallerCyclesPayoutCallSuccessTimestampNanos(opt_timestamp_ns) => {
                    self.cmcaller_cycles_payout_call_success_timestamp_nanos = opt_timestamp_ns;                            
                },
                DoCyclesPayoutSponse::ManagementCanisterPositCyclesCallSuccess(management_canister_posit_cycles_call_success) => {
                    self.management_canister_posit_cycles_call_success = management_canister_posit_cycles_call_success;
                },
                DoCyclesPayoutSponse::NothingToDo => {}
            }
        }
    }

}


trait CyclesPayoutDataTrait {
    fn cycles_payout_data(&self) -> CyclesPayoutData;
    fn cycles_payout_payee(&self) -> Principal;
    fn cycles_payout_payee_method(&self) -> &'static str;
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError>;
    fn cycles(&self) -> Cycles;
    fn cm_call_id(&self) -> u128;
    fn cm_call_callback_method(&self) -> &'static str;
}



#[derive(Clone, CandidType, Deserialize)]
struct TokenTransferBlockHeightAndTimestampNanos {
    block_height: Option<BlockId>, // if None that means there was no transfer in this token-payout. it is an unlock of the funds within the same-token-id.
    timestamp_nanos: u128,
}
#[derive(Clone, CandidType, Deserialize)]
struct TokenPayoutData {
    token_transfer: Option<TokenTransferBlockHeightAndTimestampNanos>,
    cm_message_call_success_timestamp_nanos: Option<u128>,
    cm_message_callback_complete: Option<Option<(u32, String)>>, // first option for the callback-completion, second option for the possible-positor-message-call-error
}
impl TokenPayoutData {
    // no new fn because a void_token_position.token_payout_data must start with the token_transfer = Some(TokenTransferBlockHeightAndTimestampNanos)
    fn is_waiting_for_the_cmcaller_callback(&self) -> bool {
        self.cm_message_call_success_timestamp_nanos.is_some() 
        && self.cm_message_callback_complete.is_none()
    }
    fn is_complete(&self) -> bool {
        if self.cm_message_callback_complete.is_some() {
            return true;
        }
        false
    }
    fn handle_do_token_payout_sponse(&mut self, do_token_payout_sponse: DoTokenPayoutSponse) {
        match do_token_payout_sponse {
            DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType) => {
                
            },
            DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_transfer, _cm_message_error_type) => {
                self.token_transfer = Some(token_transfer);
            },
            DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageSuccess(token_transfer, cm_message_call_success_timestamp_nanos) => {
                self.token_transfer = Some(token_transfer);
                self.cm_message_call_success_timestamp_nanos = Some(cm_message_call_success_timestamp_nanos);
            },
            DoTokenPayoutSponse::NothingForTheDo => {},
        }
    }
    
}

trait TokenPayoutDataTrait {
    fn token_payout_data(&self) -> TokenPayoutData;
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



#[derive(Clone, CandidType, Deserialize)]
struct CyclesPositionPurchase {
    cycles_position_id: PositionId,
    cycles_position_positor: Principal,
    cycles_position_cycles_per_token_rate: CyclesPerToken,
    id: PurchaseId,
    purchaser: Principal,
    cycles: Cycles,
    timestamp_nanos: u128,
    cycles_payout_lock: bool,
    token_payout_lock: bool,
    cycles_payout_data: CyclesPayoutData,
    token_payout_data: TokenPayoutData
}
impl CyclesPayoutDataTrait for CyclesPositionPurchase {
    fn cycles_payout_data(&self) -> CyclesPayoutData { self.cycles_payout_data.clone() }
    fn cycles_payout_payee(&self) -> Principal { self.purchaser }
    fn cycles_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_PURCHASER }
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
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
    fn cycles(&self) -> Cycles { self.cycles }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_PURCHASER }
}
impl TokenPayoutDataTrait for CyclesPositionPurchase {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
    fn token_payout_payee(&self) -> Principal { self.cycles_position_positor }
    fn token_payout_payor(&self) -> Principal { self.purchaser }
    fn token_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR }
    fn token_payout_payee_method_quest_bytes(&self, token_payout_data_token_transfer: TokenTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError> {
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
    fn tokens(&self) -> Tokens { cycles_transform_tokens(self.cycles, self.cycles_position_cycles_per_token_rate) }
    fn token_transfer_memo(&self) -> Option<IcrcMemo> { Some(IcrcMemo(ByteBuf::from(*CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO))) }
    fn token_transfer_fee(&self) -> Tokens { localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_POSITOR } 
}




#[derive(Clone, CandidType, Deserialize)]
struct TokenPositionPurchase {
    token_position_id: PositionId,
    token_position_positor: Principal,
    token_position_cycles_per_token_rate: CyclesPerToken,
    id: PurchaseId,
    purchaser: Principal,
    tokens: Tokens,
    timestamp_nanos: u128,
    cycles_payout_lock: bool,
    token_payout_lock: bool,
    cycles_payout_data: CyclesPayoutData,
    token_payout_data: TokenPayoutData // even though the purchaser knows bout the purchase, it is better to send the purchaser a message when the token_transfer is complete with the block height 
}
impl CyclesPayoutDataTrait for TokenPositionPurchase {
    fn cycles_payout_data(&self) -> CyclesPayoutData {
        self.cycles_payout_data.clone()
    }
    fn cycles_payout_payee(&self) -> Principal {
        self.token_position_positor
    }
    fn cycles_payout_payee_method(&self) -> &'static str {
        CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_POSITOR
    }
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
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
    fn cycles(&self) -> Cycles {
        tokens_transform_cycles(self.tokens, self.token_position_cycles_per_token_rate)
    }
    fn cm_call_id(&self) -> u128 {
        self.id
    }
    fn cm_call_callback_method(&self) -> &'static str {
        CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_POSITOR
    }
}
impl TokenPayoutDataTrait for TokenPositionPurchase {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
    fn token_payout_payee(&self) -> Principal { self.purchaser }
    fn token_payout_payor(&self) -> Principal { self.token_position_positor }
    fn token_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_PURCHASER }
    fn token_payout_payee_method_quest_bytes(&self, token_payout_data_token_transfer: TokenTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError> {
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
    fn tokens(&self) -> Tokens { self.tokens }
    fn token_transfer_memo(&self) -> Option<IcrcMemo> { Some(IcrcMemo(ByteBuf::from(*TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO))) }
    fn token_transfer_fee(&self) -> Tokens { localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE)/*change for a fee set at the time of the lock of the funds and plus the fee willing to be paid by the purchaser*/ }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_PURCHASER }

}



#[derive(Clone, CandidType, Deserialize)]
struct VoidCyclesPosition {
    position_id: PositionId,
    positor: Principal,
    cycles: Cycles,
    cycles_payout_lock: bool,  // lock for the payout
    cycles_payout_data: CyclesPayoutData,
    timestamp_nanos: u128
}

impl CyclesPayoutDataTrait for VoidCyclesPosition {
    fn cycles_payout_data(&self) -> CyclesPayoutData {
        self.cycles_payout_data.clone()
    }
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


#[derive(CandidType, Deserialize, Clone)]
struct VoidTokenPosition {
    position_id: PositionId,
    tokens: Tokens,
    positor: Principal,
    token_payout_lock: bool,  // lock for the payout
    token_payout_data: TokenPayoutData,
    timestamp_nanos: u128
}
impl TokenPayoutDataTrait for VoidTokenPosition {
    fn token_payout_data(&self) -> TokenPayoutData { self.token_payout_data.clone() }
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



#[derive(CandidType, Deserialize)]
struct CMData {
    cts_id: Principal,
    cm_main_id: Principal,
    cm_caller: Principal,
    icrc1_token_ledger: Principal,
    icrc1_token_ledger_transfer_fee: Tokens,
    id_counter: u128,
    mid_call_user_token_balance_locks: HashSet<Principal>,
    cycles_positions: Vec<CyclesPosition>,
    token_positions: Vec<TokenPosition>,
    cycles_positions_purchases: Vec<CyclesPositionPurchase>,
    token_positions_purchases: Vec<TokenPositionPurchase>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    void_token_positions: Vec<VoidTokenPosition>,
    do_payouts_errors: Vec<(u32, String)>,
}

impl CMData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            cm_main_id: Principal::from_slice(&[]),
            cm_caller: Principal::from_slice(&[]),
            icrc1_token_ledger: Principal::from_slice(&[]),
            icrc1_token_ledger_transfer_fee: 0,
            id_counter: 0,
            mid_call_user_token_balance_locks: HashSet::new(),
            cycles_positions: Vec::new(),
            token_positions: Vec::new(),
            cycles_positions_purchases: Vec::new(),
            token_positions_purchases: Vec::new(),
            void_cycles_positions: Vec::new(),
            void_token_positions: Vec::new(),
            do_payouts_errors: Vec::new()
        }
    }
}



#[derive(CandidType, Deserialize)]
struct OldCMData {
    cts_id: Principal,
    cm_caller: Principal,
    id_counter: u128,
    mid_call_user_token_balance_locks: HashSet<Principal>,
    cycles_positions: Vec<CyclesPosition>,
    token_positions: Vec<TokenPosition>,
    cycles_positions_purchases: Vec<CyclesPositionPurchase>,
    token_positions_purchases: Vec<TokenPositionPurchase>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    void_token_positions: Vec<VoidTokenPosition>,
    do_payouts_errors: Vec<(u32, String)>,
}




pub const CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;

pub const TRANSFER_TOKEN_BALANCE_FEE: Cycles = 50_000_000_000;

pub const CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

pub const MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK: u128 = NANOS_IN_A_SECOND * SECONDS_IN_AN_HOUR * 72;
pub const VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS: u128 = SECONDS_IN_AN_HOUR * 1;


pub const MINIMUM_CYCLES_POSITION: Cycles = 1_000_000_000_000;

pub const MINIMUM_TOKEN_POSITION: Tokens = 1;



const CANISTER_NETWORK_MEMORY_ALLOCATION_MiB: usize = 500; // multiple of 10
const CANISTER_DATA_STORAGE_SIZE_MiB: usize = CANISTER_NETWORK_MEMORY_ALLOCATION_MiB / 2 - 20/*memory-size at the start [re]placement*/; 

const CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_CYCLES_POSITIONS: usize = CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<CyclesPosition>();

const TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_TOKEN_POSITIONS: usize = TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<TokenPosition>();

const TOKEN_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_TOKEN_POSITIONS_PURCHASES: usize = TOKEN_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<TokenPositionPurchase>();

const CYCLES_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_CYCLES_POSITIONS_PURCHASES: usize = CYCLES_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<CyclesPositionPurchase>();

const VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_VOID_CYCLES_POSITIONS: usize = VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidCyclesPosition>();

const VOID_TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_VOID_TOKEN_POSITIONS: usize = VOID_TOKEN_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidTokenPosition>();


const DO_VOID_CYCLES_POSITIONS_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_VOID_TOKEN_POSITIONS_TOKEN_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_CYCLES_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_CYCLES_POSITIONS_PURCHASES_TOKEN_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_TOKEN_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_TOKEN_POSITIONS_PURCHASES_TOKEN_PAYOUTS_CHUNK_SIZE: usize = 10;



const CM_MESSAGE_METHOD_VOID_CYCLES_POSITION_POSITOR: &'static str       = "cm_message_void_cycles_position_positor";
const CM_MESSAGE_METHOD_VOID_TOKEN_POSITION_POSITOR: &'static str          = "cm_message_void_token_position_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR: &'static str   = "cm_message_cycles_position_purchase_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_cycles_position_purchase_purchaser";
const CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_POSITOR: &'static str      = "cm_message_token_position_purchase_positor";
const CM_MESSAGE_METHOD_TOKEN_POSITION_PURCHASE_PURCHASER: &'static str    = "cm_message_token_position_purchase_purchaser";

const CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_cycles_position_purchase_purchaser_cmcaller_callback";
const CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_POSITOR: &'static str = "cm_message_cycles_position_purchase_positor_cmcaller_callback";
const CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_token_position_purchase_purchaser_cmcaller_callback";
const CMCALLER_CALLBACK_TOKEN_POSITION_PURCHASE_POSITOR: &'static str = "cm_message_token_position_purchase_positor_cmcaller_callback";
const CMCALLER_CALLBACK_VOID_CYCLES_POSITION_POSITOR: &'static str = "cm_message_void_cycles_position_positor_cmcaller_callback";
const CMCALLER_CALLBACK_VOID_TOKEN_POSITION_POSITOR: &'static str = "cm_message_void_token_position_positor_cmcaller_callback";



const TOKEN_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO: &[u8; 8] = b"CM-IPP-0";
const CYCLES_POSITION_PURCHASE_TOKEN_TRANSFER_MEMO: &[u8; 8] = b"CM-CPP-0";

const TRANSFER_TOKEN_BALANCE_MEMO: &[u8; 8] = b"CMTRNSFR";

const SEE_CYCLES_POSITIONS_CHUNK_SIZE: usize = 300;
const SEE_TOKEN_POSITIONS_CHUNK_SIZE: usize = 300;
const SEE_CYCLES_POSITIONS_PURCHASES_CHUNK_SIZE: usize = 300;
const SEE_TOKEN_POSITIONS_PURCHASES_CHUNK_SIZE: usize = 300;



const MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS: usize = 500;


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;





thread_local! {
    static CM_DATA: RefCell<CMData> = RefCell::new(CMData::new()); 
    
    // not save through the upgrades
    static TOKEN_LEDGER_ID: Cell<Principal> = Cell::new(Principal::from_slice(&[]));
    static TOKEN_LEDGER_TRANSFER_FEE: Cell<Tokens> = Cell::new(0);
    static STOP_CALLS: Cell<bool> = Cell::new(false);
    static STATE_SNAPSHOT: RefCell<Vec<u8>> = RefCell::new(Vec::new());    
}


// -------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CMInit {
    cts_id: Principal,
    cm_main_id: Principal,
    cm_caller: Principal,
    icrc1_token_ledger: Principal,
    icrc1_token_ledger_transfer_fee: Tokens,
} 

#[init]
fn init(cm_init: CMInit) {
    with_mut(&CM_DATA, |cm_data| { 
        cm_data.cts_id = cm_init.cts_id; 
        cm_data.cm_main_id = cm_init.cm_main_id; 
        cm_data.cm_caller = cm_init.cm_caller;
        cm_data.icrc1_token_ledger = cm_init.icrc1_token_ledger; 
        cm_data.icrc1_token_ledger_transfer_fee = cm_init.icrc1_token_ledger_transfer_fee;
    });
    
    localkey::cell::set(&TOKEN_LEDGER_ID, cm_init.icrc1_token_ledger);
    localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, cm_init.icrc1_token_ledger_transfer_fee);
} 


// -------------------------------------------------------------


fn create_state_snapshot() {
    let mut cm_data_candid_bytes: Vec<u8> = with(&CM_DATA, |cm_data| { encode_one(cm_data).unwrap() });
    cm_data_candid_bytes.shrink_to_fit();
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = cm_data_candid_bytes; 
    });
}

fn load_state_snapshot_data() {
    
    let cm_data_of_the_state_snapshot: CMData = with(&STATE_SNAPSHOT, |state_snapshot| {
        match decode_one::<CMData>(state_snapshot) {
            Ok(cm_data) => cm_data,
            Err(_) => {
                trap("error decode of the state-snapshot CMData");
                /*
                let old_cm_data: OldCMData = decode_one::<OldCMData>(state_snapshot).unwrap();
                let cm_data: CMData = CMData{
                    cts_id: old_cm_data.cts_id,
                    cm_caller: old_cm_data.cm_caller,
                    id_counter: old_cm_data.id_counter,
                    mid_call_user_token_balance_locks: old_cm_data.mid_call_user_token_balance_locks,
                    cycles_positions: old_cm_data.cycles_positions,
                    token_positions: old_cm_data.token_positions,
                    cycles_positions_purchases: old_cm_data.cycles_positions_purchases,
                    token_positions_purchases: old_cm_data.token_positions_purchases,
                    void_cycles_positions: old_cm_data.void_cycles_positions,
                    void_token_positions: old_cm_data.void_token_positions,
                    do_payouts_errors: old_cm_data.do_payouts_errors
                };
                cm_data
                */
            }
        }
    });

    localkey::cell::set(&TOKEN_LEDGER_ID, cm_data_of_the_state_snapshot.icrc1_token_ledger);
    localkey::cell::set(&TOKEN_LEDGER_TRANSFER_FEE, cm_data_of_the_state_snapshot.icrc1_token_ledger_transfer_fee);

    with_mut(&CM_DATA, |cm_data| {
        *cm_data = cm_data_of_the_state_snapshot;    
    });
    
}

// -------------------------------------------------------------


#[pre_upgrade]
fn pre_upgrade() {
    
    create_state_snapshot();
    
    let current_stable_size_wasm_pages: u64 = stable64_size();
    let current_stable_size_bytes: u64 = current_stable_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;

    with(&STATE_SNAPSHOT, |state_snapshot| {
        let want_stable_memory_size_bytes: u64 = STABLE_MEMORY_HEADER_SIZE_BYTES + 8/*len of the state_snapshot*/ + state_snapshot.len() as u64; 
        if current_stable_size_bytes < want_stable_memory_size_bytes {
            stable64_grow(((want_stable_memory_size_bytes - current_stable_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1).unwrap();
        }
        stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES, &((state_snapshot.len() as u64).to_be_bytes()));
        stable64_write(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, state_snapshot);
    });
}

#[post_upgrade]
fn post_upgrade() {
    let mut state_snapshot_len_u64_be_bytes: [u8; 8] = [0; 8];
    stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES, &mut state_snapshot_len_u64_be_bytes);
    let state_snapshot_len_u64: u64 = u64::from_be_bytes(state_snapshot_len_u64_be_bytes); 
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = vec![0; state_snapshot_len_u64 as usize]; 
        stable64_read(STABLE_MEMORY_HEADER_SIZE_BYTES + 8, state_snapshot);
    });
    
    load_state_snapshot_data();
} 


// -------------------------------------------------------------

#[no_mangle]
fn canister_inspect_message() {
    use cts_lib::ic_cdk::api::call::{method_name, accept_message};
    
    if [
        "create_cycles_position",
        "create_token_position",
        "purchase_cycles_position",
        "purchase_token_position",
        "void_position",
        "see_token_lock",
        "transfer_token_balance",
        "cm_message_cycles_position_purchase_purchaser_cmcaller_callback",
        "cm_message_cycles_position_purchase_positor_cmcaller_callback",
        "cm_message_token_position_purchase_purchaser_cmcaller_callback",
        "cm_message_token_position_purchase_positor_cmcaller_callback",
        "cm_message_void_cycles_position_positor_cmcaller_callback",
        "cm_message_void_token_position_positor_cmcaller_callback",
        "see_cycles_positions",
        "see_token_positions",
        "see_cycles_positions_purchases",
        "see_token_positions_purchases",
        "download_cycles_positions_rchunks",
        "download_token_positions_rchunks",
        "download_cycles_positions_purchases_rchunks",
        "download_token_positions_purchases_rchunks"
    ].contains(&&method_name()[..]) {
        trap("this method must be call by a canister or a query call.");
    }
    
    
    accept_message();    
}


// -------------------------------------------------------------

fn cts_id() -> Principal {
    with(&CM_DATA, |cm_data| { cm_data.cts_id })
}

fn new_id(cm_data_id_counter: &mut u128) -> u128 {
    let id: u128 = cm_data_id_counter.clone();
    *(cm_data_id_counter) += 1;
    id
}


async fn token_transfer(q: TokenTransferArg) -> Result<Result<BlockId, TokenTransferError>, (u32, String)> {
    icrc1_transfer(localkey::cell::get(&TOKEN_LEDGER_ID), q).await
}

async fn token_balance(count_id: IcrcId) -> Result<Tokens, (u32, String)> {
    icrc1_balance_of(localkey::cell::get(&TOKEN_LEDGER_ID), count_id).await
}




async fn check_user_cycles_market_token_ledger_balance(user_id: &Principal) -> Result<Tokens, (u32, String)> {
    token_balance(
        IcrcId{
            owner: ic_cdk::api::id(),
            subaccount: Some(principal_token_subaccount(user_id))
        }
    ).await
}


fn check_user_token_balance_in_the_lock(cm_data: &CMData, user_id: &Principal) -> Tokens {
    cm_data.token_positions.iter()
        .filter(|token_position: &&TokenPosition| { token_position.positor == *user_id })
        .fold(0, |cummulator: Tokens, user_token_position: &TokenPosition| {
            cummulator + user_token_position.tokens + ( user_token_position.tokens / user_token_position.minimum_purchase * localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) )
        })
    +
    cm_data.cycles_positions_purchases.iter()
        .filter(|cycles_position_purchase: &&CyclesPositionPurchase| {
            cycles_position_purchase.purchaser == *user_id && cycles_position_purchase.token_payout_data.token_transfer.is_none() 
        })
        .fold(0, |cummulator: Tokens, user_cycles_position_purchase_with_unpaid_tokens: &CyclesPositionPurchase| {
            cummulator + cycles_transform_tokens(user_cycles_position_purchase_with_unpaid_tokens.cycles, user_cycles_position_purchase_with_unpaid_tokens.cycles_position_cycles_per_token_rate) + localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE)
        })
    +
    cm_data.token_positions_purchases.iter()
        .filter(|token_position_purchase: &&TokenPositionPurchase| {
            token_position_purchase.token_position_positor == *user_id && token_position_purchase.token_payout_data.token_transfer.is_none() 
        })
        .fold(0, |cummulator: Tokens, token_position_purchase_with_the_user_as_the_positor_with_unpaid_tokens: &TokenPositionPurchase| {
            cummulator + token_position_purchase_with_the_user_as_the_positor_with_unpaid_tokens.tokens + localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE)
        })
}



pub enum DoCyclesPayoutError {
    CandidError(CandidError),
    CMCallCallPerformError(u32),
    CMCallCallError((u32, String)),
    CMCallError(CMCallError),
    ManagementCanisterCallPerformError(u32),
    ManagementCanisterCallError((u32, String)),
}
impl From<CandidError> for DoCyclesPayoutError {
    fn from(ce: CandidError) -> DoCyclesPayoutError {
        DoCyclesPayoutError::CandidError(ce)  
    }
}

// use this enum in the stead of returning the CyclesPayoutData cause we want to make sure the cycles_payout_data is not re-place by this output cause the cycles_transferrer-transfer_cycles_callback can come back before this output is put back on the purchase/vcp. so we use this struct so that only the fields get re-place. 
pub enum DoCyclesPayoutSponse {
    CMCallerCyclesPayoutCallSuccessTimestampNanos(Option<u128>),
    ManagementCanisterPositCyclesCallSuccess(bool),
    NothingToDo
}

type DoCyclesPayoutResult = Result<DoCyclesPayoutSponse, DoCyclesPayoutError>;

async fn do_cycles_payout<T: CyclesPayoutDataTrait>(q: T) -> DoCyclesPayoutResult {
    
    if q.cycles_payout_data().cmcaller_cycles_payout_call_success_timestamp_nanos.is_none() {
        let cmcaller_cycles_payout_call_success_timestamp_nanos: Option<u128>;
        
        let mut call_future = call_raw128(
            with(&CM_DATA, |cm_data| { cm_data.cm_caller }),
            "cm_call",
            &encode_one(
                CMCallQuest{
                    cm_call_id: q.cm_call_id(),
                    for_the_canister: q.cycles_payout_payee(),
                    method: q.cycles_payout_payee_method().to_string(),
                    put_bytes: q.cycles_payout_payee_method_quest_bytes()?,
                    cycles: q.cycles(),
                    cm_callback_method: q.cm_call_callback_method().to_string(),
                }
            )?,
            q.cycles() + 10_000_000_000 // for the cm_caller
        );
                
        if let Poll::Ready(call_result_with_an_error) = futures::poll!(&mut call_future) {
            //cmcaller_cycles_payout_call_success_timestamp_nanos = None;
            return Err(DoCyclesPayoutError::CMCallCallPerformError(call_result_with_an_error.unwrap_err().0 as u32));
        } 
        match call_future.await {
            Ok(sponse_bytes) => match decode_one::<CMCallResult>(&sponse_bytes) {
                Ok(cm_call_result) => match cm_call_result {
                    Ok(()) => {
                        cmcaller_cycles_payout_call_success_timestamp_nanos = Some(time_nanos());
                    },
                    Err(cm_call_error) => {
                        //cmcaller_cycles_payout_call_success_timestamp_nanos = None;
                        return Err(DoCyclesPayoutError::CMCallError(cm_call_error))
                    }
                },
                Err(_candid_decode_error) => {
                    if msg_cycles_refunded128() >= q.cycles() {
                        cmcaller_cycles_payout_call_success_timestamp_nanos = None;
                    } else {
                        cmcaller_cycles_payout_call_success_timestamp_nanos = Some(time_nanos());
                    }    
                }
            },
            Err(cm_call_call_error) => {
                //cmcaller_cycles_payout_call_success_timestamp_nanos = None;
                return Err(DoCyclesPayoutError::CMCallCallError((cm_call_call_error.0 as u32, cm_call_call_error.1)));
            }
        }
        return Ok(DoCyclesPayoutSponse::CMCallerCyclesPayoutCallSuccessTimestampNanos(cmcaller_cycles_payout_call_success_timestamp_nanos));
    }
    
    if let Some((cycles_transfer_refund, _)) = q.cycles_payout_data().cmcaller_cycles_payout_callback_complete {
        if cycles_transfer_refund != 0 
        && q.cycles_payout_data().management_canister_posit_cycles_call_success == false {
            let management_canister_posit_cycles_call_success: bool;
            match call_with_payment128::<(management_canister::CanisterIdRecord,),()>(
                MANAGEMENT_CANISTER_ID,
                "deposit_cycles",
                (management_canister::CanisterIdRecord{
                    canister_id: q.cycles_payout_payee()
                },),
                cycles_transfer_refund
            ).await {
                Ok(_) => {
                    management_canister_posit_cycles_call_success = true;
                },
                Err(_) => {
                    management_canister_posit_cycles_call_success = false;
                }
            }
            return Ok(DoCyclesPayoutSponse::ManagementCanisterPositCyclesCallSuccess(management_canister_posit_cycles_call_success));
        }
    }    
    
    return Ok(DoCyclesPayoutSponse::NothingToDo);
}



enum TokenTransferErrorType {
    TokenTransferError(TokenTransferError),
    TokenTransferCallError((u32, String))
}

enum CMMessageErrorType {
    CMCallQuestCandidEncodeError(CandidError),
    CMCallQuestPutBytesCandidEncodeError(CandidError),
    CMCallerCallError(CMCallError),
    CMCallerCallSponseCandidDecodeError(CandidError),
    CMCallerCallCallError((u32, String))
}

enum DoTokenPayoutSponse {
    TokenTransferError(TokenTransferErrorType),
    TokenTransferSuccessAndCMMessageError(TokenTransferBlockHeightAndTimestampNanos, CMMessageErrorType),
    TokenTransferSuccessAndCMMessageSuccess(TokenTransferBlockHeightAndTimestampNanos, u128),
    //CMMessageError(CMMessageErrorType),
    //CMMessageSuccess(u128),
    NothingForTheDo,
}

async fn do_token_payout<T: TokenPayoutDataTrait>(q: T) -> DoTokenPayoutSponse {
    
    let token_payout_data_token_transfer: TokenTransferBlockHeightAndTimestampNanos = match q.token_payout_data().token_transfer {
        Some(token_transfer_data) => token_transfer_data,
        None => {
            let token_transfer_created_at_time: u64 = time_nanos_u64()-NANOS_IN_A_SECOND as u64;
            match token_transfer(
                TokenTransferArg{
                    memo: q.token_transfer_memo(),
                    amount: q.tokens().into(),
                    fee: Some(q.token_transfer_fee().into()),
                    from_subaccount: Some(principal_token_subaccount(&q.token_payout_payor())),
                    to: IcrcId{owner: ic_cdk::api::id(), subaccount: Some(principal_token_subaccount(&q.token_payout_payee()))},
                    created_at_time: Some(token_transfer_created_at_time)
                }
            ).await {
                Ok(token_transfer_result) => match token_transfer_result {
                    Ok(block_height) => {
                        TokenTransferBlockHeightAndTimestampNanos{
                            block_height: Some(block_height),
                            timestamp_nanos: token_transfer_created_at_time as u128
                        }
                    },
                    Err(token_transfer_error) => {
                        return DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType::TokenTransferError(token_transfer_error));
                    }
                },
                Err(token_transfer_call_error) => {
                    return DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType::TokenTransferCallError(token_transfer_call_error));
                }
            }
        }
    };
    
    match q.token_payout_data().cm_message_call_success_timestamp_nanos {
        Some(cm_message_call_success_timestamp_nanos) => return DoTokenPayoutSponse::NothingForTheDo,
        None => {
            let call_future = call_raw128(
                with(&CM_DATA, |cm_data| { cm_data.cm_caller }),
                "cm_call",
                &match encode_one(
                    CMCallQuest{
                        cm_call_id: q.cm_call_id(),
                        for_the_canister: q.token_payout_payee(),
                        method: q.token_payout_payee_method().to_string(),
                        put_bytes: match q.token_payout_payee_method_quest_bytes(token_payout_data_token_transfer.clone()) {
                            Ok(b) => b,
                            Err(candid_error) => {
                                return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallQuestPutBytesCandidEncodeError(candid_error));     
                            }
                        },
                        cycles: 0,
                        cm_callback_method: q.cm_call_callback_method().to_string(),
                    }
                ) {
                    Ok(b) => b,
                    Err(candid_error) => {
                        return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallQuestCandidEncodeError(candid_error));
                    }
                },
                0 + 10_000_000_000 // for the cm_caller
            );
            match call_future.await {
                Ok(b) => match decode_one::<CMCallResult>(&b) {
                    Ok(cm_call_sponse) => match cm_call_sponse {
                        Ok(()) => {
                            return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageSuccess(token_payout_data_token_transfer, time_nanos());
                        },
                        Err(cm_call_error) => {
                            return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallerCallError(cm_call_error));
                        }
                    },
                    Err(candid_error) => {
                        return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallerCallSponseCandidDecodeError(candid_error));                    
                    }
                },
                Err(call_error) => {
                    return DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_payout_data_token_transfer, CMMessageErrorType::CMCallerCallCallError((call_error.0 as u32, call_error.1)));                    
                } 
            }
        }
    }
    
}



async fn _do_payouts() {

    let mut void_cycles_positions_cycles_payouts_chunk: Vec<(VoidCyclesPositionId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new();
    let mut void_token_positions_token_payouts_chunk: Vec<(VoidTokenPositionId, _/*anonymous-future of the do_token_payout-async-function*/)> = Vec::new();
    let mut cycles_positions_purchases_cycles_payouts_chunk: Vec<(CyclesPositionPurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut cycles_positions_purchases_token_payouts_chunk: Vec<(CyclesPositionPurchaseId, _/*anonymous-future of the token_transfer-function*/)> = Vec::new();
    let mut token_positions_purchases_cycles_payouts_chunk: Vec<(TokenPositionPurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut token_positions_purchases_token_payouts_chunk: Vec<(TokenPositionPurchaseId, _/*anonymous-future of the token_transfer-function*/)> = Vec::new();

    with_mut(&CM_DATA, |cm_data| {
        let mut i: usize = 0;
        while i < cm_data.void_cycles_positions.len() && void_cycles_positions_cycles_payouts_chunk.len() < DO_VOID_CYCLES_POSITIONS_CYCLES_PAYOUTS_CHUNK_SIZE {
            let vcp: &mut VoidCyclesPosition = &mut cm_data.void_cycles_positions[i];
            if vcp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() {
                if time_nanos().saturating_sub(vcp.cycles_payout_data.cmcaller_cycles_payout_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK {
                    std::mem::drop(vcp);
                    cm_data.void_cycles_positions.remove(i);
                    continue;
                }
                // skip
            } else if vcp.cycles_payout_lock == true { 
                // skip
            } else {
                vcp.cycles_payout_lock = true;
                void_cycles_positions_cycles_payouts_chunk.push(
                    (
                        vcp.position_id,
                        do_cycles_payout(vcp.clone())
                    )
                );
            }
            i += 1;
        }
        
        let mut i: usize = 0;
        while i < cm_data.void_token_positions.len() && void_token_positions_token_payouts_chunk.len() < DO_VOID_TOKEN_POSITIONS_TOKEN_PAYOUTS_CHUNK_SIZE {
            let vip: &mut VoidTokenPosition = &mut cm_data.void_token_positions[i];
            if vip.token_payout_data.is_waiting_for_the_cmcaller_callback() {
                if time_nanos().saturating_sub(vip.token_payout_data.cm_message_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK {
                    std::mem::drop(vip);
                    cm_data.void_token_positions.remove(i);
                    continue;
                }
                // skip
            } else if vip.token_payout_lock == true { 
                // skip
            } else {
                vip.token_payout_lock = true;
                void_token_positions_token_payouts_chunk.push(
                    (
                        vip.position_id,
                        do_token_payout(vip.clone())
                    )
                );
            }
            i += 1;
        }
        
        
        let mut i: usize = 0;
        while i < cm_data.cycles_positions_purchases.len() {
            let cpp: &mut CyclesPositionPurchase = &mut cm_data.cycles_positions_purchases[i];                    
            if cpp.cycles_payout_data.is_complete() == false 
            && cpp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() == false
            && cpp.cycles_payout_lock == false
            && cycles_positions_purchases_cycles_payouts_chunk.len() < DO_CYCLES_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE {
                cpp.cycles_payout_lock = true;    
                cycles_positions_purchases_cycles_payouts_chunk.push(
                    (
                        cpp.id,
                        do_cycles_payout(cpp.clone())
                    )
                );
            }
            if cpp.token_payout_data.is_complete() == false
            && cpp.token_payout_data.is_waiting_for_the_cmcaller_callback() == false
            && cpp.token_payout_lock == false
            && cycles_positions_purchases_token_payouts_chunk.len() < DO_CYCLES_POSITIONS_PURCHASES_TOKEN_PAYOUTS_CHUNK_SIZE {
                cpp.token_payout_lock = true;
                cycles_positions_purchases_token_payouts_chunk.push(
                    (     
                        cpp.id,
                        do_token_payout(cpp.clone())                        
                    )
                );
            }
            i += 1;
        }
        
        let mut i: usize = 0;
        while i < cm_data.token_positions_purchases.len() {
            let ipp: &mut TokenPositionPurchase = &mut cm_data.token_positions_purchases[i];                    
            if ipp.cycles_payout_data.is_complete() == false 
            && ipp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() == false
            && ipp.cycles_payout_lock == false
            && token_positions_purchases_cycles_payouts_chunk.len() < DO_TOKEN_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE {
                ipp.cycles_payout_lock = true;
                token_positions_purchases_cycles_payouts_chunk.push(
                    (
                        ipp.id,
                        do_cycles_payout(ipp.clone())
                    )
                );
            }
            if ipp.token_payout_data.is_complete() == false
            && ipp.token_payout_data.is_waiting_for_the_cmcaller_callback() == false
            && ipp.token_payout_lock == false                                                        
            && token_positions_purchases_token_payouts_chunk.len() < DO_TOKEN_POSITIONS_PURCHASES_TOKEN_PAYOUTS_CHUNK_SIZE {
                ipp.token_payout_lock = true;
                token_positions_purchases_token_payouts_chunk.push(
                    (     
                        ipp.id,
                        do_token_payout(ipp.clone())
                    )
                );
            }
            i += 1;
        }
        
    });

    let (vcps_ids, vcps_do_cycles_payouts_futures): (Vec<VoidCyclesPositionId>, Vec<_/*do_cycles_payout-future*/>) = void_cycles_positions_cycles_payouts_chunk.into_iter().unzip();
    let (vips_ids, vips_do_token_payouts_futures): (Vec<VoidTokenPositionId>, Vec<_/*do_token_payout-future*/>) = void_token_positions_token_payouts_chunk.into_iter().unzip();
    let (cpps_cycles_payouts_ids, cpps_do_cycles_payouts_futures): (Vec<CyclesPositionPurchaseId>, Vec<_/*do_cycles_payout-future*/>) = cycles_positions_purchases_cycles_payouts_chunk.into_iter().unzip();
    let (cpps_token_payouts_ids, cpps_do_token_payouts_futures): (Vec<CyclesPositionPurchaseId>, Vec<_/*do_token_payout-future*/>) = cycles_positions_purchases_token_payouts_chunk.into_iter().unzip();
    let (ipps_cycles_payouts_ids, ipps_do_cycles_payouts_futures): (Vec<TokenPositionPurchaseId>, Vec<_/*do_cycles_payout-future*/>) = token_positions_purchases_cycles_payouts_chunk.into_iter().unzip();
    let (ipps_token_payouts_ids, ipps_do_token_payouts_futures): (Vec<TokenPositionPurchaseId>, Vec<_/*do_token_payout-future*/>) = token_positions_purchases_token_payouts_chunk.into_iter().unzip();
    
    let (
        vcps_do_cycles_payouts_rs,
        vips_do_token_payouts_rs,
        cpps_do_cycles_payouts_rs,
        cpps_do_token_payouts_rs,
        ipps_do_cycles_payouts_rs,
        ipps_do_token_payouts_rs
    ): (
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoTokenPayoutSponse>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoTokenPayoutSponse>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoTokenPayoutSponse>,
    ) = futures::join!(
        futures::future::join_all(vcps_do_cycles_payouts_futures),
        futures::future::join_all(vips_do_token_payouts_futures),
        futures::future::join_all(cpps_do_cycles_payouts_futures),
        futures::future::join_all(cpps_do_token_payouts_futures),
        futures::future::join_all(ipps_do_cycles_payouts_futures),
        futures::future::join_all(ipps_do_token_payouts_futures),
    );

    with_mut(&CM_DATA, |cm_data| {
        for (vcp_id, do_cycles_payout_result) in vcps_ids.into_iter().zip(vcps_do_cycles_payouts_rs.into_iter()) {      
            let vcp_void_cycles_positions_i: usize = {
                match cm_data.void_cycles_positions.binary_search_by_key(&vcp_id, |vcp| { vcp.position_id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }  
            };
            let vcp: &mut VoidCyclesPosition = &mut cm_data.void_cycles_positions[vcp_void_cycles_positions_i];
            vcp.cycles_payout_lock = false;
            vcp.cycles_payout_data.handle_do_cycles_payout_result(do_cycles_payout_result);
            if vcp.cycles_payout_data.is_complete() {
                std::mem::drop(vcp);
                cm_data.void_cycles_positions.remove(vcp_void_cycles_positions_i);
            }
        }
        for (vip_id, do_token_payout_sponse) in vips_ids.into_iter().zip(vips_do_token_payouts_rs.into_iter()) {      
            let vip_void_token_positions_i: usize = {
                match cm_data.void_token_positions.binary_search_by_key(&vip_id, |vip| { vip.position_id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }  
            };
            let vip: &mut VoidTokenPosition = &mut cm_data.void_token_positions[vip_void_token_positions_i];
            vip.token_payout_lock = false;
            vip.token_payout_data.handle_do_token_payout_sponse(do_token_payout_sponse);
            if vip.token_payout_data.is_complete() {
                std::mem::drop(vip);
                cm_data.void_token_positions.remove(vip_void_token_positions_i);
            }
        }
        for (cpp_id, do_cycles_payout_result) in cpps_cycles_payouts_ids.into_iter().zip(cpps_do_cycles_payouts_rs.into_iter()) {
            let cpp_cycles_positions_purchases_i: usize = {
                match cm_data.cycles_positions_purchases.binary_search_by_key(&cpp_id, |cpp| { cpp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let cpp: &mut CyclesPositionPurchase = &mut cm_data.cycles_positions_purchases[cpp_cycles_positions_purchases_i];
            cpp.cycles_payout_lock = false;
            cpp.cycles_payout_data.handle_do_cycles_payout_result(do_cycles_payout_result);
        }
        for (cpp_id, do_token_payout_sponse) in cpps_token_payouts_ids.into_iter().zip(cpps_do_token_payouts_rs.into_iter()) {
            let cpp_cycles_positions_purchases_i: usize = {
                match cm_data.cycles_positions_purchases.binary_search_by_key(&cpp_id, |cpp| { cpp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let cpp: &mut CyclesPositionPurchase = &mut cm_data.cycles_positions_purchases[cpp_cycles_positions_purchases_i];
            cpp.token_payout_lock = false;
            cpp.token_payout_data.handle_do_token_payout_sponse(do_token_payout_sponse);
        }
        for (ipp_id, do_cycles_payout_result) in ipps_cycles_payouts_ids.into_iter().zip(ipps_do_cycles_payouts_rs.into_iter()) {
            let ipp_token_positions_purchases_i: usize = {
                match cm_data.token_positions_purchases.binary_search_by_key(&ipp_id, |ipp| { ipp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let ipp: &mut TokenPositionPurchase = &mut cm_data.token_positions_purchases[ipp_token_positions_purchases_i];
            ipp.cycles_payout_lock = false;
            ipp.cycles_payout_data.handle_do_cycles_payout_result(do_cycles_payout_result);
        }
        for (ipp_id, do_token_payout_sponse) in ipps_token_payouts_ids.into_iter().zip(ipps_do_token_payouts_rs.into_iter()) {
            let ipp_token_positions_purchases_i: usize = {
                match cm_data.token_positions_purchases.binary_search_by_key(&ipp_id, |ipp| { ipp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let ipp: &mut TokenPositionPurchase = &mut cm_data.token_positions_purchases[ipp_token_positions_purchases_i];
            ipp.token_payout_lock = false;
            ipp.token_payout_data.handle_do_token_payout_sponse(do_token_payout_sponse);
        }
        
    });
    
}


async fn do_payouts() {
    
    if with(&CM_DATA, |cm_data| { 
        cm_data.void_cycles_positions.len() == 0
        && cm_data.cycles_positions_purchases.len() == 0
        && cm_data.token_positions_purchases.len() == 0
    }) { return; }

    match call::<(),()>(
        ic_cdk::api::id(),
        "do_payouts_public_method",
        (),
    ).await {
        Ok(()) => {},
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| {
                cm_data.do_payouts_errors.push((call_error.0 as u32, call_error.1));
            });
        }
    }
}

#[update]
pub async fn do_payouts_public_method() {
    if [ic_cdk::api::id(), with(&CM_DATA, |cm_data| { cm_data.cts_id })].contains(&caller()) == false {
        trap("caller without the authorization.");
    }
    
    _do_payouts().await;

}


// -------------------------------------------------------------





#[update(manual_reply = true)]
pub async fn create_cycles_position(q: CreateCyclesPositionQuest) { // -> Result<CreateCyclesPositionSuccess, CreateCyclesPositionError> {

    let positor: Principal = caller();

    if q.minimum_purchase > q.cycles {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumPurchaseMustBeEqualOrLessThanTheCyclesPosition),));    
        do_payouts().await;
        return;
    }

    if q.cycles < MINIMUM_CYCLES_POSITION {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumCyclesPosition(MINIMUM_CYCLES_POSITION)),));
        do_payouts().await;
        return;
    }
    
    if q.minimum_purchase == 0 {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumPurchaseCannotBeZero),));
        do_payouts().await;
        return;
    }

    if q.cycles % q.cycles_per_token_rate != 0 {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMustBeAMultipleOfTheCyclesPerTokenRate),));
        do_payouts().await;
        return;
    }

    if q.minimum_purchase % q.cycles_per_token_rate != 0 {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumPurchaseMustBeAMultipleOfTheCyclesPerTokenRate),));
        do_payouts().await;
        return;
    }

    let msg_cycles_quirement: Cycles = CREATE_POSITION_FEE.checked_add(q.cycles).unwrap_or(Cycles::MAX); 

    if msg_cycles_available128() < msg_cycles_quirement {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MsgCyclesTooLow{ create_position_fee: CREATE_POSITION_FEE  }),));
        do_payouts().await;
        return;
    }

    if canister_balance128().checked_add(msg_cycles_quirement).is_none() {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsFull),));
        do_payouts().await;
        return;
    }

    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.cycles_positions.len() >= MAX_CYCLES_POSITIONS {
            if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsBusy),));
                return Err(());
            }
            // new
            // :highest-cost-position of the cycles-positions.
            let (
                cycles_position_with_the_lowest_cycles_per_token_rate_cycles_positions_i,
                cycles_position_with_the_lowest_cycles_per_token_rate_ref
            ): (usize, &CyclesPosition) = { 
                cm_data.cycles_positions.iter()
                    .enumerate()
                    .min_by_key(|(_,cycles_position): &(usize,&CyclesPosition)| { cycles_position.cycles_per_token_rate })
                    .unwrap()
            };
            if q.cycles_per_token_rate > cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles_per_token_rate 
            && q.cycles >= cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles {
                // bump
                
                std::mem::drop(cycles_position_with_the_lowest_cycles_per_token_rate_ref);
                
                let cycles_position_lowest_cycles_per_token_rate: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_with_the_lowest_cycles_per_token_rate_cycles_positions_i);
                
                let cycles_position_lowest_cycles_per_token_rate_void_cycles_positions_insertion_i = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_cycles_per_token_rate.id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_lowest_cycles_per_token_rate_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_lowest_cycles_per_token_rate.id,
                        positor:        cycles_position_lowest_cycles_per_token_rate.positor,
                        cycles:         cycles_position_lowest_cycles_per_token_rate.cycles,
                        cycles_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time_nanos()
                    }
                );
                Ok(())
            } else {
                reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles_per_token_rate + 1, minimum_cycles_position_for_a_bump: cycles_position_with_the_lowest_cycles_per_token_rate_ref.cycles }),));
                return Err(());
            }

            
            
            
            /*
            // old
            let cycles_position_highest_cycles_per_token_rate: CyclesPerToken = { 
                cm_data.cycles_positions.iter()
                    .max_by_key(|cycles_position: &&CyclesPosition| { cycles_position.cycles_per_token_rate })
                    .unwrap()
                    .cycles_per_token_rate
            };
            if q.cycles_per_token_rate > cycles_position_highest_cycles_per_token_rate && q.cycles >= MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP {
                // bump
                let cycles_position_lowest_cycles_per_token_rate_position_id: PositionId = {
                    cm_data.cycles_positions.iter()
                        .min_by_key(|cycles_position: &&CyclesPosition| { cycles_position.cycles_per_token_rate })
                        .unwrap()
                        .id
                };
                let cycles_position_lowest_cycles_per_token_rate_cycles_positions_i: usize = {
                    cm_data.cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_cycles_per_token_rate_position_id,
                        |cycles_position| { cycles_position.id }
                    ).unwrap()
                };
                let cycles_position_lowest_cycles_per_token_rate: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_lowest_cycles_per_token_rate_cycles_positions_i);
                if cycles_position_lowest_cycles_per_token_rate.id != cycles_position_lowest_cycles_per_token_rate_position_id { trap("outside the bounds of the contract.") }
                let cycles_position_lowest_cycles_per_token_rate_void_cycles_positions_insertion_i = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_cycles_per_token_rate_position_id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_lowest_cycles_per_token_rate_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_lowest_cycles_per_token_rate.id,
                        positor:        cycles_position_lowest_cycles_per_token_rate.positor,
                        cycles:         cycles_position_lowest_cycles_per_token_rate.cycles,
                        cycles_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time_nanos()
                    }
                );
                Ok(())
            } else {
                reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: cycles_position_highest_cycles_per_token_rate + 1, minimum_cycles_position_for_a_bump: MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP }),));
                return Err(());
            }
            */
        } else {
            Ok(())
        }
    }) {
        Ok(()) => {},
        Err(()) => {
            do_payouts().await;
            return;
        }
    }
        
    
    let position_id: PositionId = with_mut(&CM_DATA, |cm_data| {
        let id: PositionId = new_id(&mut cm_data.id_counter); 
        cm_data.cycles_positions.push(
            CyclesPosition{
                id,   
                positor,
                cycles: q.cycles,
                minimum_purchase: q.minimum_purchase,
                cycles_per_token_rate: q.cycles_per_token_rate,
                timestamp_nanos: time_nanos(),
            }
        );
        id
    });
    
    msg_cycles_accept128(msg_cycles_quirement);

    reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Ok(
        CreateCyclesPositionSuccess{
            position_id
        }
    ),));
    
    do_payouts().await;
    return;
}




// ------------------



#[update(manual_reply = true)]
pub async fn create_token_position(q: CreateTokenPositionQuest) { //-> Result<CreateTokenPositionSuccess,CreateTokenPositionError> {

    let positor: Principal = caller();

    if q.minimum_purchase > q.tokens {
        reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::MinimumPurchaseMustBeEqualOrLessThanTheTokenPosition),));    
        do_payouts().await;
        return;
    }

    if q.tokens < MINIMUM_TOKEN_POSITION {
        reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::MinimumTokenPosition(MINIMUM_TOKEN_POSITION)),));
        do_payouts().await;
        return;
    }
    
    if q.minimum_purchase == 0 {
        reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::MinimumPurchaseCannotBeZero),));
        do_payouts().await;
        return;
    }


    let msg_cycles_quirement: Cycles = CREATE_POSITION_FEE; 

    if msg_cycles_available128() < msg_cycles_quirement {
        reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::MsgCyclesTooLow{ create_position_fee: CREATE_POSITION_FEE  }),));
        do_payouts().await;
        return;
    }

    if canister_balance128().checked_add(msg_cycles_quirement).is_none() {
        reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::CyclesMarketIsFull),));
        do_payouts().await;
        return;
    }

    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_token_balance_locks.len() >= MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS {
            reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::CyclesMarketIsBusy),));
            return Err(());
        }
        if cm_data.mid_call_user_token_balance_locks.contains(&positor) {
            reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall),));
            return Err(());
        }
        cm_data.mid_call_user_token_balance_locks.insert(positor);
        Ok(())
    }) {
        Ok(()) => {},
        Err(()) => {
            do_payouts().await;
            return;
        }
    }
    
    // check token balance and make sure to unlock the user on returns after here 
    let user_token_ledger_balance: Tokens = match check_user_cycles_market_token_ledger_balance(&positor).await {
        Ok(token_ledger_balance) => token_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&positor); });
            reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::CheckUserCyclesMarketTokenLedgerBalanceError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &positor) });
    
    let usable_user_token_balance: Tokens = user_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock);
    
    if usable_user_token_balance < q.tokens + ( q.tokens / q.minimum_purchase * localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) ) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&positor); });
        reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::UserTokenBalanceTooLow{ user_token_balance: usable_user_token_balance }),));
        do_payouts().await;
        return;
    }
    
    
    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.token_positions.len() >= MAX_TOKEN_POSITIONS {            
            if cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS {
                reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::CyclesMarketIsBusy),));
                return Err(());
            }
            // new
            let (
                token_position_with_the_highest_cycles_per_token_rate_token_positions_i,
                token_position_with_the_highest_cycles_per_token_rate_ref
            ): (usize, &TokenPosition) = {
                cm_data.token_positions.iter()
                    .enumerate()
                    .max_by_key(|(_, token_position): &(usize, &TokenPosition)| { token_position.cycles_per_token_rate })
                    .unwrap()
            };
            if q.cycles_per_token_rate < token_position_with_the_highest_cycles_per_token_rate_ref.cycles_per_token_rate 
            && q.tokens >= token_position_with_the_highest_cycles_per_token_rate_ref.tokens {
                // bump
                
                std::mem::drop(token_position_with_the_highest_cycles_per_token_rate_ref);
                
                let token_position_highest_cycles_per_token_rate: TokenPosition = cm_data.token_positions.remove(token_position_with_the_highest_cycles_per_token_rate_token_positions_i);                
                
                let token_position_highest_cycles_per_token_rate_void_token_positions_insertion_i = { 
                    cm_data.void_token_positions.binary_search_by_key(
                        &token_position_highest_cycles_per_token_rate.id,
                        |void_token_position| { void_token_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_token_positions.insert(
                    token_position_highest_cycles_per_token_rate_void_token_positions_insertion_i,
                    VoidTokenPosition{                        
                        position_id:    token_position_highest_cycles_per_token_rate.id,
                        positor:        token_position_highest_cycles_per_token_rate.positor,
                        tokens:         token_position_highest_cycles_per_token_rate.tokens,
                        timestamp_nanos: time_nanos(),
                        token_payout_lock: false,
                        token_payout_data: TokenPayoutData{
                            token_transfer: Some(TokenTransferBlockHeightAndTimestampNanos{
                                block_height: None,
                                timestamp_nanos: time_nanos(),
                            }),
                            cm_message_call_success_timestamp_nanos: None,
                            cm_message_callback_complete: None            
                        }
                    }
                ); 
                Ok(())
            } else {
                reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::CyclesMarketIsFull_MaximumRateAndMinimumTokenPositionForABump{ maximum_rate_for_a_bump: token_position_with_the_highest_cycles_per_token_rate_ref.cycles_per_token_rate - 1, minimum_token_position_for_a_bump: token_position_with_the_highest_cycles_per_token_rate_ref.tokens }),));
                return Err(());
            }

            
            
            
            /*
            // old
            let token_position_lowest_cycles_per_token_rate: CyclesPerToken = { 
                cm_data.token_positions.iter()
                    .min_by_key(|token_position: &&TokenPosition| { token_position.cycles_per_token_rate })
                    .unwrap()
                    .cycles_per_token_rate
            };
            if q.cycles_per_token_rate < token_position_lowest_cycles_per_token_rate && q.tokens >= MINIMUM_TOKEN_POSITION_FOR_AN_TOKEN_POSITION_BUMP {
                // bump
                let token_position_highest_cycles_per_token_rate_position_id: PositionId = {
                    cm_data.token_positions.iter()
                        .max_by_key(|token_position: &&TokenPosition| { token_position.cycles_per_token_rate })
                        .unwrap()
                        .id
                };
                let token_position_highest_cycles_per_token_rate_token_positions_i: usize = {
                    cm_data.token_positions.binary_search_by_key(
                        &token_position_highest_cycles_per_token_rate_position_id,
                        |token_position| { token_position.id }
                    ).unwrap()
                };
                
                
                let token_position_highest_cycles_per_token_rate: TokenPosition = cm_data.token_positions.remove(token_position_highest_cycles_per_token_rate_token_positions_i);                
                if token_position_highest_cycles_per_token_rate.id != token_position_highest_cycles_per_token_rate_position_id { trap("outside the bounds of the contract.") }
                let token_position_highest_cycles_per_token_rate_void_token_positions_insertion_i = { 
                    cm_data.void_token_positions.binary_search_by_key(
                        &token_position_highest_cycles_per_token_rate_position_id,
                        |void_token_position| { void_token_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_token_positions.insert(
                    token_position_highest_cycles_per_token_rate_void_token_positions_insertion_i,
                    VoidTokenPosition{                        
                        position_id:    token_position_highest_cycles_per_token_rate.id,
                        positor:        token_position_highest_cycles_per_token_rate.positor,
                        tokens:            token_position_highest_cycles_per_token_rate.tokens,
                        timestamp_nanos: time_nanos(),
                        token_payout_lock: false,
                        token_payout_data: TokenPayoutData{
                            token_transfer: Some(TokenTransferBlockHeightAndTimestampNanos{
                                block_height: None,
                                timestamp_nanos: time_nanos(),
                            }),
                            cm_message_call_success_timestamp_nanos: None,
                            cm_message_callback_complete: None            
                        }
                    }
                ); 
                Ok(())
            } else {
                reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Err(CreateTokenPositionError::CyclesMarketIsFull_MaximumRateAndMinimumTokenPositionForABump{ maximum_rate_for_a_bump: token_position_lowest_cycles_per_token_rate - 1, minimum_token_position_for_a_bump: MINIMUM_TOKEN_POSITION_FOR_AN_TOKEN_POSITION_BUMP }),));
                return Err(());
            }
            */
        } else {
            Ok(())    
        }
    }) {
        Ok(()) => {},
        Err(()) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&positor); });
            do_payouts().await;
            return;
        }
    }
    
    
    
    let position_id: PositionId = with_mut(&CM_DATA, |cm_data| {
        let id: PositionId = new_id(&mut cm_data.id_counter); 
        cm_data.token_positions.push(
            TokenPosition{
                id,   
                positor,
                tokens: q.tokens,
                minimum_purchase: q.minimum_purchase,
                cycles_per_token_rate: q.cycles_per_token_rate,
                timestamp_nanos: time_nanos(),
            }
        );
        id
    });
    
    msg_cycles_accept128(msg_cycles_quirement);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&positor); });
    
    reply::<(Result<CreateTokenPositionSuccess, CreateTokenPositionError>,)>((Ok(
        CreateTokenPositionSuccess{
            position_id
        }
    ),));
    do_payouts().await;
    return;
}


// ------------------


#[update(manual_reply = true)]
pub async fn purchase_cycles_position(q: PurchaseCyclesPositionQuest) { // -> Result<PurchaseCyclesPositionSuccess, PurchaseCyclesPositionError>
    
    let purchaser: Principal = caller();
    
    if msg_cycles_available128() < PURCHASE_POSITION_FEE {
        reply::<(PurchaseCyclesPositionResult,)>((Err(PurchaseCyclesPositionError::MsgCyclesTooLow{ purchase_position_fee: PURCHASE_POSITION_FEE }),));
        do_payouts().await;
        return;
    }
    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_token_balance_locks.len() >= MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_token_balance_locks.contains(&purchaser) {
            return Err(PurchaseCyclesPositionError::CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall);
        }
        cm_data.mid_call_user_token_balance_locks.insert(purchaser);
        Ok(())
    }) {
        Ok(()) => {},
        Err(purchase_cycles_position_error) => {
            reply::<(PurchaseCyclesPositionResult,)>((Err(purchase_cycles_position_error),));
            do_payouts().await;
            return;
        }
    }
    
    // check token balance and make sure to unlock the user on returns after here 
    let user_token_ledger_balance: Tokens = match check_user_cycles_market_token_ledger_balance(&purchaser).await {
        Ok(token_ledger_balance) => token_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&purchaser); });
            reply::<(PurchaseCyclesPositionResult,)>((Err(PurchaseCyclesPositionError::CheckUserCyclesMarketTokenLedgerBalanceError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &purchaser) });
    
    let usable_user_token_balance: Tokens = user_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock);

    let cycles_position_purchase_id: PurchaseId = match with_mut(&CM_DATA, |cm_data| {
        if cm_data.cycles_positions_purchases.len() >= MAX_CYCLES_POSITIONS_PURCHASES {
            let remove_cpp_id: PurchaseId = match cm_data.cycles_positions_purchases.iter().filter(
                |cycles_position_purchase: &&CyclesPositionPurchase| {
                    ( cycles_position_purchase.cycles_payout_data.is_complete() || ( cycles_position_purchase.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() && time_nanos().saturating_sub(cycles_position_purchase.cycles_payout_data.cmcaller_cycles_payout_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) ) && ( cycles_position_purchase.token_payout_data.is_complete() || ( cycles_position_purchase.token_payout_data.is_waiting_for_the_cmcaller_callback() && time_nanos().saturating_sub(cycles_position_purchase.token_payout_data.cm_message_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) ) 
                }
            ).min_by_key(
                |cycles_position_purchase: &&CyclesPositionPurchase| {
                    cycles_position_purchase.timestamp_nanos
                }    
            ) {
                None => {
                    return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);    
                },
                Some(remove_cpp) => {
                    remove_cpp.id
                }
            };
            let remove_cpp_cycles_positions_purchases_i: usize = match cm_data.cycles_positions_purchases.binary_search_by_key(&remove_cpp_id, |cpp| { cpp.id }) {
                Ok(i) => i,
                Err(_) => /*will not happen*/return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy),
            };
            cm_data.cycles_positions_purchases.remove(remove_cpp_cycles_positions_purchases_i); 
        }
        let cycles_position_cycles_positions_i: usize = match cm_data.cycles_positions.binary_search_by_key(
            &q.cycles_position_id,
            |cycles_position| { cycles_position.id }
        ) {
            Ok(i) => i,
            Err(_) => { return Err(PurchaseCyclesPositionError::CyclesPositionNotFound); }
        };
        let cycles_position_ref: &CyclesPosition = &cm_data.cycles_positions[cycles_position_cycles_positions_i];
        if q.cycles % cycles_position_ref.cycles_per_token_rate as u128 != 0 {
            return Err(PurchaseCyclesPositionError::PurchaseCyclesMustBeAMultipleOfTheCyclesPerTokenRate);
        }
        if cycles_position_ref.cycles < q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: cycles_position_ref.cycles });
        }
        if cycles_position_ref.minimum_purchase > q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: cycles_position_ref.minimum_purchase });
        }        
        
        if usable_user_token_balance < cycles_transform_tokens(q.cycles, cycles_position_ref.cycles_per_token_rate) + localkey::cell::get(&TOKEN_LEDGER_TRANSFER_FEE) {
            return Err(PurchaseCyclesPositionError::UserTokenBalanceTooLow{ user_token_balance: usable_user_token_balance });
        }
        
        if cycles_position_ref.cycles - q.cycles < cycles_position_ref.minimum_purchase 
        && cycles_position_ref.cycles - q.cycles != 0
        && cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
                
        let cycles_position_purchase_id: PurchaseId = new_id(&mut cm_data.id_counter);
        cm_data.cycles_positions_purchases.push(
            CyclesPositionPurchase {
                cycles_position_id: cycles_position_ref.id,
                cycles_position_positor: cycles_position_ref.positor,
                cycles_position_cycles_per_token_rate: cycles_position_ref.cycles_per_token_rate,
                id: cycles_position_purchase_id,
                purchaser,
                cycles: q.cycles,
                timestamp_nanos: time_nanos(),
                cycles_payout_lock: false,
                token_payout_lock: false,
                cycles_payout_data: CyclesPayoutData::new(),
                token_payout_data: TokenPayoutData{
                    token_transfer: None,
                    cm_message_call_success_timestamp_nanos: None,
                    cm_message_callback_complete: None    
                }
            }
        );

        std::mem::drop(cycles_position_ref);
        cm_data.cycles_positions[cycles_position_cycles_positions_i].cycles -= q.cycles;
        if cm_data.cycles_positions[cycles_position_cycles_positions_i].cycles < cm_data.cycles_positions[cycles_position_cycles_positions_i].minimum_purchase {
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_cycles_positions_i);
            if cycles_position_for_the_void.cycles != 0 {
                let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_for_the_void.id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_for_the_void_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_for_the_void.id,
                        positor:        cycles_position_for_the_void.positor,
                        cycles:         cycles_position_for_the_void.cycles,
                        cycles_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time_nanos()
                    }
                );
            }
        }   
        
        Ok(cycles_position_purchase_id)
    }) {
        Ok(cycles_position_purchase_id) => cycles_position_purchase_id,
        Err(purchase_cycles_position_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&purchaser); });
            reply::<(PurchaseCyclesPositionResult,)>((Err(purchase_cycles_position_error),));
            do_payouts().await;
            return;
        }
    };
    
    msg_cycles_accept128(PURCHASE_POSITION_FEE);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&purchaser); });
    reply::<(PurchaseCyclesPositionResult,)>((Ok(PurchaseCyclesPositionSuccess{
        purchase_id: cycles_position_purchase_id
    }),));
    do_payouts().await;
    return;

}





// -------------------



#[update(manual_reply = true)]
pub async fn purchase_token_position(q: PurchaseTokenPositionQuest) {

    let purchaser: Principal = caller();

    let token_position_purchase_id: PurchaseId = match with_mut(&CM_DATA, |cm_data| {
        if cm_data.token_positions_purchases.len() >= MAX_TOKEN_POSITIONS_PURCHASES {
            let remove_ipp_id: PurchaseId = match cm_data.token_positions_purchases.iter().filter(
                |token_position_purchase: &&TokenPositionPurchase| {
                    ( token_position_purchase.cycles_payout_data.is_complete() || ( token_position_purchase.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() && time_nanos().saturating_sub(token_position_purchase.cycles_payout_data.cmcaller_cycles_payout_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) ) && ( token_position_purchase.token_payout_data.is_complete() || ( token_position_purchase.token_payout_data.is_waiting_for_the_cmcaller_callback() && time_nanos().saturating_sub(token_position_purchase.token_payout_data.cm_message_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) )
                }
            ).min_by_key(
                |token_position_purchase: &&TokenPositionPurchase| {
                    token_position_purchase.timestamp_nanos
                }    
            ) {
                None => {
                    return Err(PurchaseTokenPositionError::CyclesMarketIsBusy);    
                },
                Some(remove_ipp) => {
                    remove_ipp.id
                }
            };
            let remove_ipp_token_positions_purchases_i: usize = match cm_data.token_positions_purchases.binary_search_by_key(&remove_ipp_id, |ipp| { ipp.id }) {
                Ok(i) => i,
                Err(_) => /*will not happen*/return Err(PurchaseTokenPositionError::CyclesMarketIsBusy),
            };
            cm_data.token_positions_purchases.remove(remove_ipp_token_positions_purchases_i);            
        }
        let token_position_token_positions_i: usize = match cm_data.token_positions.binary_search_by_key(
            &q.token_position_id,
            |token_position| { token_position.id }
        ) {
            Ok(i) => i,
            Err(_) => { return Err(PurchaseTokenPositionError::TokenPositionNotFound); }
        };
        let token_position_ref: &TokenPosition = &cm_data.token_positions[token_position_token_positions_i];
        if token_position_ref.tokens < q.tokens {
            return Err(PurchaseTokenPositionError::TokenPositionTokenIsLessThanThePurchaseQuest{ token_position_tokens: token_position_ref.tokens.clone() });
        }
        if token_position_ref.minimum_purchase > q.tokens {
            return Err(PurchaseTokenPositionError::TokenPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ token_position_minimum_purchase: token_position_ref.minimum_purchase.clone() });
        }        

        if &token_position_ref.tokens - &q.tokens < token_position_ref.minimum_purchase 
        && token_position_ref.tokens - q.tokens != 0
        && cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS {
            return Err(PurchaseTokenPositionError::CyclesMarketIsBusy);
        }
        

        let msg_cycles_quirement: Cycles = PURCHASE_POSITION_FEE + tokens_transform_cycles(q.tokens, token_position_ref.cycles_per_token_rate); 
        if msg_cycles_available128() < msg_cycles_quirement {
            return Err(PurchaseTokenPositionError::MsgCyclesTooLow{ purchase_position_fee: PURCHASE_POSITION_FEE });
        }
        msg_cycles_accept128(msg_cycles_quirement);
                
        let token_position_purchase_id: PurchaseId = new_id(&mut cm_data.id_counter);
        
        cm_data.token_positions_purchases.push(
            TokenPositionPurchase{
                token_position_id: token_position_ref.id,
                token_position_positor: token_position_ref.positor,
                token_position_cycles_per_token_rate: token_position_ref.cycles_per_token_rate,
                id: token_position_purchase_id,
                purchaser,
                tokens: q.tokens,
                timestamp_nanos: time_nanos(),
                cycles_payout_lock: false,
                token_payout_lock: false,
                cycles_payout_data: CyclesPayoutData::new(),
                token_payout_data: TokenPayoutData{
                    token_transfer: None,
                    cm_message_call_success_timestamp_nanos: None,
                    cm_message_callback_complete: None    
                }
            }
        );

        std::mem::drop(token_position_ref);
        cm_data.token_positions[token_position_token_positions_i].tokens -= q.tokens;
        if cm_data.token_positions[token_position_token_positions_i].tokens < cm_data.token_positions[token_position_token_positions_i].minimum_purchase {            
            let token_position_for_the_void: TokenPosition = cm_data.token_positions.remove(token_position_token_positions_i);
            if token_position_for_the_void.tokens != 0 {
                let token_position_for_the_void_void_token_positions_insertion_i: usize = { 
                    cm_data.void_token_positions.binary_search_by_key(
                        &token_position_for_the_void.id,
                        |void_token_position| { void_token_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_token_positions.insert(
                    token_position_for_the_void_void_token_positions_insertion_i,
                    VoidTokenPosition{
                        position_id:    token_position_for_the_void.id,
                        positor:        token_position_for_the_void.positor,
                        tokens:            token_position_for_the_void.tokens,
                        timestamp_nanos: time_nanos(),
                        token_payout_lock: false,
                        token_payout_data: TokenPayoutData{
                            token_transfer: Some(TokenTransferBlockHeightAndTimestampNanos{
                                block_height: None,
                                timestamp_nanos: time_nanos(),
                            }),
                            cm_message_call_success_timestamp_nanos: None,
                            cm_message_callback_complete: None            
                        }
                    }
                );
            }    
            
        }
        
        Ok(token_position_purchase_id)
    }) {
        Ok(token_position_purchase_id) => token_position_purchase_id,
        Err(purchase_token_position_error) => {
            reply::<(PurchaseTokenPositionResult,)>((Err(purchase_token_position_error),));
            do_payouts().await;
            return;
        }
    };
    
    
    reply::<(PurchaseTokenPositionResult,)>((Ok(PurchaseTokenPositionSuccess{
        purchase_id: token_position_purchase_id
    }),));
    do_payouts().await;
    return;
    
}




// --------------------------



#[update(manual_reply = true)]
pub async fn void_position(q: VoidPositionQuest) {
    match with_mut(&CM_DATA, |cm_data| {
        if let Ok(cycles_position_i) = cm_data.cycles_positions.binary_search_by_key(&q.position_id, |cycles_position| { cycles_position.id }) {
            if cm_data.cycles_positions[cycles_position_i].positor != caller() {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(cm_data.cycles_positions[cycles_position_i].timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: cm_data.cycles_positions[cycles_position_i].timestamp_nanos/NANOS_IN_A_SECOND });
            }  
            if cm_data.void_cycles_positions.len() >= MAX_VOID_CYCLES_POSITIONS {
                return Err(VoidPositionError::CyclesMarketIsBusy);
            }
            let cycles_position_for_the_void: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_i);
            let cycles_position_for_the_void_void_cycles_positions_insertion_i: usize = cm_data.void_cycles_positions.binary_search_by_key(&cycles_position_for_the_void.id, |vcp| { vcp.position_id }).unwrap_err();
            cm_data.void_cycles_positions.insert(
                cycles_position_for_the_void_void_cycles_positions_insertion_i,
                VoidCyclesPosition{
                    position_id:    cycles_position_for_the_void.id,
                    positor:        cycles_position_for_the_void.positor,
                    cycles:         cycles_position_for_the_void.cycles,
                    cycles_payout_lock: false,
                    cycles_payout_data: CyclesPayoutData::new(),
                    timestamp_nanos: time_nanos()                
                }
            );
            Ok(())
        } else if let Ok(token_position_i) = cm_data.token_positions.binary_search_by_key(&q.position_id, |token_position| { token_position.id }) {
            if cm_data.token_positions[token_position_i].positor != caller() {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(cm_data.token_positions[token_position_i].timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: cm_data.token_positions[token_position_i].timestamp_nanos/NANOS_IN_A_SECOND });
            }
            if cm_data.void_token_positions.len() >= MAX_VOID_TOKEN_POSITIONS {
                return Err(VoidPositionError::CyclesMarketIsBusy);
            }
            let token_position_for_the_void: TokenPosition = cm_data.token_positions.remove(token_position_i);
            let token_position_for_the_void_void_token_positions_insertion_i: usize = cm_data.void_token_positions.binary_search_by_key(&token_position_for_the_void.id, |vip| { vip.position_id }).unwrap_err();
            cm_data.void_token_positions.insert(
                token_position_for_the_void_void_token_positions_insertion_i,
                VoidTokenPosition{
                    position_id:    token_position_for_the_void.id,
                    positor:        token_position_for_the_void.positor,
                    tokens:            token_position_for_the_void.tokens,
                    timestamp_nanos: time_nanos(),
                    token_payout_lock: false,
                    token_payout_data: TokenPayoutData{
                        token_transfer: Some(TokenTransferBlockHeightAndTimestampNanos{
                            block_height: None,
                            timestamp_nanos: time_nanos(),
                        }),
                        cm_message_call_success_timestamp_nanos: None,
                        cm_message_callback_complete: None            
                    }
                }
            );
            Ok(())
        } else {
            return Err(VoidPositionError::PositionNotFound);
        }
    }) {
        Ok(()) => {
            reply::<(VoidPositionResult,)>((Ok(()),));
        },
        Err(void_position_error) => {
            reply::<(VoidPositionResult,)>((Err(void_position_error),));
        }
    }
    
    do_payouts().await;
    return;    
}


// ----------------

#[derive(CandidType, Deserialize)]
pub struct SeeTokenLockQuest {
    principal_id: Principal,
}

#[query]
pub fn see_token_lock(q: SeeTokenLockQuest) -> Tokens {
    with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &q.principal_id) })
}


// ----------------


#[update(manual_reply = true)]
pub async fn transfer_token_balance(q: TransferTokenBalanceQuest) {
    
    let user_id: Principal = caller();
    
    if msg_cycles_available128() < TRANSFER_TOKEN_BALANCE_FEE {
        reply::<(TransferTokenBalanceResult,)>((Err(TransferTokenBalanceError::MsgCyclesTooLow{ transfer_token_balance_fee: TRANSFER_TOKEN_BALANCE_FEE }),));
        do_payouts().await;
        return;
    }

    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_token_balance_locks.len() >= MAX_MID_CALL_USER_TOKEN_BALANCE_LOCKS {
            return Err(TransferTokenBalanceError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_token_balance_locks.contains(&user_id) {
            return Err(TransferTokenBalanceError::CallerIsInTheMiddleOfACreateTokenPositionOrPurchaseCyclesPositionOrTransferTokenBalanceCall);
        }
        cm_data.mid_call_user_token_balance_locks.insert(user_id);
        Ok(())
    }) {
        Ok(()) => {},
        Err(transfer_token_balance_error) => {
            reply::<(TransferTokenBalanceResult,)>((Err(transfer_token_balance_error),));
            do_payouts().await;
            return;
        }
    }
    
    // check token balance and make sure to unlock the user on returns after here 
    let user_token_ledger_balance: Tokens = match check_user_cycles_market_token_ledger_balance(&user_id).await {
        Ok(token_ledger_balance) => token_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });
            reply::<(TransferTokenBalanceResult,)>((Err(TransferTokenBalanceError::CheckUserCyclesMarketTokenLedgerBalanceCallError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    };
    
    let user_token_balance_in_the_lock: Tokens = with(&CM_DATA, |cm_data| { check_user_token_balance_in_the_lock(cm_data, &user_id) });
    
    let usable_user_token_balance: Tokens = user_token_ledger_balance.saturating_sub(user_token_balance_in_the_lock);
    
    if usable_user_token_balance < q.tokens.saturating_add(q.token_fee) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });
        reply::<(TransferTokenBalanceResult,)>((Err(TransferTokenBalanceError::UserTokenBalanceTooLow{ user_token_balance: usable_user_token_balance }),));
        do_payouts().await;
        return;          
    }

    match token_transfer(
        TokenTransferArg {
            memo: Some(IcrcMemo(ByteBuf::from(*TRANSFER_TOKEN_BALANCE_MEMO))),
            amount: q.tokens.into(),
            fee: Some(q.token_fee.into()),
            from_subaccount: Some(principal_token_subaccount(&user_id)),
            to: q.to,
            created_at_time: Some(q.created_at_time.unwrap_or(time_nanos_u64()))
        }   
    ).await {
        Ok(token_transfer_result) => match token_transfer_result {
            Ok(token_transfer_block_height) => {
                msg_cycles_accept128(TRANSFER_TOKEN_BALANCE_FEE);
                reply::<(TransferTokenBalanceResult,)>((Ok(token_transfer_block_height),));
            },
            Err(token_transfer_error) => {
                match token_transfer_error {
                    TokenTransferError::BadFee{ .. } => {
                        msg_cycles_accept128(TRANSFER_TOKEN_BALANCE_FEE);
                    },
                    _ => {}
                }
                reply::<(TransferTokenBalanceResult,)>((Err(TransferTokenBalanceError::TokenTransferError(token_transfer_error)),));
            }
        },
        Err(token_transfer_call_error) => {
            reply::<(TransferTokenBalanceResult,)>((Err(TransferTokenBalanceError::TokenTransferCallError(token_transfer_call_error)),));
        }
    }

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_token_balance_locks.remove(&user_id); });
    do_payouts().await;
    return;
}



// -------------------------------

#[query(manual_reply = true)]
pub fn download_cycles_positions_rchunks(q: DownloadRChunkQuest) {
    with(&CM_DATA, |cm_data| {
        reply((rchunk_data(q, &cm_data.cycles_positions),));
    });
}

#[query(manual_reply = true)]
pub fn download_token_positions_rchunks(q: DownloadRChunkQuest) {
    with(&CM_DATA, |cm_data| {
        reply((rchunk_data(q, &cm_data.token_positions),));
    });
}

#[query(manual_reply = true)]
pub fn download_cycles_positions_purchases_rchunks(q: DownloadRChunkQuest) {
    with(&CM_DATA, |cm_data| {
        reply((rchunk_data(q, &cm_data.cycles_positions_purchases),));
    });
}

#[query(manual_reply = true)]
pub fn download_token_positions_purchases_rchunks(q: DownloadRChunkQuest) {
    with(&CM_DATA, |cm_data| {
        reply((rchunk_data(q, &cm_data.token_positions_purchases),));
    });
}



// ----------------------



#[derive(CandidType, Deserialize)]
pub struct SeeCyclesPositionsQuest {
    chunk_i: u128
}

#[query(manual_reply = true)]
pub fn see_cycles_positions(q: SeeCyclesPositionsQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[CyclesPosition]>,)>((
            cm_data.cycles_positions.chunks(SEE_CYCLES_POSITIONS_CHUNK_SIZE).nth(q.chunk_i as usize)
        ,));
    });
}

#[derive(CandidType, Deserialize)]
pub struct SeeTokenPositionsQuest {
    chunk_i: u128
}

#[query(manual_reply = true)]
pub fn see_token_positions(q: SeeTokenPositionsQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[TokenPosition]>,)>((
            cm_data.token_positions.chunks(SEE_TOKEN_POSITIONS_CHUNK_SIZE).nth(q.chunk_i as usize)
        ,));
    });
}

#[derive(CandidType, Deserialize)]
pub struct SeeCyclesPositionsPurchasesQuest {
    chunk_i: u128
}

#[query(manual_reply = true)]
pub fn see_cycles_positions_purchases(q: SeeCyclesPositionsPurchasesQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[CyclesPositionPurchase]>,)>((
            cm_data.cycles_positions_purchases.chunks(SEE_CYCLES_POSITIONS_PURCHASES_CHUNK_SIZE).nth(q.chunk_i as usize)
        ,));
    });
}

#[derive(CandidType, Deserialize)]
pub struct SeeTokenPositionsPurchasesQuest {
    chunk_i: u128
}

#[query(manual_reply = true)]
pub fn see_token_positions_purchases(q: SeeTokenPositionsPurchasesQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[TokenPositionPurchase]>,)>((
            cm_data.token_positions_purchases.chunks(SEE_TOKEN_POSITIONS_PURCHASES_CHUNK_SIZE).nth(q.chunk_i as usize)
        ,));
    });
}


// ---------------------------------


#[update(manual_reply = true)]
pub async fn trigger_payouts() {
    reply::<()>(());
    do_payouts().await;
    return;
}


// -------------------------------------------------------------


#[update(manual_reply = true)]
pub async fn cm_message_cycles_position_purchase_purchaser_cmcaller_callback(q: CMCallbackQuest) -> () {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(cycles_position_purchase_cycles_positions_purchases_i) = cm_data.cycles_positions_purchases.binary_search_by_key(&q.cm_call_id, |cycles_position_purchase| { cycles_position_purchase.id }) {
            cm_data.cycles_positions_purchases[cycles_position_purchase_cycles_positions_purchases_i]
            .cycles_payout_data
            .cmcaller_cycles_payout_callback_complete = Some((cycles_transfer_refund, q.opt_call_error));
        }
    });

    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_cycles_position_purchase_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(cycles_position_purchase_cycles_positions_purchases_i) = cm_data.cycles_positions_purchases.binary_search_by_key(&q.cm_call_id, |cycles_position_purchase| { cycles_position_purchase.id }) {
            cm_data.cycles_positions_purchases[cycles_position_purchase_cycles_positions_purchases_i]
            .token_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_token_position_purchase_purchaser_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
        
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(token_position_purchase_token_positions_purchases_i) = cm_data.token_positions_purchases.binary_search_by_key(&q.cm_call_id, |token_position_purchase| { token_position_purchase.id }) {
            cm_data.token_positions_purchases[token_position_purchase_token_positions_purchases_i]
            .token_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_token_position_purchase_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(token_position_purchase_token_positions_purchases_i) = cm_data.token_positions_purchases.binary_search_by_key(&q.cm_call_id, |token_position_purchase| { token_position_purchase.id }) {
            cm_data.token_positions_purchases[token_position_purchase_token_positions_purchases_i]
            .cycles_payout_data
            .cmcaller_cycles_payout_callback_complete = Some((cycles_transfer_refund, q.opt_call_error));
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_void_cycles_position_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
    
    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(void_cycles_position_void_cycles_positions_i) = cm_data.void_cycles_positions.binary_search_by_key(&q.cm_call_id, |void_cycles_position| { void_cycles_position.position_id }) {
            cm_data.void_cycles_positions[void_cycles_position_void_cycles_positions_i]
            .cycles_payout_data
            .cmcaller_cycles_payout_callback_complete = Some((cycles_transfer_refund, q.opt_call_error));
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_void_token_position_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    with_mut(&CM_DATA, |cm_data| {
        if let Ok(void_token_position_void_token_positions_i) = cm_data.void_token_positions.binary_search_by_key(&q.cm_call_id, |void_token_position| { void_token_position.position_id }) {
            cm_data.void_token_positions[void_token_position_void_token_positions_i]
            .token_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}





// -------------------------------------------------------------



#[update]
pub fn cts_set_stop_calls_flag(stop_calls_flag: bool) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::set(&STOP_CALLS, stop_calls_flag);
}

#[query]
pub fn cts_see_stop_calls_flag() -> bool {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    localkey::cell::get(&STOP_CALLS)
}





#[update]
pub fn cts_create_state_snapshot() -> u64/*len of the state_snapshot*/ {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    create_state_snapshot();
    
    with(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.len() as u64
    })
}





#[export_name = "canister_query cts_download_state_snapshot"]
pub fn cts_download_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT, |state_snapshot| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot.chunks(chunk_size).nth(chunk_i as usize),));
    });
}



#[update]
pub fn cts_clear_state_snapshot() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = Vec::new();
    });    
}

#[update]
pub fn cts_append_state_snapshot(mut append_bytes: Vec<u8>) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.append(&mut append_bytes);
    });
}

#[update]
pub fn cts_load_state_snapshot_data() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    load_state_snapshot_data();
}



// -------------------------------------------------------------



#[query(manual_reply = true)]
pub fn cts_see_payouts_errors(chunk_i: u32) {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[(u32, String)]>,)>((cm_data.do_payouts_errors.chunks(100).nth(chunk_i as usize),));
    });    
}



#[update]
pub fn cts_clear_payouts_errors() {
    if caller() != cts_id() {
        trap("Caller must be the CTS for this method.");
    }
    
    with_mut(&CM_DATA, |cm_data| {
        cm_data.do_payouts_errors = Vec::new();
    });    
}



