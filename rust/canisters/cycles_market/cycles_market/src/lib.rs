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
        cycles_to_icptokens,
        icptokens_to_cycles,
        principal_icp_subaccount,
        round_robin,
        time_nanos,
        time_nanos_u64,
        time_seconds,
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
        XdrPerMyriadPerIcp,
        cycles_transferrer,
        management_canister,
        cycles_market::*,
        cm_caller::*
    },
    ic_ledger_types::{
        IcpTransferError,
        IcpTransferArgs,
        icp_transfer,
        IcpTransferResult,
        IcpTokens,
        icp_account_balance,
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs,
        ICP_LEDGER_TRANSFER_DEFAULT_FEE,
        IcpId,
        IcpIdSub,
        IcpTimestamp,
        IcpMemo,
        IcpBlockHeight,
    },
    ic_cdk::{
        self,
        api::{
            id as cycles_market_canister_id,
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


// on a cycles-payout, the cycles-market will try once to send the cycles with a cycles_transfer-method call and if it fails, the cycles-market will use the deposit_cycles management canister method and close the position.

// make sure the positors and purchaser are secret and hidden. public-data is the position-id, the commodity, the minimum purchase, and the rate, (and the timestamp? no that makes it traceable)

type VoidCyclesPositionId = PositionId;
type VoidIcpPositionId = PositionId;
type CyclesPositionPurchaseId = PurchaseId;
type IcpPositionPurchaseId = PurchaseId;


#[derive(CandidType, Deserialize)]
struct CyclesPosition {
    id: PositionId,   
    positor: Principal,
    cycles: Cycles,
    minimum_purchase: Cycles,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    timestamp_nanos: u128,
}

#[derive(CandidType, Deserialize)]
struct IcpPosition {
    id: PositionId,   
    positor: Principal,
    icp: IcpTokens,
    minimum_purchase: IcpTokens,
    xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
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
struct IcpTransferBlockHeightAndTimestampNanos {
    block_height: Option<IcpBlockHeight>, // if None that means there was no transfer in this icp-payout. it is an unlock of the funds within the same-icp-id.
    timestamp_nanos: u128,
}
#[derive(Clone, CandidType, Deserialize)]
struct IcpPayoutData {
    icp_transfer: Option<IcpTransferBlockHeightAndTimestampNanos>,
    cm_message_call_success_timestamp_nanos: Option<u128>,
    cm_message_callback_complete: Option<Option<(u32, String)>>, // first option for the callback-completion, second option for the possible-positor-message-call-error
}
impl IcpPayoutData {
    // no new fn because a void_icp_position.icp_payout_data must start with the icp_transfer = Some(IcpTransferBlockHeightAndTimestampNanos)
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
    fn handle_do_icp_payout_sponse(&mut self, do_icp_payout_sponse: DoIcpPayoutSponse) {
        match do_icp_payout_sponse {
            DoIcpPayoutSponse::IcpTransferError(IcpTransferErrorType) => {
                
            },
            DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageError(icp_transfer, _cm_message_error_type) => {
                self.icp_transfer = Some(icp_transfer);
            },
            DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageSuccess(icp_transfer, cm_message_call_success_timestamp_nanos) => {
                self.icp_transfer = Some(icp_transfer);
                self.cm_message_call_success_timestamp_nanos = Some(cm_message_call_success_timestamp_nanos);
            },
            DoIcpPayoutSponse::NothingForTheDo => {},
        }
    }
    
}

trait IcpPayoutDataTrait {
    fn icp_payout_data(&self) -> IcpPayoutData;
    fn icp_payout_payee(&self) -> Principal;
    fn icp_payout_payor(&self) -> Principal;
    fn icp_payout_payee_method(&self) -> &'static str;
    fn icp_payout_payee_method_quest_bytes(&self, icp_payout_data_icp_transfer: IcpTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError>; 
    fn icp(&self) -> IcpTokens;
    fn icp_transfer_memo(&self) -> IcpMemo;
    fn cm_call_id(&self) -> u128;
    fn cm_call_callback_method(&self) -> &'static str; 
}



#[derive(Clone, CandidType, Deserialize)]
struct CyclesPositionPurchase {
    cycles_position_id: PositionId,
    cycles_position_positor: Principal,
    cycles_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    id: PurchaseId,
    purchaser: Principal,
    cycles: Cycles,
    timestamp_nanos: u128,
    cycles_payout_lock: bool,
    icp_payout_lock: bool,
    cycles_payout_data: CyclesPayoutData,
    icp_payout_data: IcpPayoutData
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
                cycles_position_xdr_permyriad_per_icp_rate: self.cycles_position_xdr_permyriad_per_icp_rate,
                purchase_id: self.id,
                purchase_timestamp_nanos: self.timestamp_nanos,
                icp_payment: cycles_to_icptokens(self.cycles, self.cycles_position_xdr_permyriad_per_icp_rate),
            }
        ) 
    }
    fn cycles(&self) -> Cycles { self.cycles }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_PURCHASER }
}
impl IcpPayoutDataTrait for CyclesPositionPurchase {
    fn icp_payout_data(&self) -> IcpPayoutData { self.icp_payout_data.clone() }
    fn icp_payout_payee(&self) -> Principal { self.cycles_position_positor }
    fn icp_payout_payor(&self) -> Principal { self.purchaser }
    fn icp_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR }
    fn icp_payout_payee_method_quest_bytes(&self, icp_payout_data_icp_transfer: IcpTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMCyclesPositionPurchasePositorMessageQuest {
                cycles_position_id: self.cycles_position_id,
                purchase_id: self.id,
                purchaser: self.purchaser,
                purchase_timestamp_nanos: self.timestamp_nanos,
                cycles_purchase: self.cycles,
                cycles_position_xdr_permyriad_per_icp_rate: self.cycles_position_xdr_permyriad_per_icp_rate,
                icp_payment: self.icp(),
                icp_transfer_block_height: icp_payout_data_icp_transfer.block_height.unwrap(), 
                icp_transfer_timestamp_nanos: icp_payout_data_icp_transfer.timestamp_nanos,
            }    
        )
    } 
    fn icp(&self) -> IcpTokens { cycles_to_icptokens(self.cycles, self.cycles_position_xdr_permyriad_per_icp_rate) }
    fn icp_transfer_memo(&self) -> IcpMemo { CYCLES_POSITION_PURCHASE_ICP_TRANSFER_MEMO }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_POSITOR } 
}




#[derive(Clone, CandidType, Deserialize)]
struct IcpPositionPurchase {
    icp_position_id: PositionId,
    icp_position_positor: Principal,
    icp_position_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp,
    id: PurchaseId,
    purchaser: Principal,
    icp: IcpTokens,
    timestamp_nanos: u128,
    cycles_payout_lock: bool,
    icp_payout_lock: bool,
    cycles_payout_data: CyclesPayoutData,
    icp_payout_data: IcpPayoutData // even though the purchaser knows bout the purchase, it is better to send the purchaser a message when the icp_transfer is complete with the block height 
}
impl CyclesPayoutDataTrait for IcpPositionPurchase {
    fn cycles_payout_data(&self) -> CyclesPayoutData {
        self.cycles_payout_data.clone()
    }
    fn cycles_payout_payee(&self) -> Principal {
        self.icp_position_positor
    }
    fn cycles_payout_payee_method(&self) -> &'static str {
        CM_MESSAGE_METHOD_ICP_POSITION_PURCHASE_POSITOR
    }
    fn cycles_payout_payee_method_quest_bytes(&self) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMIcpPositionPurchasePositorMessageQuest{
                icp_position_id: self.icp_position_id,
                icp_position_xdr_permyriad_per_icp_rate: self.icp_position_xdr_permyriad_per_icp_rate,
                purchase_id: self.id,
                purchaser: self.purchaser,
                icp_purchase: self.icp,
                purchase_timestamp_nanos: self.timestamp_nanos,
            }
        )
    }
    fn cycles(&self) -> Cycles {
        icptokens_to_cycles(self.icp, self.icp_position_xdr_permyriad_per_icp_rate)
    }
    fn cm_call_id(&self) -> u128 {
        self.id
    }
    fn cm_call_callback_method(&self) -> &'static str {
        CMCALLER_CALLBACK_ICP_POSITION_PURCHASE_POSITOR
    }
}
impl IcpPayoutDataTrait for IcpPositionPurchase {
    fn icp_payout_data(&self) -> IcpPayoutData { self.icp_payout_data.clone() }
    fn icp_payout_payee(&self) -> Principal { self.purchaser }
    fn icp_payout_payor(&self) -> Principal { self.icp_position_positor }
    fn icp_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_ICP_POSITION_PURCHASE_PURCHASER }
    fn icp_payout_payee_method_quest_bytes(&self, icp_payout_data_icp_transfer: IcpTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMIcpPositionPurchasePurchaserMessageQuest {
                icp_position_id: self.icp_position_id,
                purchase_id: self.id, 
                positor: self.icp_position_positor,
                purchase_timestamp_nanos: self.timestamp_nanos,
                icp_purchase: self.icp(),
                icp_position_xdr_permyriad_per_icp_rate: self.icp_position_xdr_permyriad_per_icp_rate,
                cycles_payment: icptokens_to_cycles(self.icp, self.icp_position_xdr_permyriad_per_icp_rate),
                icp_transfer_block_height: icp_payout_data_icp_transfer.block_height.unwrap(),
                icp_transfer_timestamp_nanos: icp_payout_data_icp_transfer.timestamp_nanos,
            }
        )
    }
    fn icp(&self) -> IcpTokens { self.icp }
    fn icp_transfer_memo(&self) -> IcpMemo { ICP_POSITION_PURCHASE_ICP_TRANSFER_MEMO }
    fn cm_call_id(&self) -> u128 { self.id }
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_ICP_POSITION_PURCHASE_PURCHASER }

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
struct VoidIcpPosition {
    position_id: PositionId,
    icp: IcpTokens,
    positor: Principal,
    icp_payout_lock: bool,  // lock for the payout
    icp_payout_data: IcpPayoutData,
    timestamp_nanos: u128
}
impl IcpPayoutDataTrait for VoidIcpPosition {
    fn icp_payout_data(&self) -> IcpPayoutData { self.icp_payout_data.clone() }
    fn icp_payout_payee(&self) -> Principal { self.positor }
    fn icp_payout_payor(&self) -> Principal { self.positor }
    fn icp_payout_payee_method(&self) -> &'static str { CM_MESSAGE_METHOD_VOID_ICP_POSITION_POSITOR }
    fn icp_payout_payee_method_quest_bytes(&self, _icp_payout_data_icp_transfer: IcpTransferBlockHeightAndTimestampNanos) -> Result<Vec<u8>, CandidError> {
        encode_one(
            CMVoidIcpPositionPositorMessageQuest {
                position_id: self.position_id,
                void_icp: self.icp(),
                timestamp_nanos: self.timestamp_nanos
            }
        )
    }
    fn icp(&self) -> IcpTokens { self.icp }
    fn icp_transfer_memo(&self) -> IcpMemo { IcpMemo(0) }
    fn cm_call_id(&self) -> u128 { self.position_id }  
    fn cm_call_callback_method(&self) -> &'static str { CMCALLER_CALLBACK_VOID_ICP_POSITION_POSITOR } 
}



#[derive(CandidType, Deserialize)]
struct CMData {
    cts_id: Principal,
    cm_caller: Principal,
    id_counter: u128,
    mid_call_user_icp_balance_locks: HashSet<Principal>,
    cycles_positions: Vec<CyclesPosition>,
    icp_positions: Vec<IcpPosition>,
    cycles_positions_purchases: Vec<CyclesPositionPurchase>,
    icp_positions_purchases: Vec<IcpPositionPurchase>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    void_icp_positions: Vec<VoidIcpPosition>,
    do_payouts_errors: Vec<(u32, String)>,
}

impl CMData {
    fn new() -> Self {
        Self {
            cts_id: Principal::from_slice(&[]),
            cm_caller: Principal::from_slice(&[]),
            id_counter: 0,
            mid_call_user_icp_balance_locks: HashSet::new(),
            cycles_positions: Vec::new(),
            icp_positions: Vec::new(),
            cycles_positions_purchases: Vec::new(),
            icp_positions_purchases: Vec::new(),
            void_cycles_positions: Vec::new(),
            void_icp_positions: Vec::new(),
            do_payouts_errors: Vec::new()
        }
    }
}



#[derive(CandidType, Deserialize)]
struct OldCMData {
    cts_id: Principal,
    cm_caller: Principal,
    id_counter: u128,
    mid_call_user_icp_balance_locks: HashSet<Principal>,
    cycles_positions: Vec<CyclesPosition>,
    icp_positions: Vec<IcpPosition>,
    cycles_positions_purchases: Vec<CyclesPositionPurchase>,
    icp_positions_purchases: Vec<IcpPositionPurchase>,
    void_cycles_positions: Vec<VoidCyclesPosition>,
    void_icp_positions: Vec<VoidIcpPosition>,
    do_payouts_errors: Vec<(u32, String)>,
}




pub const CREATE_POSITION_FEE: Cycles = 50_000_000_000;
pub const PURCHASE_POSITION_FEE: Cycles = 50_000_000_000;

pub const TRANSFER_ICP_BALANCE_FEE: Cycles = 50_000_000_000;

pub const CYCLES_TRANSFERRER_TRANSFER_CYCLES_FEE: Cycles = 20_000_000_000;

pub const MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK: u128 = NANOS_IN_A_SECOND * SECONDS_IN_AN_HOUR * 72;
pub const VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS: u128 = SECONDS_IN_AN_HOUR * 1;


pub const MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP: Cycles = 20_000_000_000_000;
pub const MINIMUM_CYCLES_POSITION: Cycles = 1_000_000_000_000;

pub const MINIMUM_ICP_POSITION_FOR_AN_ICP_POSITION_BUMP: IcpTokens = IcpTokens::from_e8s(200000000);
pub const MINIMUM_ICP_POSITION: IcpTokens = IcpTokens::from_e8s(50000000);



const CANISTER_NETWORK_MEMORY_ALLOCATION_MiB: usize = 500; // multiple of 10
const CANISTER_DATA_STORAGE_SIZE_MiB: usize = CANISTER_NETWORK_MEMORY_ALLOCATION_MiB / 2 - 20/*memory-size at the start [re]placement*/; 

const CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_CYCLES_POSITIONS: usize = CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<CyclesPosition>();

const ICP_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_ICP_POSITIONS: usize = ICP_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<IcpPosition>();

const ICP_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_ICP_POSITIONS_PURCHASES: usize = ICP_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<IcpPositionPurchase>();

const CYCLES_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_CYCLES_POSITIONS_PURCHASES: usize = CYCLES_POSITIONS_PURCHASES_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<CyclesPositionPurchase>();

const VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_VOID_CYCLES_POSITIONS: usize = VOID_CYCLES_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidCyclesPosition>();

const VOID_ICP_POSITIONS_MAX_STORAGE_SIZE_MiB: usize = CANISTER_DATA_STORAGE_SIZE_MiB / 6 * 1;
const MAX_VOID_ICP_POSITIONS: usize = VOID_ICP_POSITIONS_MAX_STORAGE_SIZE_MiB * MiB / std::mem::size_of::<VoidIcpPosition>();


const DO_VOID_CYCLES_POSITIONS_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_VOID_ICP_POSITIONS_ICP_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_CYCLES_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_CYCLES_POSITIONS_PURCHASES_ICP_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_ICP_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE: usize = 10;
const DO_ICP_POSITIONS_PURCHASES_ICP_PAYOUTS_CHUNK_SIZE: usize = 10;



const CM_MESSAGE_METHOD_VOID_CYCLES_POSITION_POSITOR: &'static str       = "cm_message_void_cycles_position_positor";
const CM_MESSAGE_METHOD_VOID_ICP_POSITION_POSITOR: &'static str          = "cm_message_void_icp_position_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_POSITOR: &'static str   = "cm_message_cycles_position_purchase_positor";
const CM_MESSAGE_METHOD_CYCLES_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_cycles_position_purchase_purchaser";
const CM_MESSAGE_METHOD_ICP_POSITION_PURCHASE_POSITOR: &'static str      = "cm_message_icp_position_purchase_positor";
const CM_MESSAGE_METHOD_ICP_POSITION_PURCHASE_PURCHASER: &'static str    = "cm_message_icp_position_purchase_purchaser";

const CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_cycles_position_purchase_purchaser_cmcaller_callback";
const CMCALLER_CALLBACK_CYCLES_POSITION_PURCHASE_POSITOR: &'static str = "cm_message_cycles_position_purchase_positor_cmcaller_callback";
const CMCALLER_CALLBACK_ICP_POSITION_PURCHASE_PURCHASER: &'static str = "cm_message_icp_position_purchase_purchaser_cmcaller_callback";
const CMCALLER_CALLBACK_ICP_POSITION_PURCHASE_POSITOR: &'static str = "cm_message_icp_position_purchase_positor_cmcaller_callback";
const CMCALLER_CALLBACK_VOID_CYCLES_POSITION_POSITOR: &'static str = "cm_message_void_cycles_position_positor_cmcaller_callback";
const CMCALLER_CALLBACK_VOID_ICP_POSITION_POSITOR: &'static str = "cm_message_void_icp_position_positor_cmcaller_callback";

const ICP_POSITION_PURCHASE_ICP_TRANSFER_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CM-IPP-0"));
const CYCLES_POSITION_PURCHASE_ICP_TRANSFER_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CM-CPP-0"));

const TRANSFER_ICP_BALANCE_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CMTRNSFR"));

const SEE_CYCLES_POSITIONS_CHUNK_SIZE: usize = 300;
const SEE_ICP_POSITIONS_CHUNK_SIZE: usize = 300;
const SEE_CYCLES_POSITIONS_PURCHASES_CHUNK_SIZE: usize = 300;
const SEE_ICP_POSITIONS_PURCHASES_CHUNK_SIZE: usize = 300;



const MAX_MID_CALL_USER_ICP_BALANCE_LOCKS: usize = 500;


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;





thread_local! {
    static CM_DATA: RefCell<CMData> = RefCell::new(CMData::new()); 
    
    // not save through the upgrades
    static STOP_CALLS: Cell<bool> = Cell::new(false);
    static STATE_SNAPSHOT: RefCell<Vec<u8>> = RefCell::new(Vec::new());    
}


// -------------------------------------------------------------


#[derive(CandidType, Deserialize)]
struct CMInit {
    cts_id: Principal,
    cm_caller: Principal,
} 

#[init]
fn init(cm_init: CMInit) {
    with_mut(&CM_DATA, |cm_data| { 
        cm_data.cts_id = cm_init.cts_id; 
        cm_data.cm_caller = cm_init.cm_caller;
    });
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
                    mid_call_user_icp_balance_locks: old_cm_data.mid_call_user_icp_balance_locks,
                    cycles_positions: old_cm_data.cycles_positions,
                    icp_positions: old_cm_data.icp_positions,
                    cycles_positions_purchases: old_cm_data.cycles_positions_purchases,
                    icp_positions_purchases: old_cm_data.icp_positions_purchases,
                    void_cycles_positions: old_cm_data.void_cycles_positions,
                    void_icp_positions: old_cm_data.void_icp_positions,
                    do_payouts_errors: old_cm_data.do_payouts_errors
                };
                cm_data
                */
            }
        }
    });

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
        "create_icp_position",
        "purchase_cycles_position",
        "purchase_icp_position",
        "void_position",
        "see_icp_lock",
        "transfer_icp_balance",
        "cm_message_cycles_position_purchase_purchaser_cmcaller_callback",
        "cm_message_cycles_position_purchase_positor_cmcaller_callback",
        "cm_message_icp_position_purchase_purchaser_cmcaller_callback",
        "cm_message_icp_position_purchase_positor_cmcaller_callback",
        "cm_message_void_cycles_position_positor_cmcaller_callback",
        "cm_message_void_icp_position_positor_cmcaller_callback",
    ].contains(&&method_name()[..]) {
        trap("this method must be call by a canister.");
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


async fn check_user_cycles_market_icp_ledger_balance(user_id: &Principal) -> CallResult<IcpTokens> {
    icp_account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs { account: user_icp_id(&cycles_market_canister_id(), user_id) }    
    ).await
}


fn check_user_icp_balance_in_the_lock(cm_data: &CMData, user_id: &Principal) -> IcpTokens {
    cm_data.icp_positions.iter()
        .filter(|icp_position: &&IcpPosition| { icp_position.positor == *user_id })
        .fold(IcpTokens::from_e8s(0), |cummulator: IcpTokens, user_icp_position: &IcpPosition| {
            cummulator + user_icp_position.icp + ( IcpTokens::from_e8s(user_icp_position.icp.e8s() / user_icp_position.minimum_purchase.e8s() * ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s()) ) 
        })
    +
    cm_data.cycles_positions_purchases.iter()
        .filter(|cycles_position_purchase: &&CyclesPositionPurchase| {
            cycles_position_purchase.purchaser == *user_id && cycles_position_purchase.icp_payout_data.icp_transfer.is_none() 
        })
        .fold(IcpTokens::from_e8s(0), |cummulator: IcpTokens, user_cycles_position_purchase_with_unpaid_icp: &CyclesPositionPurchase| {
            cummulator + cycles_to_icptokens(user_cycles_position_purchase_with_unpaid_icp.cycles, user_cycles_position_purchase_with_unpaid_icp.cycles_position_xdr_permyriad_per_icp_rate) + ICP_LEDGER_TRANSFER_DEFAULT_FEE
        })
    +
    cm_data.icp_positions_purchases.iter()
        .filter(|icp_position_purchase: &&IcpPositionPurchase| {
            icp_position_purchase.icp_position_positor == *user_id && icp_position_purchase.icp_payout_data.icp_transfer.is_none() 
        })
        .fold(IcpTokens::from_e8s(0), |cummulator: IcpTokens, icp_position_purchase_with_the_user_as_the_positor_with_unpaid_icp: &IcpPositionPurchase| {
            cummulator + icp_position_purchase_with_the_user_as_the_positor_with_unpaid_icp.icp + ICP_LEDGER_TRANSFER_DEFAULT_FEE
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
            q.cycles()
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



enum IcpTransferErrorType {
    IcpTransferError(IcpTransferError),
    IcpTransferCallError((u32, String))
}

enum CMMessageErrorType {
    CMCallQuestCandidEncodeError(CandidError),
    CMCallQuestPutBytesCandidEncodeError(CandidError),
    CMCallerCallError(CMCallError),
    CMCallerCallSponseCandidDecodeError(CandidError),
    CMCallerCallCallError((u32, String))
}

enum DoIcpPayoutSponse {
    IcpTransferError(IcpTransferErrorType),
    IcpTransferSuccessAndCMMessageError(IcpTransferBlockHeightAndTimestampNanos, CMMessageErrorType),
    IcpTransferSuccessAndCMMessageSuccess(IcpTransferBlockHeightAndTimestampNanos, u128),
    //CMMessageError(CMMessageErrorType),
    //CMMessageSuccess(u128),
    NothingForTheDo,
}

async fn do_icp_payout<T: IcpPayoutDataTrait>(q: T) -> DoIcpPayoutSponse {
    
    let icp_payout_data_icp_transfer: IcpTransferBlockHeightAndTimestampNanos = match q.icp_payout_data().icp_transfer {
        Some(icp_transfer_data) => icp_transfer_data,
        None => {
            let icp_transfer_created_at_time: u64 = time_nanos_u64()-NANOS_IN_A_SECOND as u64;
            match icp_transfer(
                MAINNET_LEDGER_CANISTER_ID,
                IcpTransferArgs{
                    memo: q.icp_transfer_memo(),
                    amount: q.icp(),
                    fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                    from_subaccount: Some(principal_icp_subaccount(&q.icp_payout_payor())),
                    to: IcpId::new(&cycles_market_canister_id(), &principal_icp_subaccount(&q.icp_payout_payee())),
                    created_at_time: Some(IcpTimestamp { timestamp_nanos: icp_transfer_created_at_time })
                }
            ).await {
                Ok(icp_transfer_result) => match icp_transfer_result {
                    Ok(block_height) => {
                        IcpTransferBlockHeightAndTimestampNanos{
                            block_height: Some(block_height),
                            timestamp_nanos: icp_transfer_created_at_time as u128
                        }
                    },
                    Err(icp_transfer_error) => {
                        return DoIcpPayoutSponse::IcpTransferError(IcpTransferErrorType::IcpTransferError(icp_transfer_error));
                    }
                },
                Err(icp_transfer_call_error) => {
                    return DoIcpPayoutSponse::IcpTransferError(IcpTransferErrorType::IcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1)));
                }
            }
        }
    };
    
    match q.icp_payout_data().cm_message_call_success_timestamp_nanos {
        Some(cm_message_call_success_timestamp_nanos) => return DoIcpPayoutSponse::NothingForTheDo,
        None => {
            let call_future = call_raw128(
                with(&CM_DATA, |cm_data| { cm_data.cm_caller }),
                "cm_call",
                &match encode_one(
                    CMCallQuest{
                        cm_call_id: q.cm_call_id(),
                        for_the_canister: q.icp_payout_payee(),
                        method: q.icp_payout_payee_method().to_string(),
                        put_bytes: match q.icp_payout_payee_method_quest_bytes(icp_payout_data_icp_transfer.clone()) {
                            Ok(b) => b,
                            Err(candid_error) => {
                                return DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageError(icp_payout_data_icp_transfer, CMMessageErrorType::CMCallQuestPutBytesCandidEncodeError(candid_error));     
                            }
                        },
                        cycles: 0,
                        cm_callback_method: q.cm_call_callback_method().to_string(),
                    }
                ) {
                    Ok(b) => b,
                    Err(candid_error) => {
                        return DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageError(icp_payout_data_icp_transfer, CMMessageErrorType::CMCallQuestCandidEncodeError(candid_error));
                    }
                },
                0
            );
            match call_future.await {
                Ok(b) => match decode_one::<CMCallResult>(&b) {
                    Ok(cm_call_sponse) => match cm_call_sponse {
                        Ok(()) => {
                            return DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageSuccess(icp_payout_data_icp_transfer, time_nanos());
                        },
                        Err(cm_call_error) => {
                            return DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageError(icp_payout_data_icp_transfer, CMMessageErrorType::CMCallerCallError(cm_call_error));
                        }
                    },
                    Err(candid_error) => {
                        return DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageError(icp_payout_data_icp_transfer, CMMessageErrorType::CMCallerCallSponseCandidDecodeError(candid_error));                    
                    }
                },
                Err(call_error) => {
                    return DoIcpPayoutSponse::IcpTransferSuccessAndCMMessageError(icp_payout_data_icp_transfer, CMMessageErrorType::CMCallerCallCallError((call_error.0 as u32, call_error.1)));                    
                } 
            }
        }
    }
    
}



async fn _do_payouts() {

    let mut void_cycles_positions_cycles_payouts_chunk: Vec<(VoidCyclesPositionId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new();
    let mut void_icp_positions_icp_payouts_chunk: Vec<(VoidIcpPositionId, _/*anonymous-future of the do_icp_payout-async-function*/)> = Vec::new();
    let mut cycles_positions_purchases_cycles_payouts_chunk: Vec<(CyclesPositionPurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut cycles_positions_purchases_icp_payouts_chunk: Vec<(CyclesPositionPurchaseId, _/*anonymous-future of the icp_transfer-function*/)> = Vec::new();
    let mut icp_positions_purchases_cycles_payouts_chunk: Vec<(IcpPositionPurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut icp_positions_purchases_icp_payouts_chunk: Vec<(IcpPositionPurchaseId, _/*anonymous-future of the icp_transfer-function*/)> = Vec::new();

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
        while i < cm_data.void_icp_positions.len() && void_icp_positions_icp_payouts_chunk.len() < DO_VOID_ICP_POSITIONS_ICP_PAYOUTS_CHUNK_SIZE {
            let vip: &mut VoidIcpPosition = &mut cm_data.void_icp_positions[i];
            if vip.icp_payout_data.is_waiting_for_the_cmcaller_callback() {
                if time_nanos().saturating_sub(vip.icp_payout_data.cm_message_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK {
                    std::mem::drop(vip);
                    cm_data.void_icp_positions.remove(i);
                    continue;
                }
                // skip
            } else if vip.icp_payout_lock == true { 
                // skip
            } else {
                vip.icp_payout_lock = true;
                void_icp_positions_icp_payouts_chunk.push(
                    (
                        vip.position_id,
                        do_icp_payout(vip.clone())
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
            if cpp.icp_payout_data.is_complete() == false
            && cpp.icp_payout_data.is_waiting_for_the_cmcaller_callback() == false
            && cpp.icp_payout_lock == false
            && cycles_positions_purchases_icp_payouts_chunk.len() < DO_CYCLES_POSITIONS_PURCHASES_ICP_PAYOUTS_CHUNK_SIZE {
                cpp.icp_payout_lock = true;
                cycles_positions_purchases_icp_payouts_chunk.push(
                    (     
                        cpp.id,
                        do_icp_payout(cpp.clone())                        
                    )
                );
            }
            i += 1;
        }
        
        let mut i: usize = 0;
        while i < cm_data.icp_positions_purchases.len() {
            let ipp: &mut IcpPositionPurchase = &mut cm_data.icp_positions_purchases[i];                    
            if ipp.cycles_payout_data.is_complete() == false 
            && ipp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() == false
            && ipp.cycles_payout_lock == false
            && icp_positions_purchases_cycles_payouts_chunk.len() < DO_ICP_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE {
                ipp.cycles_payout_lock = true;
                icp_positions_purchases_cycles_payouts_chunk.push(
                    (
                        ipp.id,
                        do_cycles_payout(ipp.clone())
                    )
                );
            }
            if ipp.icp_payout_data.is_complete() == false
            && ipp.icp_payout_data.is_waiting_for_the_cmcaller_callback() == false
            && ipp.icp_payout_lock == false                                                        
            && icp_positions_purchases_icp_payouts_chunk.len() < DO_ICP_POSITIONS_PURCHASES_ICP_PAYOUTS_CHUNK_SIZE {
                ipp.icp_payout_lock = true;
                icp_positions_purchases_icp_payouts_chunk.push(
                    (     
                        ipp.id,
                        do_icp_payout(ipp.clone())
                    )
                );
            }
            i += 1;
        }
        
    });

    let (vcps_ids, vcps_do_cycles_payouts_futures): (Vec<VoidCyclesPositionId>, Vec<_/*do_cycles_payout-future*/>) = void_cycles_positions_cycles_payouts_chunk.into_iter().unzip();
    let (vips_ids, vips_do_icp_payouts_futures): (Vec<VoidIcpPositionId>, Vec<_/*do_icp_payout-future*/>) = void_icp_positions_icp_payouts_chunk.into_iter().unzip();
    let (cpps_cycles_payouts_ids, cpps_do_cycles_payouts_futures): (Vec<CyclesPositionPurchaseId>, Vec<_/*do_cycles_payout-future*/>) = cycles_positions_purchases_cycles_payouts_chunk.into_iter().unzip();
    let (cpps_icp_payouts_ids, cpps_do_icp_payouts_futures): (Vec<CyclesPositionPurchaseId>, Vec<_/*do_icp_payout-future*/>) = cycles_positions_purchases_icp_payouts_chunk.into_iter().unzip();
    let (ipps_cycles_payouts_ids, ipps_do_cycles_payouts_futures): (Vec<IcpPositionPurchaseId>, Vec<_/*do_cycles_payout-future*/>) = icp_positions_purchases_cycles_payouts_chunk.into_iter().unzip();
    let (ipps_icp_payouts_ids, ipps_do_icp_payouts_futures): (Vec<IcpPositionPurchaseId>, Vec<_/*do_icp_payout-future*/>) = icp_positions_purchases_icp_payouts_chunk.into_iter().unzip();
    
    /*
    let cycles_payouts_futures = futures::future::join3(
        futures::future::join_all(vcps_do_cycles_payouts_futures),
        futures::future::join_all(cpps_do_cycles_payouts_futures),
        futures::future::join_all(ipps_do_cycles_payouts_futures),
    );
    
    let icp_payouts_futures = futures::future::join3(
        futures::future::join_all(vips_do_icp_payouts_futures),
        futures::future::join_all(cpps_do_icp_payouts_futures),
        futures::future::join_all(ipps_do_icp_payouts_futures),
    );
    
    let (
        cycles_payouts_rs, 
        icp_payouts_rs           
    ) = futures::future::join(
        cycles_payouts_futures,
        icp_payouts_futures,
    ).await;
    
    let (
        vcps_do_cycles_payouts_rs,
        cpps_do_cycles_payouts_rs,
        ipps_do_cycles_payouts_rs
    ): (
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
    ) = cycles_payouts_rs;
    
    let (
        vips_do_icp_payouts_rs,
        cpps_do_icp_payouts_rs,
        ipps_do_icp_payouts_rs
    ): (
        Vec<DoIcpPayoutSponse>,
        Vec<DoIcpPayoutSponse>,
        Vec<DoIcpPayoutSponse>
    ) = icp_payouts_rs;
    */
    let (
        vcps_do_cycles_payouts_rs,
        vips_do_icp_payouts_rs,
        cpps_do_cycles_payouts_rs,
        cpps_do_icp_payouts_rs,
        ipps_do_cycles_payouts_rs,
        ipps_do_icp_payouts_rs
    ): (
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoIcpPayoutSponse>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoIcpPayoutSponse>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoIcpPayoutSponse>,
    ) = futures::join!(
        futures::future::join_all(vcps_do_cycles_payouts_futures),
        futures::future::join_all(vips_do_icp_payouts_futures),
        futures::future::join_all(cpps_do_cycles_payouts_futures),
        futures::future::join_all(cpps_do_icp_payouts_futures),
        futures::future::join_all(ipps_do_cycles_payouts_futures),
        futures::future::join_all(ipps_do_icp_payouts_futures),
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
        for (vip_id, do_icp_payout_sponse) in vips_ids.into_iter().zip(vips_do_icp_payouts_rs.into_iter()) {      
            let vip_void_icp_positions_i: usize = {
                match cm_data.void_icp_positions.binary_search_by_key(&vip_id, |vip| { vip.position_id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }  
            };
            let vip: &mut VoidIcpPosition = &mut cm_data.void_icp_positions[vip_void_icp_positions_i];
            vip.icp_payout_lock = false;
            vip.icp_payout_data.handle_do_icp_payout_sponse(do_icp_payout_sponse);
            if vip.icp_payout_data.is_complete() {
                std::mem::drop(vip);
                cm_data.void_icp_positions.remove(vip_void_icp_positions_i);
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
        for (cpp_id, do_icp_payout_sponse) in cpps_icp_payouts_ids.into_iter().zip(cpps_do_icp_payouts_rs.into_iter()) {
            let cpp_cycles_positions_purchases_i: usize = {
                match cm_data.cycles_positions_purchases.binary_search_by_key(&cpp_id, |cpp| { cpp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let cpp: &mut CyclesPositionPurchase = &mut cm_data.cycles_positions_purchases[cpp_cycles_positions_purchases_i];
            cpp.icp_payout_lock = false;
            cpp.icp_payout_data.handle_do_icp_payout_sponse(do_icp_payout_sponse);
        }
        for (ipp_id, do_cycles_payout_result) in ipps_cycles_payouts_ids.into_iter().zip(ipps_do_cycles_payouts_rs.into_iter()) {
            let ipp_icp_positions_purchases_i: usize = {
                match cm_data.icp_positions_purchases.binary_search_by_key(&ipp_id, |ipp| { ipp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let ipp: &mut IcpPositionPurchase = &mut cm_data.icp_positions_purchases[ipp_icp_positions_purchases_i];
            ipp.cycles_payout_lock = false;
            ipp.cycles_payout_data.handle_do_cycles_payout_result(do_cycles_payout_result);
        }
        for (ipp_id, do_icp_payout_sponse) in ipps_icp_payouts_ids.into_iter().zip(ipps_do_icp_payouts_rs.into_iter()) {
            let ipp_icp_positions_purchases_i: usize = {
                match cm_data.icp_positions_purchases.binary_search_by_key(&ipp_id, |ipp| { ipp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let ipp: &mut IcpPositionPurchase = &mut cm_data.icp_positions_purchases[ipp_icp_positions_purchases_i];
            ipp.icp_payout_lock = false;
            ipp.icp_payout_data.handle_do_icp_payout_sponse(do_icp_payout_sponse);
        }
        
    });
    
}


async fn do_payouts() {
    
    if with(&CM_DATA, |cm_data| { 
        cm_data.void_cycles_positions.len() == 0
        && cm_data.cycles_positions_purchases.len() == 0
        && cm_data.icp_positions_purchases.len() == 0
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

    if q.cycles % q.xdr_permyriad_per_icp_rate as u128 != 0 {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMustBeAMultipleOfTheXdrPerMyriadPerIcpRate),));
        do_payouts().await;
        return;
    }

    if q.minimum_purchase % q.xdr_permyriad_per_icp_rate as u128 != 0 {
        reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::MinimumPurchaseMustBeAMultipleOfTheXdrPerMyriadPerIcpRate),));
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
            let cycles_position_highest_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp = { 
                cm_data.cycles_positions.iter()
                    .max_by_key(|cycles_position: &&CyclesPosition| { cycles_position.xdr_permyriad_per_icp_rate })
                    .unwrap()
                    .xdr_permyriad_per_icp_rate
            };
            if q.xdr_permyriad_per_icp_rate > cycles_position_highest_xdr_permyriad_per_icp_rate && q.cycles >= MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP {
                // bump
                let cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id: PositionId = {
                    cm_data.cycles_positions.iter()
                        .min_by_key(|cycles_position: &&CyclesPosition| { cycles_position.xdr_permyriad_per_icp_rate })
                        .unwrap()
                        .id
                };
                let cycles_position_lowest_xdr_permyriad_per_icp_rate_cycles_positions_i: usize = {
                    cm_data.cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id,
                        |cycles_position| { cycles_position.id }
                    ).unwrap()
                };
                let cycles_position_lowest_xdr_permyriad_per_icp_rate: CyclesPosition = cm_data.cycles_positions.remove(cycles_position_lowest_xdr_permyriad_per_icp_rate_cycles_positions_i);
                if cycles_position_lowest_xdr_permyriad_per_icp_rate.id != cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id { trap("outside the bounds of the contract.") }
                let cycles_position_lowest_xdr_permyriad_per_icp_rate_void_cycles_positions_insertion_i = { 
                    cm_data.void_cycles_positions.binary_search_by_key(
                        &cycles_position_lowest_xdr_permyriad_per_icp_rate_position_id,
                        |void_cycles_position| { void_cycles_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_cycles_positions.insert(
                    cycles_position_lowest_xdr_permyriad_per_icp_rate_void_cycles_positions_insertion_i,
                    VoidCyclesPosition{
                        position_id:    cycles_position_lowest_xdr_permyriad_per_icp_rate.id,
                        positor:        cycles_position_lowest_xdr_permyriad_per_icp_rate.positor,
                        cycles:         cycles_position_lowest_xdr_permyriad_per_icp_rate.cycles,
                        cycles_payout_lock: false,
                        cycles_payout_data: CyclesPayoutData::new(),
                        timestamp_nanos: time_nanos()
                    }
                );
                Ok(())
            } else {
                reply::<(Result<CreateCyclesPositionSuccess, CreateCyclesPositionError>,)>((Err(CreateCyclesPositionError::CyclesMarketIsFull_MinimumRateAndMinimumCyclesPositionForABump{ minimum_rate_for_a_bump: cycles_position_highest_xdr_permyriad_per_icp_rate + 1, minimum_cycles_position_for_a_bump: MINIMUM_CYCLES_POSITION_FOR_A_CYCLES_POSITION_BUMP }),));
                return Err(());
            }
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
                xdr_permyriad_per_icp_rate: q.xdr_permyriad_per_icp_rate,
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
pub async fn create_icp_position(q: CreateIcpPositionQuest) { //-> Result<CreateIcpPositionSuccess,CreateIcpPositionError> {

    let positor: Principal = caller();

    if q.minimum_purchase > q.icp {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MinimumPurchaseMustBeEqualOrLessThanTheIcpPosition),));    
        do_payouts().await;
        return;
    }

    if q.icp < MINIMUM_ICP_POSITION {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MinimumIcpPosition(MINIMUM_ICP_POSITION)),));
        do_payouts().await;
        return;
    }
    
    if q.minimum_purchase.e8s() == 0 {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MinimumPurchaseCannotBeZero),));
        do_payouts().await;
        return;
    }


    let msg_cycles_quirement: Cycles = CREATE_POSITION_FEE; 

    if msg_cycles_available128() < msg_cycles_quirement {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::MsgCyclesTooLow{ create_position_fee: CREATE_POSITION_FEE  }),));
        do_payouts().await;
        return;
    }

    if canister_balance128().checked_add(msg_cycles_quirement).is_none() {
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CyclesMarketIsFull),));
        do_payouts().await;
        return;
    }

    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_icp_balance_locks.len() >= MAX_MID_CALL_USER_ICP_BALANCE_LOCKS {
            reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CyclesMarketIsBusy),));
            return Err(());
        }
        if cm_data.mid_call_user_icp_balance_locks.contains(&positor) {
            reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall),));
            return Err(());
        }
        cm_data.mid_call_user_icp_balance_locks.insert(positor);
        Ok(())
    }) {
        Ok(()) => {},
        Err(()) => {
            do_payouts().await;
            return;
        }
    }
    
    // check icp balance and make sure to unlock the user on returns after here 
    let user_icp_ledger_balance: IcpTokens = match check_user_cycles_market_icp_ledger_balance(&positor).await {
        Ok(icp_ledger_balance) => icp_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
            reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CheckUserCyclesMarketIcpLedgerBalanceError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    };
    
    let user_icp_balance_in_the_lock: IcpTokens = with(&CM_DATA, |cm_data| { check_user_icp_balance_in_the_lock(cm_data, &positor) });
    
    let usable_user_icp_balance: IcpTokens = IcpTokens::from_e8s(user_icp_ledger_balance.e8s().saturating_sub(user_icp_balance_in_the_lock.e8s()));
    
    if usable_user_icp_balance < q.icp + ( IcpTokens::from_e8s(q.icp.e8s() / q.minimum_purchase.e8s() * ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s()) ) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
        reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::UserIcpBalanceTooLow{ user_icp_balance: usable_user_icp_balance }),));
        do_payouts().await;
        return;
    }
    
    
    
    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.icp_positions.len() >= MAX_ICP_POSITIONS {            
            if cm_data.void_icp_positions.len() >= MAX_VOID_ICP_POSITIONS {
                reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CyclesMarketIsBusy),));
                return Err(());
            }
            let icp_position_lowest_xdr_permyriad_per_icp_rate: XdrPerMyriadPerIcp = { 
                cm_data.icp_positions.iter()
                    .min_by_key(|icp_position: &&IcpPosition| { icp_position.xdr_permyriad_per_icp_rate })
                    .unwrap()
                    .xdr_permyriad_per_icp_rate
            };
            if q.xdr_permyriad_per_icp_rate < icp_position_lowest_xdr_permyriad_per_icp_rate && q.icp >= MINIMUM_ICP_POSITION_FOR_AN_ICP_POSITION_BUMP {
                // bump
                let icp_position_highest_xdr_permyriad_per_icp_rate_position_id: PositionId = {
                    cm_data.icp_positions.iter()
                        .max_by_key(|icp_position: &&IcpPosition| { icp_position.xdr_permyriad_per_icp_rate })
                        .unwrap()
                        .id
                };
                let icp_position_highest_xdr_permyriad_per_icp_rate_icp_positions_i: usize = {
                    cm_data.icp_positions.binary_search_by_key(
                        &icp_position_highest_xdr_permyriad_per_icp_rate_position_id,
                        |icp_position| { icp_position.id }
                    ).unwrap()
                };
                let icp_position_highest_xdr_permyriad_per_icp_rate: IcpPosition = cm_data.icp_positions.remove(icp_position_highest_xdr_permyriad_per_icp_rate_icp_positions_i);                
                if icp_position_highest_xdr_permyriad_per_icp_rate.id != icp_position_highest_xdr_permyriad_per_icp_rate_position_id { trap("outside the bounds of the contract.") }
                let icp_position_highest_xdr_permyriad_per_icp_rate_void_icp_positions_insertion_i = { 
                    cm_data.void_icp_positions.binary_search_by_key(
                        &icp_position_highest_xdr_permyriad_per_icp_rate_position_id,
                        |void_icp_position| { void_icp_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_icp_positions.insert(
                    icp_position_highest_xdr_permyriad_per_icp_rate_void_icp_positions_insertion_i,
                    VoidIcpPosition{                        
                        position_id:    icp_position_highest_xdr_permyriad_per_icp_rate.id,
                        positor:        icp_position_highest_xdr_permyriad_per_icp_rate.positor,
                        icp:            icp_position_highest_xdr_permyriad_per_icp_rate.icp,
                        timestamp_nanos: time_nanos(),
                        icp_payout_lock: false,
                        icp_payout_data: IcpPayoutData{
                            icp_transfer: Some(IcpTransferBlockHeightAndTimestampNanos{
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
                reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Err(CreateIcpPositionError::CyclesMarketIsFull_MaximumRateAndMinimumIcpPositionForABump{ maximum_rate_for_a_bump: icp_position_lowest_xdr_permyriad_per_icp_rate - 1, minimum_icp_position_for_a_bump: MINIMUM_ICP_POSITION_FOR_AN_ICP_POSITION_BUMP }),));
                return Err(());
            }
        } else {
            Ok(())    
        }
    }) {
        Ok(()) => {},
        Err(()) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
            do_payouts().await;
            return;
        }
    }
    
    
    
    let position_id: PositionId = with_mut(&CM_DATA, |cm_data| {
        let id: PositionId = new_id(&mut cm_data.id_counter); 
        cm_data.icp_positions.push(
            IcpPosition{
                id,   
                positor,
                icp: q.icp,
                minimum_purchase: q.minimum_purchase,
                xdr_permyriad_per_icp_rate: q.xdr_permyriad_per_icp_rate,
                timestamp_nanos: time_nanos(),
            }
        );
        id
    });
    
    msg_cycles_accept128(msg_cycles_quirement);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&positor); });
    
    reply::<(Result<CreateIcpPositionSuccess, CreateIcpPositionError>,)>((Ok(
        CreateIcpPositionSuccess{
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
        if cm_data.mid_call_user_icp_balance_locks.len() >= MAX_MID_CALL_USER_ICP_BALANCE_LOCKS {
            return Err(PurchaseCyclesPositionError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_icp_balance_locks.contains(&purchaser) {
            return Err(PurchaseCyclesPositionError::CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall);
        }
        cm_data.mid_call_user_icp_balance_locks.insert(purchaser);
        Ok(())
    }) {
        Ok(()) => {},
        Err(purchase_cycles_position_error) => {
            reply::<(PurchaseCyclesPositionResult,)>((Err(purchase_cycles_position_error),));
            do_payouts().await;
            return;
        }
    }
    
    // check icp balance and make sure to unlock the user on returns after here 
    let user_icp_ledger_balance: IcpTokens = match check_user_cycles_market_icp_ledger_balance(&purchaser).await {
        Ok(icp_ledger_balance) => icp_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&purchaser); });
            reply::<(PurchaseCyclesPositionResult,)>((Err(PurchaseCyclesPositionError::CheckUserCyclesMarketIcpLedgerBalanceError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    };
    
    let user_icp_balance_in_the_lock: IcpTokens = with(&CM_DATA, |cm_data| { check_user_icp_balance_in_the_lock(cm_data, &purchaser) });
    
    let usable_user_icp_balance: IcpTokens = IcpTokens::from_e8s(user_icp_ledger_balance.e8s().saturating_sub(user_icp_balance_in_the_lock.e8s()));

    let cycles_position_purchase_id: PurchaseId = match with_mut(&CM_DATA, |cm_data| {
        if cm_data.cycles_positions_purchases.len() >= MAX_CYCLES_POSITIONS_PURCHASES {
            let remove_cpp_id: PurchaseId = match cm_data.cycles_positions_purchases.iter().filter(
                |cycles_position_purchase: &&CyclesPositionPurchase| {
                    ( cycles_position_purchase.cycles_payout_data.is_complete() || ( cycles_position_purchase.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() && time_nanos().saturating_sub(cycles_position_purchase.cycles_payout_data.cmcaller_cycles_payout_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) ) && ( cycles_position_purchase.icp_payout_data.is_complete() || ( cycles_position_purchase.icp_payout_data.is_waiting_for_the_cmcaller_callback() && time_nanos().saturating_sub(cycles_position_purchase.icp_payout_data.cm_message_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) ) 
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
        if q.cycles % cycles_position_ref.xdr_permyriad_per_icp_rate as u128 != 0 {
            return Err(PurchaseCyclesPositionError::PurchaseCyclesMustBeAMultipleOfTheXdrPerMyriadPerIcpRate);
        }
        if cycles_position_ref.cycles < q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionCyclesIsLessThanThePurchaseQuest{ cycles_position_cycles: cycles_position_ref.cycles });
        }
        if cycles_position_ref.minimum_purchase > q.cycles {
            return Err(PurchaseCyclesPositionError::CyclesPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ cycles_position_minimum_purchase: cycles_position_ref.minimum_purchase });
        }        
        
        if usable_user_icp_balance < cycles_to_icptokens(q.cycles, cycles_position_ref.xdr_permyriad_per_icp_rate) + ICP_LEDGER_TRANSFER_DEFAULT_FEE {
            return Err(PurchaseCyclesPositionError::UserIcpBalanceTooLow{ user_icp_balance: usable_user_icp_balance });
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
                cycles_position_xdr_permyriad_per_icp_rate: cycles_position_ref.xdr_permyriad_per_icp_rate,
                id: cycles_position_purchase_id,
                purchaser,
                cycles: q.cycles,
                timestamp_nanos: time_nanos(),
                cycles_payout_lock: false,
                icp_payout_lock: false,
                cycles_payout_data: CyclesPayoutData::new(),
                icp_payout_data: IcpPayoutData{
                    icp_transfer: None,
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
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&purchaser); });
            reply::<(PurchaseCyclesPositionResult,)>((Err(purchase_cycles_position_error),));
            do_payouts().await;
            return;
        }
    };
    
    msg_cycles_accept128(PURCHASE_POSITION_FEE);

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&purchaser); });
    reply::<(PurchaseCyclesPositionResult,)>((Ok(PurchaseCyclesPositionSuccess{
        purchase_id: cycles_position_purchase_id
    }),));
    do_payouts().await;
    return;

}





// -------------------



#[update(manual_reply = true)]
pub async fn purchase_icp_position(q: PurchaseIcpPositionQuest) {

    let purchaser: Principal = caller();

    let icp_position_purchase_id: PurchaseId = match with_mut(&CM_DATA, |cm_data| {
        if cm_data.icp_positions_purchases.len() >= MAX_ICP_POSITIONS_PURCHASES {
            let remove_ipp_id: PurchaseId = match cm_data.icp_positions_purchases.iter().filter(
                |icp_position_purchase: &&IcpPositionPurchase| {
                    ( icp_position_purchase.cycles_payout_data.is_complete() || ( icp_position_purchase.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() && time_nanos().saturating_sub(icp_position_purchase.cycles_payout_data.cmcaller_cycles_payout_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) ) && ( icp_position_purchase.icp_payout_data.is_complete() || ( icp_position_purchase.icp_payout_data.is_waiting_for_the_cmcaller_callback() && time_nanos().saturating_sub(icp_position_purchase.icp_payout_data.cm_message_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK ) )
                }
            ).min_by_key(
                |icp_position_purchase: &&IcpPositionPurchase| {
                    icp_position_purchase.timestamp_nanos
                }    
            ) {
                None => {
                    return Err(PurchaseIcpPositionError::CyclesMarketIsBusy);    
                },
                Some(remove_ipp) => {
                    remove_ipp.id
                }
            };
            let remove_ipp_icp_positions_purchases_i: usize = match cm_data.icp_positions_purchases.binary_search_by_key(&remove_ipp_id, |ipp| { ipp.id }) {
                Ok(i) => i,
                Err(_) => /*will not happen*/return Err(PurchaseIcpPositionError::CyclesMarketIsBusy),
            };
            cm_data.icp_positions_purchases.remove(remove_ipp_icp_positions_purchases_i);            
        }
        let icp_position_icp_positions_i: usize = match cm_data.icp_positions.binary_search_by_key(
            &q.icp_position_id,
            |icp_position| { icp_position.id }
        ) {
            Ok(i) => i,
            Err(_) => { return Err(PurchaseIcpPositionError::IcpPositionNotFound); }
        };
        let icp_position_ref: &IcpPosition = &cm_data.icp_positions[icp_position_icp_positions_i];
        if icp_position_ref.icp < q.icp {
            return Err(PurchaseIcpPositionError::IcpPositionIcpIsLessThanThePurchaseQuest{ icp_position_icp: icp_position_ref.icp });
        }
        if icp_position_ref.minimum_purchase > q.icp {
            return Err(PurchaseIcpPositionError::IcpPositionMinimumPurchaseIsGreaterThanThePurchaseQuest{ icp_position_minimum_purchase: icp_position_ref.minimum_purchase });
        }        

        if icp_position_ref.icp - q.icp < icp_position_ref.minimum_purchase 
        && icp_position_ref.icp - q.icp != IcpTokens::from_e8s(0)
        && cm_data.void_icp_positions.len() >= MAX_VOID_ICP_POSITIONS {
            return Err(PurchaseIcpPositionError::CyclesMarketIsBusy);
        }
        

        let msg_cycles_quirement: Cycles = PURCHASE_POSITION_FEE + icptokens_to_cycles(q.icp, icp_position_ref.xdr_permyriad_per_icp_rate); 
        if msg_cycles_available128() < msg_cycles_quirement {
            return Err(PurchaseIcpPositionError::MsgCyclesTooLow{ purchase_position_fee: PURCHASE_POSITION_FEE });
        }
        msg_cycles_accept128(msg_cycles_quirement);
                
        let icp_position_purchase_id: PurchaseId = new_id(&mut cm_data.id_counter);
        
        cm_data.icp_positions_purchases.push(
            IcpPositionPurchase{
                icp_position_id: icp_position_ref.id,
                icp_position_positor: icp_position_ref.positor,
                icp_position_xdr_permyriad_per_icp_rate: icp_position_ref.xdr_permyriad_per_icp_rate,
                id: icp_position_purchase_id,
                purchaser,
                icp: q.icp,
                timestamp_nanos: time_nanos(),
                cycles_payout_lock: false,
                icp_payout_lock: false,
                cycles_payout_data: CyclesPayoutData::new(),
                icp_payout_data: IcpPayoutData{
                    icp_transfer: None,
                    cm_message_call_success_timestamp_nanos: None,
                    cm_message_callback_complete: None    
                }
            }
        );

        std::mem::drop(icp_position_ref);
        cm_data.icp_positions[icp_position_icp_positions_i].icp -= q.icp;
        if cm_data.icp_positions[icp_position_icp_positions_i].icp < cm_data.icp_positions[icp_position_icp_positions_i].minimum_purchase {            
            let icp_position_for_the_void: IcpPosition = cm_data.icp_positions.remove(icp_position_icp_positions_i);
            if icp_position_for_the_void.icp.e8s() != 0 {
                let icp_position_for_the_void_void_icp_positions_insertion_i: usize = { 
                    cm_data.void_icp_positions.binary_search_by_key(
                        &icp_position_for_the_void.id,
                        |void_icp_position| { void_icp_position.position_id }
                    ).unwrap_err()
                };
                cm_data.void_icp_positions.insert(
                    icp_position_for_the_void_void_icp_positions_insertion_i,
                    VoidIcpPosition{
                        position_id:    icp_position_for_the_void.id,
                        positor:        icp_position_for_the_void.positor,
                        icp:            icp_position_for_the_void.icp,
                        timestamp_nanos: time_nanos(),
                        icp_payout_lock: false,
                        icp_payout_data: IcpPayoutData{
                            icp_transfer: Some(IcpTransferBlockHeightAndTimestampNanos{
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
        
        Ok(icp_position_purchase_id)
    }) {
        Ok(icp_position_purchase_id) => icp_position_purchase_id,
        Err(purchase_icp_position_error) => {
            reply::<(PurchaseIcpPositionResult,)>((Err(purchase_icp_position_error),));
            do_payouts().await;
            return;
        }
    };
    
    
    reply::<(PurchaseIcpPositionResult,)>((Ok(PurchaseIcpPositionSuccess{
        purchase_id: icp_position_purchase_id
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
        } else if let Ok(icp_position_i) = cm_data.icp_positions.binary_search_by_key(&q.position_id, |icp_position| { icp_position.id }) {
            if cm_data.icp_positions[icp_position_i].positor != caller() {
                return Err(VoidPositionError::WrongCaller);
            }
            if time_seconds().saturating_sub(cm_data.icp_positions[icp_position_i].timestamp_nanos/NANOS_IN_A_SECOND) < VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS {
                return Err(VoidPositionError::MinimumWaitTime{ minimum_wait_time_seconds: VOID_POSITION_MINIMUM_WAIT_TIME_SECONDS, position_creation_timestamp_seconds: cm_data.icp_positions[icp_position_i].timestamp_nanos/NANOS_IN_A_SECOND });
            }
            if cm_data.void_icp_positions.len() >= MAX_VOID_ICP_POSITIONS {
                return Err(VoidPositionError::CyclesMarketIsBusy);
            }
            let icp_position_for_the_void: IcpPosition = cm_data.icp_positions.remove(icp_position_i);
            let icp_position_for_the_void_void_icp_positions_insertion_i: usize = cm_data.void_icp_positions.binary_search_by_key(&icp_position_for_the_void.id, |vip| { vip.position_id }).unwrap_err();
            cm_data.void_icp_positions.insert(
                icp_position_for_the_void_void_icp_positions_insertion_i,
                VoidIcpPosition{
                    position_id:    icp_position_for_the_void.id,
                    positor:        icp_position_for_the_void.positor,
                    icp:            icp_position_for_the_void.icp,
                    timestamp_nanos: time_nanos(),
                    icp_payout_lock: false,
                    icp_payout_data: IcpPayoutData{
                        icp_transfer: Some(IcpTransferBlockHeightAndTimestampNanos{
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

#[query]
pub fn see_icp_lock() -> IcpTokens {
    with(&CM_DATA, |cm_data| { check_user_icp_balance_in_the_lock(cm_data, &caller()) })
}


// ----------------


#[update(manual_reply = true)]
pub async fn transfer_icp_balance(q: TransferIcpBalanceQuest) {
    
    let user_id: Principal = caller();
    
    if msg_cycles_available128() < TRANSFER_ICP_BALANCE_FEE {
        reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::MsgCyclesTooLow{ transfer_icp_balance_fee: TRANSFER_ICP_BALANCE_FEE }),));
        do_payouts().await;
        return;
    }

    match with_mut(&CM_DATA, |cm_data| {
        if cm_data.mid_call_user_icp_balance_locks.len() >= MAX_MID_CALL_USER_ICP_BALANCE_LOCKS {
            return Err(TransferIcpBalanceError::CyclesMarketIsBusy);
        }
        if cm_data.mid_call_user_icp_balance_locks.contains(&user_id) {
            return Err(TransferIcpBalanceError::CallerIsInTheMiddleOfACreateIcpPositionOrPurchaseCyclesPositionOrTransferIcpBalanceCall);
        }
        cm_data.mid_call_user_icp_balance_locks.insert(user_id);
        Ok(())
    }) {
        Ok(()) => {},
        Err(transfer_icp_balance_error) => {
            reply::<(TransferIcpBalanceResult,)>((Err(transfer_icp_balance_error),));
            do_payouts().await;
            return;
        }
    }
    
    // check icp balance and make sure to unlock the user on returns after here 
    let user_icp_ledger_balance: IcpTokens = match check_user_cycles_market_icp_ledger_balance(&user_id).await {
        Ok(icp_ledger_balance) => icp_ledger_balance,
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&user_id); });
            reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::CheckUserCyclesMarketIcpLedgerBalanceCallError((call_error.0 as u32, call_error.1))),));
            do_payouts().await;
            return;            
        }
    };
    
    let user_icp_balance_in_the_lock: IcpTokens = with(&CM_DATA, |cm_data| { check_user_icp_balance_in_the_lock(cm_data, &user_id) });
    
    let usable_user_icp_balance: IcpTokens = IcpTokens::from_e8s(user_icp_ledger_balance.e8s().saturating_sub(user_icp_balance_in_the_lock.e8s()));
    
    if usable_user_icp_balance < IcpTokens::from_e8s(q.icp.e8s().saturating_add(q.icp_fee.e8s())) {
        with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&user_id); });
        reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::UserIcpBalanceTooLow{ user_icp_balance: usable_user_icp_balance }),));
        do_payouts().await;
        return;          
    }

    match icp_transfer(
        MAINNET_LEDGER_CANISTER_ID,
        IcpTransferArgs {
            memo: TRANSFER_ICP_BALANCE_MEMO,
            amount: q.icp,
            fee: q.icp_fee,
            from_subaccount: Some(principal_icp_subaccount(&user_id)),
            to: q.to,
            created_at_time: Some(IcpTimestamp { timestamp_nanos: time_nanos_u64()-NANOS_IN_A_SECOND as u64 })
        }   
    ).await {
        Ok(icp_transfer_result) => match icp_transfer_result {
            Ok(icp_transfer_block_height) => {
                msg_cycles_accept128(TRANSFER_ICP_BALANCE_FEE);
                reply::<(TransferIcpBalanceResult,)>((Ok(icp_transfer_block_height),));
            },
            Err(icp_transfer_error) => {
                match icp_transfer_error {
                    IcpTransferError::BadFee{ .. } => {
                        msg_cycles_accept128(TRANSFER_ICP_BALANCE_FEE);
                    },
                    _ => {}
                }
                reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::IcpTransferError(icp_transfer_error)),));
            }
        },
        Err(icp_transfer_call_error) => {
            reply::<(TransferIcpBalanceResult,)>((Err(TransferIcpBalanceError::IcpTransferCallError((icp_transfer_call_error.0 as u32, icp_transfer_call_error.1))),));
        }
    }

    with_mut(&CM_DATA, |cm_data| { cm_data.mid_call_user_icp_balance_locks.remove(&user_id); });
    do_payouts().await;
    return;
}



// -------------------------------

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
pub struct SeeIcpPositionsQuest {
    chunk_i: u128
}

#[query(manual_reply = true)]
pub fn see_icp_positions(q: SeeIcpPositionsQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[IcpPosition]>,)>((
            cm_data.icp_positions.chunks(SEE_ICP_POSITIONS_CHUNK_SIZE).nth(q.chunk_i as usize)
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
pub struct SeeIcpPositionsPurchasesQuest {
    chunk_i: u128
}

#[query(manual_reply = true)]
pub fn see_icp_positions_purchases(q: SeeIcpPositionsPurchasesQuest) {
    with(&CM_DATA, |cm_data| {
        reply::<(Option<&[IcpPositionPurchase]>,)>((
            cm_data.icp_positions_purchases.chunks(SEE_ICP_POSITIONS_PURCHASES_CHUNK_SIZE).nth(q.chunk_i as usize)
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
            .icp_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_icp_position_purchase_purchaser_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }
        
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(icp_position_purchase_icp_positions_purchases_i) = cm_data.icp_positions_purchases.binary_search_by_key(&q.cm_call_id, |icp_position_purchase| { icp_position_purchase.id }) {
            cm_data.icp_positions_purchases[icp_position_purchase_icp_positions_purchases_i]
            .icp_payout_data
            .cm_message_callback_complete = Some(q.opt_call_error);
        }
    });
    
    reply::<()>(());
    do_payouts().await;
    return;
}

#[update(manual_reply = true)]
pub async fn cm_message_icp_position_purchase_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    let cycles_transfer_refund: Cycles = msg_cycles_accept128(msg_cycles_available128());
    
    with_mut(&CM_DATA, |cm_data| {
        if let Ok(icp_position_purchase_icp_positions_purchases_i) = cm_data.icp_positions_purchases.binary_search_by_key(&q.cm_call_id, |icp_position_purchase| { icp_position_purchase.id }) {
            cm_data.icp_positions_purchases[icp_position_purchase_icp_positions_purchases_i]
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
pub async fn cm_message_void_icp_position_positor_cmcaller_callback(q: CMCallbackQuest) {
    if caller() != with(&CM_DATA, |cm_data| { cm_data.cm_caller }) {
        trap("this method is for the cycles_market-caller");
    }

    with_mut(&CM_DATA, |cm_data| {
        if let Ok(void_icp_position_void_icp_positions_i) = cm_data.void_icp_positions.binary_search_by_key(&q.cm_call_id, |void_icp_position| { void_icp_position.position_id }) {
            cm_data.void_icp_positions[void_icp_position_void_icp_positions_i]
            .icp_payout_data
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



