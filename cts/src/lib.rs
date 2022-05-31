
// lock each user from making other calls on each async call that awaits, like the collect_balance call, lock the user at the begining and unlock the user at the end. 
// will callbacks (the code after an await) get dropped if the subnet is under heavy load?
// when calling canisters that i dont know if they can possible give-back unexpected candid, use call_raw and dont panic on the candid-decode, return an error.
// dont want to implement From<(RejectionCode, String)> for the return errors in the calls async that call other canisters because if the function makes more than one call then the ? with the from can give-back a wrong error type 
// always check user lock before any awaits (or maybe after the first await if not fective?). 
// in the cycles-market, let a seller set a minimum-purchase-quantity. which can be the full-mount that is up for the sale or less 
// always unlock the user af-ter the last await-call()
// does dereferencing a borrow give the ownership? try on a non-copy type. yes it does a move
// sending cycles to a canister is the same risk as sending icp to a canister. 
// put a max_fee on a cycles-transfer-purchase & on a cycles-bank-purchase?
// 5xdr first-time-user-fee, valid for one year. with 100mbs of storage for the year and standard-call-rate-limits. after the year, if the user doesn't pay for more space, the user-storage gets deleted and the user cycles balance and icp balance stays for another 3 years.
// if a user.user_canister == None: means the user must pay for some storage minimum 5xdr see^, if it wants to do something
// 0.1 GiB / 102.4 Mib / 107374182.4 bytes user-storage for the 1 year for the 5xdr. 

// maybe use the notify/one-way call for the cycles-transfers, and take the cycles from the users-balance before the call? but then i can never know the cept-mount by the callee. I think using the cycles_transferrer canister is a good way to do it.

// choice for users to download a signed data canister-signature of past trassactions. 
// choice for users to delete past transactions to re-claim storage-space  
// if a user requested cycles-transfer call takes-more than half an hour to come back, the user is not refunded any cycles the callee does'nt take

// user does the main operations through the user_canister.
// the user-lock is on the user-canister

// each method is a contract


// icp transfers , 0.10-xdr / 14-cents flat fee


// tegrate with the icscan.io









//#![allow(unused)] // take this out when done
#![allow(non_camel_case_types)]

use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;


use ic_cdk::{
    api::{
        trap,
        caller, 
        time, 
        call::{
            arg_data,
            arg_data_raw,
            arg_data_raw_size,
            call_raw128,
            call,
            call_with_payment128,
            CallResult,
            RejectionCode,
            msg_cycles_refunded128,
            msg_cycles_available128,
            msg_cycles_accept128,
            reject,
            reply,
        },
    },
    export::{
        Principal,
        candid::{
            CandidType,
            Deserialize,
            utils::{
                encode_one, 
                // decode_one
            },
        },
    },
};
use ic_cdk_macros::{update, query, init, pre_upgrade, post_upgrade};

use global_allocator_counter::get_allocated_bytes_count;

use cts_lib::{
    types::{
        UserData,
        UserLock,
        CyclesTransferPurchaseLog,
        CyclesBankPurchaseLog,
        CyclesTransfer,
        CyclesTransferMemo,

    },
    consts::{
        MANAGEMENT_CANISTER_ID,
    },
    tools::{
        sha256,
        localkey_refcell::{
            self,
            with, 
            with_mut,
        },

    },
    ic_ledger_types::{
        IcpMemo,
        IcpId,
        IcpIdSub,
        IcpTokens,
        IcpBlockHeight,
        IcpTimestamp,
        ICP_DEFAULT_SUBACCOUNT,
        ICP_LEDGER_TRANSFER_DEFAULT_FEE,
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        MAINNET_LEDGER_CANISTER_ID, 
        icp_transfer,
        IcpTransferArgs, 
        IcpTransferResult, 
        IcpTransferError,
        icp_account_balance,
        IcpAccountBalanceArgs
    }
};


#[cfg(test)]
mod t;


mod tools;
use tools::{
    principal_icp_subaccount,
    user_icp_balance_id,
    user_cycles_balance_topup_memo_bytes,
    check_user_icp_balance,
    check_user_icp_ledger_balance,
    check_user_cycles_balance,
    CheckUserCyclesBalanceError,
    main_cts_icp_id,
    check_lock_and_lock_user,
    unlock_user,
    CheckCurrentXdrPerMyriadPerIcpCmcRateError,
    check_current_xdr_permyriad_per_icp_cmc_rate,
    // icptokens_to_cycles,
    cycles_to_icptokens,
    get_new_canister,
    GetNewCanisterError,
    // ManagementCanisterCanisterSettings,
    ManagementCanisterOptionalCanisterSettings,
    ManagementCanisterCanisterStatusRecord,
    ManagementCanisterCanisterStatusVariant,
    CanisterIdRecord,
    ChangeCanisterSettingsRecord,
    CYCLES_BALANCE_TOPUP_MEMO_START,
    thirty_bytes_as_principal,
    ledger_topup_cycles,
    LedgerTopupCyclesError,
    IcpXdrConversionRate,
    FindAndLockUserSponse,
    take_user_icp_ledger,
    find_and_plus_user_cycles_balance,
    FindAndPlusUserCyclesBalanceError,
    ICP_LEDGER_CREATE_CANISTER_MEMO,
    CmcNotifyError,
    CmcNotifyCreateCanisterQuest,
    users_map_canister_write_user_data,
    PutNewUserIntoAUsersMapCanisterError,
    put_new_user_into_a_users_map_canister,
    
    
    
};


mod stable;
use stable::{
    save_users_data,
    read_users_data,
    save_new_canisters,
    read_new_canisters

};





pub type Cycles = u128;
pub type LatestKnownCmcRate = IcpXdrConversionRate; 





mod canister_code {

    pub struct CanisterCode {
        module: Vec<u8>,
        module_hash: [u8; 32] 
    }

    impl CanisterCode {
        pub fn new(mut module: Vec<u8>) -> Self { // :mut for the shrink_to_fit
            module.shrink_to_fit();
            Self {
                module_hash: cts_lib::tools::sha256(&module), // put this on the top if move error
                module: module,
            }
        }
        pub fn module(&self) -> &Vec<u8> {
            &self.module
        }
        pub fn module_hash(&self) -> &[u8; 32] {
            &self.module_hash
        }
        pub fn change_module(&mut self, module: Vec<u8>) {
            *self = Self::new(module);
        }
    }
}

use canister_code::CanisterCode;






// :FEES.
pub const CYCLES_TRANSFER_FEE: u128 = 100_000_000_000;
pub const CONVERT_ICP_FOR_THE_CYCLES_WITH_THE_CMC_RATE_FEE: u128 = 1; // 100_000_000_000
pub const CYCLES_BANK_COST: u128 = 1; // 10_000_000_000_000;
pub const CYCLES_BANK_UPGRADE_COST: u128 = 5; // 5_000_000_000_000;
pub const ICP_PAYOUT_FEE: IcpTokens = IcpTokens::from_e8s(30000);// calculate through the xdr conversion rate ? // 100_000_000_000-cycles
pub const MINIMUM_CYCLES_TRANSFER_IN: u128 = 50_000_000_000; // enough to pay for a find_and_lock_user-call.
pub const FIND_AND_PLUS_USER_CYCLES_BALANCE_USER_NOT_FOUND_FEE: Cycles = (100_000 + 260_000 + 590_000) * with(&USERS_MAP_CANISTERS, |umcs| umcs.len()); // :do: clude wasm-instructions-counts
pub const CYCLES_PER_USER_PER_103_MiB_PER_YEAR: u128 = 5_000_000_000_000;





pub const ICP_PAYOUT_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-POUT"));
pub const ICP_FEE_MEMO: IcpMemo = IcpMemo(u64::from_be_bytes(*b"CTS-TFEE"));
pub const MAX_NEW_USERS: usize = 5000; // the max number of entries in the NEW_USERS-hashmap at the same-time
pub const MAX_USERS_MAP_CANISTERS: usize = 4; // can be 30-million at 1-gb, or 3-million at 0.1-gb,




thread_local! {

    pub static NEW_USERS: RefCell<HashMap<Principal, NewUserData>> = RefCell::new(Vec::new());
    pub static USERS_MAP_CANISTERS: RefCell<Vec<Principal>> = RefCell::new(vec![ic_cdk::api::id()]);
    pub static LATEST_KNOWN_CMC_RATE: Cell<LatestKnownCmcRate> = Cell::new(LatestKnownCmcRate { xdr_permyriad_per_icp: 0, timestamp_seconds: 0 });
    
    pub static CYCLES_BANK_CANISTER_CODE    : RefCell<Option<CanisterCode>> = RefCell::new(None);
    pub static USER_CANISTER_CODE           : RefCell<Option<CanisterCode>> = RefCell::new(None);
    pub static USERS_MAP_CANISTER_CODE      : RefCell<Option<CanisterCode>> = RefCell::new(None);
    
    pub static NEW_CANISTERS: RefCell<Vec<Principal>> = RefCell::new(Vec::new());
    
}






// if a user for the topup is not found, the cycles-transfer-station takes a fee for the user-lookup(:fee is with the base on how many users_map_canisters there are) and refunds the rest of the cycles. 
// make sure the minimum-in-cycles-transfer is more than the find_and_plus_user_cycles_balance_user_not_found_fee


#[export_name="canister_update cycles_transfer"]
pub fn cycles_transfer() {
    if arg_data_raw_size() > 100 {
        trap("arg_data_raw_size can be max 100 bytes")
    }
    
    let cycles_available: Cycles = msg_cycles_available128();
    
    if cycles_available < MINIMUM_CYCLES_TRANSFER_IN {
        trap(&format!("minimum cycles transfer: {}", MINIMUM_CYCLES_TRANSFER_IN))
    }

    let (ct,): (CyclesTransfer,) = arg_data<(CyclesTransfer,)>();

    match ct.memo {
        CyclesTransferMemo::Blob(memo_bytes) => {
            if memo_bytes.len() != 32 || &memo_bytes[..2] != CYCLES_BALANCE_TOPUP_MEMO_START {
                trap("unknown cycles transfer memo")
            }

            let user_id: Principal = thirty_bytes_as_principal(&memo_bytes[2..32].try_into().unwrap());
            
            match find_and_plus_user_cycles_balance(&user_id, plus_cycles: cycles_available).await {
                Ok(()) => {
                    reply(());
                },
                Err(find_and_plus_user_cycles_balance_error) => match find_and_plus_user_cycles_balance_error {
                    FindAndPlusUserCyclesBalanceError::UserNotFound => {
                        msg_cycles_accept128(FIND_AND_PLUS_USER_CYCLES_BALANCE_USER_NOT_FOUND_FEE);
                        reject(&format("User for the top up not found. {} cycles taken for a nonexistentuserfee", FIND_AND_PLUS_USER_CYCLES_BALANCE_USER_NOT_FOUND_FEE));
                    },
                    FindAndPlusUserCyclesBalanceError::UsersMapCanisterCallError => trap("User lookup error")
                }
            };
        },
        
        _ => trap("CyclesTransferMemo must be the Blob variant")
    };
}











#[derive(CandidType, Deserialize)]
pub struct TopUpCyclesBalanceData {
    topup_cycles_transfer_canister: Principal,
    topup_cycles_transfer_memo: CyclesTransferMemo,
}

#[derive(CandidType, Deserialize)]
pub struct TopUpIcpBalanceData {
    topup_icp_id: IcpId
}

#[derive(CandidType, Deserialize)]
pub struct TopUpBalanceData {
    topup_cycles_balance: TopUpCyclesBalanceData, 
    topup_icp_balance: TopUpIcpBalanceData,
}


#[query]
pub fn topup_balance() -> TopUpBalanceData {
    let user_id: Principal = caller();
    TopUpBalanceData {
        topup_cycles_balance: TopUpCyclesBalanceData {
            topup_cycles_transfer_canister: ic_cdk::api::id(),
            topup_cycles_transfer_memo: CyclesTransferMemo::Blob(user_cycles_balance_topup_memo_bytes(&user_id).to_vec())
        },
        topup_icp_balance: TopUpIcpBalanceData {
            topup_icp_id: user_icp_balance_id(&user_id)
        }
    }
}









pub type UsersMapCanisterId = Principal;

#[derive(Copy,Clone)]
struct NewUserData {
    lock_start_time_nanos: u64,
    lock: bool,
    
    // the options are for the memberance of the steps
    user_icp_ledger_balance: Option<IcpTokens>,
    current_xdr_icp_rate: Option<u64>,
    users_map_canister_data: Option<(UserData, Option<UsersMapCanisterId>)>,    
    create_user_canister_block_height: Option<IcpBlockHeight>,
    user_canister_uninstall_code: bool,
    user_canister_install_code: bool,
    user_canister_status_record: Option<ManagementCanisterCanisterStatusRecord>,
    users_map_canister_write_user_data: bool,
    collect_icp: bool,
    
    

}

impl NewUserData {
    pub fn new() -> Self {
        Self {
            lock_start_time_nanos: time(),
            lock: true,
            ..Default::default()
        }
    }
    
    pub fn current_membership_cost_icp(&self) -> Option<IcpTokens> {
        match self.current_xdr_icp_rate {
            Some(r) => Some(cycles_to_icptokens(CYCLES_PER_USER_PER_103_MiB_PER_YEAR, r)),
            None => None
        }
    }
}



#[derive(CandidType, Deserialize)]
pub enum NewUserMidCallError{
    UsersMapCanisterFindUserCallFail,
    PutNewUserIntoAUsersMapCanisterError(PutNewUserIntoAUsersMapCanisterError),
    CreateUserCanisterIcpTransferError(IcpTransferError),
    CreateUserCanisterIcpTransferCallError(String),
    CreateUserCanisterCmcNotifyError(CmcNotifyError),
    CreateUserCanisterCmcNotifyCallError(String),
    UserCanisterCodeNotFound,
    UserCanisterInstallCodeCallError(String),
    UserCanisterStatusCallError(String),
    UserCanisterModuleVerificationError,
    UserCanisterStartCanisterCallError(String),
    UsersMapCanisterWriteUserDataCallError(String),
    IcpTransferCallError(String),
    IcpTransferError(IcpTransferError),
    
}


#[derive(CandidType, Deserialize)]
pub enum NewUserError{
    CheckIcpBalanceCallError(String),
    CheckCurrentXdrPerMyriadPerIcpCmcRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    UserIcpLedgerBalanceTooLow{
        membership_cost_icp: IcpTokens,
        user_icp_ledger_balance: IcpTokens,
        icp_ledger_transfer_fee: IcpTokens
    },
    FoundUserCanister(Principal),
    UserIcpBalanceTooLow{
        membership_cost_icp: IcpTokens,
        user_icp_balance: IcpTokens,
        icp_ledger_transfer_fee: IcpTokens
    },
    CreateUserCanisterCmcNotifyError(CmcNotifyError),
    MidCallError(NewUserMidCallError),    // re-try the call on this sponse
}

#[derive(CandidType, Deserialize)]
pub enum NewUserSuccessData {
    users_map_canister: Principal,
    user_canister: Principal,
}


fn write_new_user_data(user_id: &Principal, new_user_data: NewUserData) {
    with_mut(&NEW_USERS, |new_users| {
        match new_users.get_mut(user_id) {
            Some(nud) => { *nud = new_user_data; },
            None => {}
        }
    });
    
}

// for the now a user must sign-up/register with the icp.
#[update]
pub async fn new_user() -> Result<NewUserSuccessData, NewUserError> {
    let user_id: Principal = caller();
    
    let mut new_user_data: NewUserData; 
    
    with_mut(&NEW_USERS, |new_users| {
        match new_users.get_mut(&user_id) {
            Some(mut nud) => {
                if nud.lock == true {
                    trap("new user is in the middle of another call")
                }
                nud.lock = true;
                // update nud.lock_time_start_nanos?
                new_user_data = *nud; // copy
            },
            None => {
                if new_users.len() >= MAX_NEW_USERS {
                    trap("max limit of creating new users at the same-time. try your call in a couple of seconds.")
                }
                let nud: NewUsersData = NewUsersData::new();
                new_users.insert(user_id, nud);
                new_user_data = *nud; //copy
            }
        }
    });


    if new_user_data.user_icp_ledger_balance.is_none() || new_user_data.current_xdr_icp_rate.is_none() {

        let (
            check_user_icp_ledger_balance_sponse                : CallResult<IcpTokens>, 
            check_current_xdr_permyriad_per_icp_cmc_rate_sponse : CheckCurrentXdrPerMyriadPerIcpCmcRateSponse
        ) = futures::future::join(
            check_user_icp_ledger_balance(&user_id), 
            check_current_xdr_permyriad_per_icp_cmc_rate()
        ).await; 
        
        let user_icp_ledger_balance: IcpTokens = match check_user_icp_ledger_balance_sponse {
            Ok(tokens) => tokens,
            Err(check_balance_call_error) => {
                with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
                return Err(NewUserError::CheckIcpBalanceCallError(format!("{:?}", check_balance_call_error)));
            }
        };
                
        let current_xdr_icp_rate: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate_sponse {
            Ok(rate) => rate,
            Err(check_xdr_icp_rate_error) => {
                with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
                return Err(NewUserError::CheckCurrentXdrPerMyriadPerIcpCmcRateError(check_xdr_icp_rate_error));
            }
        };
        
        let current_membership_cost_icp: IcpTokens = cycles_to_icptokens(CYCLES_PER_USER_PER_103_MiB_PER_YEAR, current_xdr_icp_rate); 
        
        if user_icp_ledger_balance < current_membership_cost_icp + IcpTokens.from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) {
            with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
            return Err(NewUserError::UserIcpLedgerBalanceTooLow{
                membership_cost_icp: current_membership_cost_icp,
                user_icp_ledger_balance,
                icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE
            });
        }

        new_user_data.user_icp_ledger_balance = Some(user_icp_ledger_balance);
        new_user_data.current_xdr_icp_rate = Some(current_xdr_icp_rate); 
        // after this, use new_user_data.field to use the data
    }


    if new_user_data.users_map_canister_data == None {
                
        let users_map_canister_data: (UserData, Option<UsersMapCanisterId>) = match find_user(&user_id).await {
            Ok((user_data, users_map_canister_id)) => {
                if let Some(uc) = user_data.user_canister {
                    with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
                    return Err(NewUserError::FoundUserCanister(uc));
                }
                if new_user_data.user_icp_ledger_balance - user_data.untaken_icp_to_collect < new_user_data.current_membership_cost_icp().unwrap() + IcpTokens.from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) {                             // can unwrap bc the code makes sure at this point there is a new_user_data.current_xdr_icp_rate 
                    with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
                    return Err(NewUserError::UserIcpBalanceTooLow{
                        membership_cost_icp: new_user_data.current_membership_cost_icp().unwrap(),
                        user_icp_balance: new_user_data.user_icp_ledger_balance - user_data.untaken_icp_to_collect,
                        icp_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE
                    });
                }
                (user_data, Some(users_map_canister_id))
            },
            Err(find_user_error) => match find_user_error {
                FindUserError::UserNotFound => {
                    (UserData::new(), None)
                },
                FindUserError::UsersMapCanisterCallFail => {
                    new_user_data.lock = false;
                    write_new_user_data(&user_id, new_user_data);
                    return Err(NewUserError::MidCallError(NewUserMidCallError::UsersMapCanisterFindUserCallFail));
                }
            }
        };
        
        new_user_data.users_map_canister_data = Some(users_map_canister_data);
    }
    

    if new_user_data.users_map_canister_data.unwrap().1.is_none() {
        
        let users_map_canister_id: UsersMapCanisterId = put_new_user_into_a_users_map_canister(user_id, new_user_data.users_map_canister_data.unwrap().0).await {
            Ok(umcid) => umcid,
            Err(put_new_user_into_a_users_map_canister_error) => {
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::PutNewUserIntoAUsersMapCanisterError(put_new_user_into_a_users_map_canister_error)));
            }
        };
        
        new_user_data.users_map_canister_data.unwrap().1 = Some(users_map_canister_id);
    }
    

    if new_user_data.create_user_canister_block_height.is_none() {
        let create_user_canister_block_height: IcpBlockHeight = match icp_transfer(
            MAINNET_LEDGER_CANISTER_ID,
            IcpTransferArgs {
                memo: ICP_LEDGER_CREATE_CANISTER_MEMO,
                amount: IcpTokens.from_e8s(new_user_data.current_membership_cost_icp().unwrap().e8s() * 0.7),                              
                fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                from_subaccount: Some(principal_icp_subaccount(&user_id)),
                to: IcpId::new(&MAINNET_CYCLES_MINTING_CANISTER_ID, &principal_icp_subaccount(&id())),
                created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
            }
        ).await {
            Ok(transfer_result) => match transfer_result {
                Ok(block_height) => block_height,
                Err(transfer_error) => {
                    new_user_data.lock = false;
                    write_new_user_data(&user_id, new_user_data);
                    return Err(NewUserError::MidCallError(NewUserMidCallError::CreateUserCanisterIcpTransferError(transfer_error)));                    
                }
            },
            Err(transfer_call_error) => {
                // match on the transfer_call_error?
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::CreateUserCanisterIcpTransferCallError(format!("{:?}", transfer_call_error))));
            }
        };
    
        new_user_data.create_user_canister_block_height = Some(create_user_canister_block_height);
    }


    if new_user_data.users_map_canister_data.unwrap().0.user_canister.is_none() {
    
        let user_canister: Principal = match call<(Result<Principal, CmcNotifyError>,)>(
            MAINNET_CYCLES_MINTING_CANISTER_ID,
            "notify_create_canister",
            (CmcNotifyCreateCanisterQuest {
                controller: id(),
                block_index: new_user_data.create_user_canister_block_height.unwrap()
            },)
        ).await {
            Ok(notify_result) => match notify_result {
                Ok(new_canister_id) => new_canister_id,
                Err(cmc_notify_error) => {
                    // match on the cmc_notify_error, if it failed bc of the cmc icp transfer block height expired, remove the user from the NEW_USERS map.     
                    match cmc_notify_error {
                        CmcNotifyError::TransactionTooOld(block_height) 
                        | CmcNotifyError::InvalidTransaction(string),
                        | CmcNotifyError::Refunded { block_index: optional_refund_block_height, reason: reason_string },
                        => {
                            with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
                            return Err(NewUserError::CreateUserCanisterCmcNotifyError(cmc_notify_error));
                        },
                        | CmcNotifyError::Other{ error_message, error_code }, 
                        | CmcNotifyError::Processing,
                        => {
                            new_user_data.lock = false;
                            write_new_user_data(&user_id, new_user_data);
                            return Err(NewUserError::MidCallError(NewUserMidCallError::CreateUserCanisterCmcNotifyError(cmc_notify_error)));   
                        },
                    }                    
                }
            },
            Err(cmc_notify_call_error) => {
                // match on the call errors?
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::CreateUserCanisterCmcNotifyCallError(format!("{:?}", cmc_notify_call_error))));
            }      
        };
        
        new_user_data.users_map_canister_data.unwrap().0.user_canister = Some(user_canister);
        new_user_data.user_canister_uninstall_code = true; // because a fresh cmc canister is empty 
    }


    if new_user_data.user_canister_uninstall_code == false {
        
        match call<(,)>(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "uninstall_code",
            (CanisterIdRecord { canister_id: new_user_data.users_map_canister_data.unwrap().0.user_canister.unwrap() },),
        ).await {
            Ok(_) => {},
            Err(uninstall_code_call_error) => {
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::UserCanisterUninstallCodeCallError(format!("{:?}", uninstall_code_call_error))));
            }
        }
        
        new_user_data.user_canister_uninstall_code = true;
    }


    if new_user_data.user_canister_install_code == false {
    
        if with(USER_CANISTER_CODE, |ucc| { ucc.is_none() }) {
            new_user_data.lock = false;
            write_new_user_data(&user_id, new_user_data);
            return Err(NewUserError::MidCallError(NewUserMidCallError::UserCanisterCodeNotFound));
        }

        let ucc_module_pointer: *const Vec<u8> = USER_CANISTER_CODE.with(|ucc_refcell| { (*ucc_refcell.borrow()).module() as *const Vec<u8> });

        match call<(,)>(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "install_code",
            (ManagementCanisterInstallCodeQuest {
                mode : ManagementCanisterInstallCodeMode::install,
                canister_id : new_user_data.users_map_canister_data.unwrap().0.user_canister.unwrap(),
                wasm_module : unsafe { &*ucc_module_pointer },
                arg : &encode_one(&UserCanisterInit{  }).unwrap() 
            },),
        ).await {
            Ok(_) => {},
            Err(put_code_call_error) => {
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::UserCanisterInstallCodeCallError(format!("{:?}", put_code_call_error))));
            }
        }
        
        new_user_data.user_canister_install_code = true;
    }
    

    if new_user_data.user_canister_status_record.is_none() {
        
        let canister_status_record: ManagementCanisterCanisterStatusRecord = match call(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "canister_status",
            (CanisterIdRecord { canister_id: new_user_data.users_map_canister_data.unwrap().0.user_canister.unwrap() },),
        ).await {
            Ok((canister_status_record,)) => canister_status_record,
            Err(canister_status_call_error) => {
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::UserCanisterStatusCallError(format!("{:?}", canister_status_call_error))));
            }
        };
        
        new_user_data.user_canister_status_record = Some(canister_status_record);
    }
        
    // no async in this if-block so no NewUserData field needed. can make it for the optimization though
    if new_user_data.user_canister_status_record.unwrap().module_hash.is_none() || new_user_data.user_canister_status_record.unwrap().module_hash.unwrap() != with(&USER_CANISTER_CODE, |ucc| *ucc.module_hash()) {
        // go back a couple of steps
        new_user_data.user_canister_uninstall_code = false;                                  
        new_user_data.user_canister_install_code = false;
        new_user_data.user_canister_status_record = None;
        new_user_data.lock = false;
        write_new_user_data(&user_id, new_user_data);
        return Err(NewUserError::MidCallError(NewUserMidCallError::UserCanisterModuleVerificationError));
    
    }
    

    if new_user_data.user_canister_status_record.unwrap().status != ManagementCanisterCanisterStatusVariant::running {
    
        match call<(,)>(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "start_canister",
            (CanisterIdRecord { canister_id: new_user_data.users_map_canister_data.unwrap().0.user_canister.unwrap() },)
        ).await {
            Ok(_) => {
                new_user_data.user_canister_status_record.unwrap().status = ManagementCanisterCanisterStatusVariant::running; 
            },
            Err(start_canister_call_error) => {
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::UserCanisterStartCanisterCallError(format!("{:?}", start_canister_call_error))));
            }
        }
        
    }
    

    if new_user_data.users_map_canister_write_user_data == false {
        
        match users_map_canister_write_user_data(
            new_user_data.users_map_canister_data.unwrap().1.unwrap(), 
            user_id, 
            new_user_data.users_map_canister_data.unwrap().0
        ).await {
            Ok(_) => {
                new_user_data.users_map_canister_write_user_data = true;
            },
            Err(users_map_canister_write_user_data_error) => {
                let mut call_error_text: String = String::new();
                match users_map_canister_write_user_data_error {
                    UsersMapCanisterWriteUserDataError::UsersMapCanisterCallFail(call_error) => {
                          call_error_text = format!("{:?}", call_error);
                    },
                    UsersMapCanisterWriteUserDataError::UserNotFoundOnThisUsersMapCanister => {
                        //something is wrong if this happens.
                    }
                }
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::UsersMapCanisterWriteUserDataCallError(call_error_text)));          
            }
        }

    }
    
    
    // log this on the user-canister, or the user canister will log this itself in the canister_init
    
    // take the money
    
    // give them back the users-map-canister-id and the user-canister-id
    
    
    if new_user_data.collect_icp == false {
        match take_user_icp_ledger(&user_id, IcpTokens.from_e8s(new_user_data.current_membership_cost_icp().unwrap().e8s() * 0.3)).await {
            Ok(icp_transfer_result) => match icp_transfer_result {
                Ok(_block_height) => {
                    new_user_data.collect_icp = true;
                },
                Err(icp_transfer_error) => {
                    //match icp_transfer_error {
                    //    IcpTransferError::BadFee{expected_fee: icptokens} => {},
                    //_ => {},
                    //}
                    new_user_data.lock = false;
                    write_new_user_data(&user_id, new_user_data);
                    return Err(NewUserError::MidCallError(NewUserMidCallError::IcpTransferError(icp_transfer_error)));          
                }
            }, 
            Err(icp_transfer_call_error) => {
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::IcpTransferCallError(format!("{:?}", icp_transfer_call_error))));          
            }               
        }
    }


    with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
    
    Ok(NewUserSuccessData {
        users_map_canister: new_user_data.users_map_canister_data.unwrap().1.unwrap(),
        user_canister: new_user_data.users_map_canister_data.unwrap().0.user_canister.unwrap()
    })
}








// certification? or replication-calls?
#[export_name = "canister_query see_users_map_canisters"]
pub fn see_users_map_canisters() {
    ic_cdk::api::call::reply::<(&Vec<Principal>,)>((unsafe { localkey_refcell::get(&USERS_MAP_CANISTERS) },))
}

















#[derive(CandidType, Deserialize)]
pub struct UserBalance {
    cycles_balance: u128,
    icp_balance: IcpTokens, 
}

#[derive(CandidType, Deserialize)]
pub enum SeeBalanceError {
    IcpLedgerCheckBalanceCallError(String),
    CheckUserCyclesBalanceError(CheckUserCyclesBalanceError),
}

pub type SeeBalanceSponse = Result<UserBalance, SeeBalanceError>;

#[update]
pub async fn see_balance() -> SeeBalanceSponse {
    let user_id: Principal = caller();
    //check_lock_and_lock_user(&user_id);
    let user_data: UserData = match get_and_lock_user(&user_id).await {
        Ok(ud) => ud,
        Err(get_and_lock_user_error) => {
            unlock_user(&user_id)
            return
        }
    };
    
    let cycles_balance: Cycles = match check_user_cycles_balance(&user_id).await {
        Ok(cycles) => cycles,
        Err(check_user_cycles_balance_error) => {
            unlock_user(&user_id);
            return Err(SeeBalanceError::CheckUserCyclesBalanceError(check_user_cycles_balance_error));
        }
    };
    
    let icp_balance: IcpTokens = match check_user_icp_balance(&user_id).await {
        Ok(tokens) => tokens,
        Err(balance_call_error) => {
            unlock_user(&user_id);
            return Err(SeeBalanceError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error)));
        }
    };
    
    unlock_user(&user_id);
    Ok(UserBalance {
        cycles_balance,
        icp_balance,
    })
}










#[derive(CandidType, Deserialize)]
pub struct IcpPayoutQuest {
    icp: IcpTokens,
    payout_icp_id: IcpId
}

#[derive(CandidType, Deserialize)]
pub struct CyclesPayoutQuest {
    cycles: u128,
    payout_cycles_transfer_canister: Principal         // the memo is: cts-payout    
}

#[derive(CandidType, Deserialize)]
pub enum CollectBalanceQuest {
    icp_payout(IcpPayoutQuest),
    cycles_payout(CyclesPayoutQuest)
}

#[derive(CandidType, Deserialize)]
pub enum IcpPayoutError {
    InvalidIcpPayout0Amount,
    IcpLedgerCheckBalanceCallError(String),
    BalanceTooLow { max_icp_payout: IcpTokens },
    IcpLedgerTransferError(IcpTransferError),
    IcpLedgerTransferCallError(String),


}

#[derive(CandidType, Deserialize)]
pub enum CyclesPayoutError {
    InvalidCyclesPayout0Amount,
    CheckUserCyclesBalanceError(CheckUserCyclesBalanceError),
    BalanceTooLow { max_cycles_payout: u128 },
    CyclesTransferCallCandidEncodeError(String),
    CyclesTransferCallError { call_error: String, paid_fee: bool, cycles_accepted: Cycles }, // fee_paid: u128 ??
}

pub type IcpPayoutSponse = Result<IcpBlockHeight, IcpPayoutError>;

pub type CyclesPayoutSponse = Result<u128, CyclesPayoutError>;

#[derive(CandidType, Deserialize)]
pub enum CollectBalanceSponse {
    icp_payout(IcpPayoutSponse),
    cycles_payout(CyclesPayoutSponse)
}

#[update]
pub async fn collect_balance(collect_balance_quest: CollectBalanceQuest) -> CollectBalanceSponse {
    let user: Principal = caller();

    check_lock_and_lock_user(&user);

    match collect_balance_quest {

        CollectBalanceQuest::icp_payout(icp_payout_quest) => {
            
            if icp_payout_quest.icp == IcpTokens::ZERO {
                unlock_user(&user);
                return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::InvalidIcpPayout0Amount));
            }
            
            let user_icp_balance: IcpTokens = match check_user_icp_balance(&user).await {
                Ok(icp_tokens) => icp_tokens,
                Err(balance_call_error) => {
                    unlock_user(&user);
                    return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error))));
                }
            };
            
            if icp_payout_quest.icp + ICP_PAYOUT_FEE + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) > user_icp_balance {
                unlock_user(&user);
                return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::BalanceTooLow { max_icp_payout: user_icp_balance - ICP_PAYOUT_FEE - IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) }));
            }
            
            let icp_payout_transfer_call: CallResult<IcpTransferResult> = icp_transfer(
                MAINNET_LEDGER_CANISTER_ID,
                IcpTransferArgs {
                    memo: ICP_PAYOUT_MEMO,
                    amount: icp_payout_quest.icp,
                    fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                    from_subaccount: Some(principal_icp_subaccount(&user)),
                    to: icp_payout_quest.payout_icp_id,
                    created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
                }
            ).await; 
            let icp_payout_transfer_call_block_index: IcpBlockHeight = match icp_payout_transfer_call {
                Ok(transfer_result) => match transfer_result {
                    Ok(block_index) => block_index,
                    Err(transfer_error) => {
                        unlock_user(&user);
                        return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferError(transfer_error)));
                    }
                },
                Err(transfer_call_error) => {
                    unlock_user(&user);
                    return CollectBalanceSponse::icp_payout(Err(IcpPayoutError::IcpLedgerTransferCallError(format!("{:?}", transfer_call_error))));
                }
            };

            let icp_payout_take_fee_transfer_call: CallResult<IcpTransferResult> = icp_transfer(
                MAINNET_LEDGER_CANISTER_ID,
                IcpTransferArgs {
                    memo: ICP_TAKE_PAYOUT_FEE_MEMO,
                    amount: ICP_PAYOUT_FEE,
                    fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE,
                    from_subaccount: Some(principal_icp_subaccount(&user)),
                    to: main_cts_icp_id(),                        
                    created_at_time: Some(IcpTimestamp { timestamp_nanos: time() })
                }
            ).await;             
            match icp_payout_take_fee_transfer_call {
                Ok(transfer_result) => match transfer_result {
                    Ok(_block_index) => {},
                    Err(_transfer_error) => {
                        USERS_DATA.with(|ud| {
                            ud.borrow_mut().get_mut(&user).unwrap().untaken_icp_to_collect += ICP_PAYOUT_FEE + ICP_LEDGER_TRANSFER_DEFAULT_FEE;
                        });
                    }  // log and take into the count 
                },
                Err(_transfer_call_error) => { // log and take into the count
                    USERS_DATA.with(|ud| {
                        ud.borrow_mut().get_mut(&user).unwrap().untaken_icp_to_collect += ICP_PAYOUT_FEE + ICP_LEDGER_TRANSFER_DEFAULT_FEE;
                    });
                }
            }
            unlock_user(&user);
            return CollectBalanceSponse::icp_payout(Ok(icp_payout_transfer_call_block_index));
        },



        CollectBalanceQuest::cycles_payout(cycles_payout_quest) => {

            if cycles_payout_quest.cycles == 0 {
                unlock_user(&user);
                return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::InvalidCyclesPayout0Amount));
            }

            let user_cycles_balance: u128 = match check_user_cycles_balance(&user).await {
                Ok(cycles) => cycles,
                Err(check_user_cycles_balance_error) => {
                    unlock_user(&user);           
                    return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::CheckUserCyclesBalanceError(check_user_cycles_balance_error)));
                } 
            };

            if cycles_payout_quest.cycles + CYCLES_TRANSFER_FEE > user_cycles_balance {
                unlock_user(&user);
                return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::BalanceTooLow { max_cycles_payout: user_cycles_balance - CYCLES_TRANSFER_FEE }));
            }
            
            // change!! take the user-cycles before the transfer, and refund in the callback 
            with_mut(&USERS_DATA, |users_data| {
                users_data.get_mut(&user).unwrap().cycles_balance -= cycles_payout_quest.cycles + CYCLES_TRANSFER_FEE;
            });

            let cycles_transfer_call_candid_bytes: Vec<u8> = match encode_one(&CyclesTransfer { memo: CyclesTransferMemo::Text("CTS-POUT".to_string()) }) {
                Ok(candid_bytes) => candid_bytes,
                Err(candid_error) => {
                    unlock_user(&user);
                    return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::CyclesTransferCallCandidEncodeError(format!("{}", candid_error))));
                }
            }; 
            
            let cycles_transfer_call: CallResult<Vec<u8>> = call_raw128(
                cycles_payout_quest.payout_cycles_transfer_canister,
                "cycles_transfer",
                &cycles_transfer_call_candid_bytes,
                cycles_payout_quest.cycles
            ).await;
            
            // check if it is possible for the canister to reject/trap but still keep the cycles. if yes, [re]turn the cycles_accepted in the error. for now, going as if not possible.
            // do some tests on a canister_error and on a canister_reject
            // test see if msg_cycles_refunded128() gives back the full cycles mount on a CanisterError. what about on a CanisterReject?

            unlock_user(&user);

            let cycles_accepted: u128 = cycles_payout_quest.cycles - msg_cycles_refunded128(); 
            
            USERS_DATA.with(|ud| { ud.borrow_mut().get_mut(&user).unwrap().cycles_balance += msg_cycles_refunded128(); });        
            
            match cycles_transfer_call {
                Ok(_) => {
                    return CollectBalanceSponse::cycles_payout(Ok(cycles_accepted));
                },
                Err(cycles_transfer_call_error) => {
                    let paid_fee: bool = match cycles_transfer_call_error.0 {
                        RejectionCode::DestinationInvalid | RejectionCode::CanisterReject | RejectionCode::CanisterError => {
                            true
                        },
                        _ => {
                            USERS_DATA.with(|ud| { ud.borrow_mut().get_mut(&user).unwrap().cycles_balance += CYCLES_TRANSFER_FEE; });
                            false
                        }
                    };
                    return CollectBalanceSponse::cycles_payout(Err(CyclesPayoutError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: paid_fee, cycles_accepted: cycles_accepted }));
                }
            }


            
        }
    }
}









#[derive(CandidType, Deserialize)]
pub struct ConvertIcpBalanceForTheCyclesWithTheCmcRateQuest {
    icp: IcpTokens
}

#[derive(CandidType, Deserialize)]
pub enum ConvertIcpBalanceForTheCyclesWithTheCmcRateError {
    CmcGetRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    IcpLedgerCheckBalanceCallError(String),
    IcpBalanceTooLow { max_icp_convert_for_the_cycles: IcpTokens },
    LedgerTopupCyclesError(LedgerTopupCyclesError),
}




// ledger takes the fee twice out of the users icp subaccount balance
// now with the new cmc-notify method ledger takes only once fee

// :flat-fee: 10369909-cycles? 20_000_000-cycles - 1/500 of a penny of an xdr

#[update]
pub async fn convert_icp_balance_for_the_cycles_with_the_cmc_rate(q: ConvertIcpBalanceForTheCyclesWithTheCmcRateQuest) -> Result<Cycles, ConvertIcpBalanceForTheCyclesWithTheCmcRateError> {    
    
    let user: Principal = caller();

    // check minimum-conversion [a]mount


    // let xdr_permyriad_per_icp: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate().await {
    //     Ok(rate) => rate,
    //     Err(check_current_rate_error) => {
    //         return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::CmcGetRateError(check_current_rate_error));
    //     }
    // };
    // let cycles: u128 = icptokens_to_cycles(q.icp, xdr_permyriad_per_icp);

    check_lock_and_lock_user(&user);

    let user_icp_balance: IcpTokens = match check_user_icp_balance(&user).await {
        Ok(icp_tokens) => icp_tokens,
        Err(balance_call_error) => {
            unlock_user(&user);
            return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error)));
        }
    };

    if q.icp + IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) > user_icp_balance {
        unlock_user(&user);
        return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::IcpBalanceTooLow { max_icp_convert_for_the_cycles: user_icp_balance - IcpTokens::from_e8s(ICP_LEDGER_TRANSFER_DEFAULT_FEE.e8s() * 2) });
    }

    let topup_cycles: Cycles = match ledger_topup_cycles(q.icp, Some(principal_icp_subaccount(&user)), ic_cdk::api::id()).await {
        Ok(cycles) => cycles,
        Err(ledger_topup_cycles_error) => {
            unlock_user(&user);
            return Err(ConvertIcpBalanceForTheCyclesWithTheCmcRateError::LedgerTopupCyclesError(ledger_topup_cycles_error));
        }
    };

    USERS_DATA.with(|ud| {
        ud.borrow_mut().get_mut(&user).unwrap().cycles_balance += topup_cycles;
    });

    unlock_user(&user);

    Ok(topup_cycles)
}











#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesTransferQuest {
    canister: Principal,
    cycles: u128,
    cycles_transfer: CyclesTransfer,
    // public: bool,
}

#[derive(CandidType, Deserialize)]
pub enum PurchaseCyclesTransferError {
    InvalidCyclesTransfer0Amount,
    CheckUserCyclesBalanceError(CheckUserCyclesBalanceError),
    BalanceTooLow { max_cycles_for_the_transfer: u128 },
    CyclesTransferCallCandidEncodeError(String),
    CyclesTransferCallError { call_error: String, paid_fee: bool }, // fee_paid: u128 ??

}

#[update]
pub async fn purchase_cycles_transfer(pctq: PurchaseCyclesTransferQuest) -> Result<CyclesTransferPurchaseLog, PurchaseCyclesTransferError> {
    let user: Principal = caller();
    
    if pctq.cycles == 0 {
        return Err(PurchaseCyclesTransferError::InvalidCyclesTransfer0Amount);
    }

    check_lock_and_lock_user(&user);

    let user_cycles_balance: u128 = match check_user_cycles_balance(&user).await {
        Ok(cycles) => cycles,
        Err(check_user_cycles_balance_error) => {
            unlock_user(&user);
            return Err(PurchaseCyclesTransferError::CheckUserCyclesBalanceError(check_user_cycles_balance_error));
        }
    };

    if user_cycles_balance < pctq.cycles + CYCLES_TRANSFER_FEE {
        unlock_user(&user);
        return Err(PurchaseCyclesTransferError::BalanceTooLow { max_cycles_for_the_transfer: user_cycles_balance - CYCLES_TRANSFER_FEE });
    }

    // change!! take the user-cycles before the transfer, and refund in the callback 

    let cycles_transfer_candid_bytes: Vec<u8> = match encode_one(&pctq.cycles_transfer) {
        Ok(candid_bytes) => candid_bytes,
        Err(candid_error) => {
            unlock_user(&user);
            return Err(PurchaseCyclesTransferError::CyclesTransferCallCandidEncodeError(format!("{}", candid_error)));
        }
    };

    let cycles_transfer_call: CallResult<Vec<u8>> = call_raw128(
        pctq.canister,
        "cycles_transfer",
        &cycles_transfer_candid_bytes,
        pctq.cycles
    ).await;

    unlock_user(&user);

    match cycles_transfer_call {
        Ok(_) => {

            let cycles_accepted: u128 = pctq.cycles - msg_cycles_refunded128();
            
            let cycles_transfer_purchase_log = CyclesTransferPurchaseLog {
                canister: pctq.canister,
                cycles_sent: pctq.cycles,
                cycles_accepted: cycles_accepted,
                cycles_transfer: pctq.cycles_transfer,
                timestamp: time(),
            };

            USERS_DATA.with(|ud| {
                let users_data: &mut HashMap<Principal, UserData> = &mut ud.borrow_mut();
                let user_data: &mut UserData = &mut users_data.get_mut(&user).unwrap();

                user_data.cycles_balance -= cycles_accepted + CYCLES_TRANSFER_FEE;

                user_data.cycles_transfer_purchases.push(cycles_transfer_purchase_log.clone());
            });

            return Ok(cycles_transfer_purchase_log);
        },
        Err(cycles_transfer_call_error) => {
            match cycles_transfer_call_error.0 {
                RejectionCode::DestinationInvalid | RejectionCode::CanisterReject | RejectionCode::CanisterError => {
                    USERS_DATA.with(|ud| { ud.borrow_mut().get_mut(&user).unwrap().cycles_balance -= CYCLES_TRANSFER_FEE; });
                    return Err(PurchaseCyclesTransferError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: true });
                },
                _ => {
                    return Err(PurchaseCyclesTransferError::CyclesTransferCallError{ call_error: format!("{:?}", cycles_transfer_call_error), paid_fee: false });
                }
            }
        }
    }
}



#[derive(CandidType, Deserialize)]
pub enum CyclesPaymentOrIcpPayment {
    cycles_payment,
    icp_payment
}

#[derive(CandidType, Deserialize)]
pub struct PurchaseCyclesBankQuest {
    cycles_payment_or_icp_payment: CyclesPaymentOrIcpPayment,
}

#[derive(CandidType, Deserialize)]
pub enum PurchaseCyclesBankError {
    CheckUserCyclesBalanceError(CheckUserCyclesBalanceError),
    CyclesBalanceTooLow { current_user_cycles_balance: u128, current_cycles_bank_cost_cycles: u128 },
    IcpCheckBalanceCallError(String),
    CmcGetRateError(CheckCurrentXdrPerMyriadPerIcpCmcRateError),
    IcpBalanceTooLow { current_user_icp_balance: IcpTokens, current_cycles_bank_cost_icp: IcpTokens, current_icp_payment_ledger_transfer_fee: IcpTokens },
    CreateCyclesBankCanisterError(GetNewCanisterError),
    UninstallCodeCallError(String),
    NoCyclesBankCode,
    PutCodeCallError(String),
    CanisterStatusCallError(String),
    CheckModuleHashError{canister_status_record_module_hash: Option<[u8; 32]>, cbc_module_hash: [u8; 32]},
    StartCanisterCallError(String),
    PutCyclesCallError(String),
    UpdateSettingsCallError(String),


}


#[derive(CandidType, Deserialize)]
pub struct ManagementCanisterInstallCodeQuest<'a> {
    mode : ManagementCanisterInstallCodeMode,
    canister_id : Principal,
    wasm_module : &'a [u8],
    arg : &'a [u8],
}

#[derive(CandidType, Deserialize)]
pub enum ManagementCanisterInstallCodeMode {
    install, 
    reinstall, 
    upgrade
}


#[update]
pub async fn purchase_cycles_bank(q: PurchaseCyclesBankQuest) -> Result<CyclesBankPurchaseLog, PurchaseCyclesBankError> {
    let user: Principal = caller();
    check_lock_and_lock_user(&user);

    let mut cycles_bank_cost_icp: Option<IcpTokens> = None;

    match q.cycles_payment_or_icp_payment {
        
        CyclesPaymentOrIcpPayment::cycles_payment => {
            
            let user_cycles_balance: u128 = match check_user_cycles_balance(&user).await {
                Ok(cycles) => cycles,
                Err(check_user_cycles_balance_error) => {
                    unlock_user(&user);
                    return Err(PurchaseCyclesBankError::CheckUserCyclesBalanceError(check_user_cycles_balance_error));
                }
            };
            
            if user_cycles_balance < CYCLES_BANK_COST {
                unlock_user(&user);
                return Err(PurchaseCyclesBankError::CyclesBalanceTooLow{ current_user_cycles_balance: user_cycles_balance, current_cycles_bank_cost_cycles: CYCLES_BANK_COST });
            }
        },
        
        CyclesPaymentOrIcpPayment::icp_payment => {
            
            let user_icp_balance: IcpTokens = match check_user_icp_balance(&user).await {
                Ok(icp_tokens) => icp_tokens,
                Err(balance_call_error) => {
                    unlock_user(&user);
                    return Err(PurchaseCyclesBankError::IcpCheckBalanceCallError(format!("{:?}", balance_call_error)));
                }
            };
            let xdr_permyriad_per_icp: u64 = match check_current_xdr_permyriad_per_icp_cmc_rate().await {
                Ok(rate) => rate,
                Err(check_current_rate_error) => {
                    unlock_user(&user);
                    return Err(PurchaseCyclesBankError::CmcGetRateError(check_current_rate_error));    
                }
            };
            cycles_bank_cost_icp = Some(cycles_to_icptokens(CYCLES_BANK_COST, xdr_permyriad_per_icp));
            if user_icp_balance < cycles_bank_cost_icp.unwrap() + ICP_LEDGER_TRANSFER_DEFAULT_FEE { // ledger fee for the icp-transfer from user subaccount to cts main
                unlock_user(&user);
                return Err(PurchaseCyclesBankError::IcpBalanceTooLow{ 
                    current_user_icp_balance: user_icp_balance, 
                    current_cycles_bank_cost_icp: cycles_bank_cost_icp.unwrap(), 
                    current_icp_payment_ledger_transfer_fee: ICP_LEDGER_TRANSFER_DEFAULT_FEE 
                });
            }
        }
    }

    // change to a create with the ledger_canister
    let cycles_bank_principal: Principal = match get_new_canister().await {
        Ok(p) => p,
        Err(e) => {
            unlock_user(&user);
            return Err(PurchaseCyclesBankError::CreateCyclesBankCanisterError(e));
        }
    };
            
    // on errors after here make sure to put the cycles-bank-canister into the NEW_CANISTERS list 

    // install code

    let uninstall_code_call: CallResult<()> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "uninstall_code",
        (CanisterIdRecord { canister_id: cycles_bank_principal },),
    ).await; 
    match uninstall_code_call {
        Ok(_) => {},
        Err(uninstall_code_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::UninstallCodeCallError(format!("{:?}", uninstall_code_call_error)));
        }
    }
    
    if CYCLES_BANK_CANISTER_CODE.with(|cbc_refcell| { (*cbc_refcell.borrow()).module().len() == 0 }) {
        unlock_user(&user);
        NEW_CANISTERS.with(|ncs| {
            ncs.borrow_mut().push(cycles_bank_principal);
        });
        return Err(PurchaseCyclesBankError::NoCyclesBankCode);
    }

    let cbc_module_pointer: *const Vec<u8> = CYCLES_BANK_CODE.with(|cbc_refcell| {
        (*cbc_refcell.borrow()).module() as *const Vec<u8>
    });

    let put_code_call: CallResult<()> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "install_code",
        (ManagementCanisterInstallCodeQuest {
            mode : ManagementCanisterInstallCodeMode::install,
            canister_id : cycles_bank_principal,
            wasm_module : unsafe { &*cbc_module_pointer },
            arg : &encode_one(vec![ic_cdk::api::id()]).unwrap() // for now the cycles-bank takes controllers in the init
        },),
    ).await;   
    match put_code_call {
        Ok(_) => {},
        Err(put_code_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::PutCodeCallError(format!("{:?}", put_code_call_error)));
        }
    }

    // check canister status
    let canister_status_call: CallResult<(ManagementCanisterCanisterStatusRecord,)> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "canister_status",
        (CanisterIdRecord { canister_id: cycles_bank_principal },),
    ).await;
    let canister_status_record: ManagementCanisterCanisterStatusRecord = match canister_status_call {
        Ok((canister_status_record,)) => canister_status_record,
        Err(canister_status_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::CanisterStatusCallError(format!("{:?}", canister_status_call_error)));
        }
    };

    // check the wasm hash of the canister
    if canister_status_record.module_hash.is_none() || canister_status_record.module_hash.unwrap() != with(&CYCLES_BANK_CODE, |cbc| *cbc.module_hash()) {
        unlock_user(&user);
        with_mut(&NEW_CANISTERS, |ncs| {
            ncs.push(cycles_bank_principal);
        });
        return Err(PurchaseCyclesBankError::CheckModuleHashError{canister_status_record_module_hash: canister_status_record.module_hash, cbc_module_hash: CYCLES_BANK_CODE.with(|cbc_refcell| { *(*cbc_refcell.borrow()).module_hash() }) });
    }

    // check the running status
    if canister_status_record.status != ManagementCanisterCanisterStatusVariant::running {

        // start canister
        let start_canister_call: CallResult<()> = call(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "start_canister",
            (CanisterIdRecord { canister_id: cycles_bank_principal },),
        ).await;
        match start_canister_call {
            Ok(_) => {},
            Err(start_canister_call_error) => {
                unlock_user(&user);
                NEW_CANISTERS.with(|ncs| {
                    ncs.borrow_mut().push(cycles_bank_principal);
                });
                return Err(PurchaseCyclesBankError::StartCanisterCallError(format!("{:?}", start_canister_call_error)));
            }
        }
    }

    if canister_status_record.cycles < 500_000_000_000 {
        // put some cycles
        let put_cycles_call: CallResult<()> = call_with_payment128(
            MANAGEMENT_CANISTER_PRINCIPAL,
            "deposit_cycles",
            (CanisterIdRecord { canister_id: cycles_bank_principal },),
            500_000_000_000 - canister_status_record.cycles
        ).await;
        match put_cycles_call {
            Ok(_) => {},
            Err(put_cycles_call_error) => {
                unlock_user(&user);
                NEW_CANISTERS.with(|ncs| {
                    ncs.borrow_mut().push(cycles_bank_principal);
                });
                return Err(PurchaseCyclesBankError::PutCyclesCallError(format!("{:?}", put_cycles_call_error)));
            }
        }
    }

    // change canister controllers
    let update_settings_call: CallResult<()> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "update_settings",
        (ChangeCanisterSettingsRecord { 
            canister_id: cycles_bank_principal,
            settings: ManagementCanisterOptionalCanisterSettings {
                controllers: Some(vec![user, cycles_bank_principal]),
                compute_allocation : None,
                memory_allocation : None,
                freezing_threshold : None
            }
        },),
    ).await;
    match update_settings_call {
        Ok(_) => {},
        Err(update_settings_call_error) => {
            unlock_user(&user);
            NEW_CANISTERS.with(|ncs| {
                ncs.borrow_mut().push(cycles_bank_principal);
            });
            return Err(PurchaseCyclesBankError::UpdateSettingsCallError(format!("{:?}", update_settings_call_error)));
        }
    }

    // // sync_controllers-method on the cycles-bank
    // // let the user call with the frontend to sync the controllers?
    // let sync_controllers_call: CallResult<Vec<Principal>> = call(
    //     cycles_bank_principal,
    //     "sync_controllers",
    //     (,),
    // ).await;
    // match sync_controllers_call {
    //     Ok(synced_controllers) => {},
    //     Err(sync_controllers_call_error) => {

    //     }
    // }

    // make the cycles-bank-purchase-log
    let cycles_bank_purchase_log = CyclesBankPurchaseLog {
        cycles_bank_principal,
        cost_cycles: CYCLES_BANK_COST,
        timestamp: time(),
    };

    // log the cycles-bank-purchase-log within the USERS_DATA.with-closure and collect the icp or cycles cost within the USERS_DATA.with-closure
    with_mut(&USERS_DATA, |users_data| {
        let user_data: &mut UserData = users_data.get_mut(&user).unwrap();

        user_data.cycles_bank_purchases.push(cycles_bank_purchase_log);
        
        match q.cycles_payment_or_icp_payment {   
            CyclesPaymentOrIcpPayment::cycles_payment => {
                user_data.cycles_balance -= CYCLES_BANK_COST;
            },
            CyclesPaymentOrIcpPayment::icp_payment => {
                user_data.untaken_icp_to_collect += cycles_bank_cost_icp.unwrap() + ICP_LEDGER_TRANSFER_DEFAULT_FEE;
            }
        }
    });

    unlock_user(&user);
    Ok(cycles_bank_purchase_log)
}








#[derive(CandidType, Deserialize)]
pub struct Fees {
    purchase_cycles_bank_cost_cycles: u128,
    purchase_cycles_bank_upgrade_cost_cycles: u128,
    purchase_cycles_transfer_cost_cycles: u128
}

#[query]
pub fn see_fees() -> Fees {
    Fees {
        purchase_cycles_bank_cost_cycles: CYCLES_BANK_COST,
        purchase_cycles_bank_upgrade_cost_cycles: CYCLES_BANK_UPGRADE_COST,
        purchase_cycles_transfer_cost_cycles: CYCLES_TRANSFER_FEE, 
    }
}






// test this!
#[no_mangle]
pub fn canister_inspect_message() {
    // caution: this function is only called for ingress messages 
    
    if caller() == Principal::anonymous() && ![""].contains(&&ic_cdk::api::call::method_name()[..] {
        trap("caller cannot be anonymous for this method.")
    }

    if &ic_cdk::api::call::method_name()[..] == "cycles_transfer" {
        trap("caller must be a canister for this method.")
    }


    ic_cdk::api::call::accept_message();
}


#[init]
fn init() {

} 

#[pre_upgrade]
fn pre_upgrade() {
    USERS_DATA.with(|ud| {
        save_users_data(&*ud.borrow());
    });
    NEW_CANISTERS.with(|ncs| {
        save_new_canisters(&*ncs.borrow());
    });
}

#[post_upgrade]
fn post_upgrade() {
    USERS_DATA.with(|ud| {
        *ud.borrow_mut() = read_users_data();
    });
    NEW_CANISTERS.with(|ncs| {
        *ncs.borrow_mut() = read_new_canisters();
    });

    
} 





#[update]
pub async fn controller_see_balance() -> SeeBalanceSponse {
    let cycles_balance: u128 = ic_cdk::api::canister_balance128();
    let icp_balance: IcpTokens = match icp_account_balance(
        MAINNET_LEDGER_CANISTER_ID,
        IcpAccountBalanceArgs {
            account : main_cts_icp_id()
        }
    ).await {
        Ok(tokens) => tokens,
        Err(balance_call_error) => {
            return Err(SeeBalanceError::IcpLedgerCheckBalanceCallError(format!("{:?}", balance_call_error)));
        } 
    };
    Ok(UserBalance {
        cycles_balance,
        icp_balance,
    })
}


#[update]
pub fn controller_put_new_canisters(mut new_canisters: Vec<Principal>) {
    NEW_CANISTERS.with(|ncs| {
        ncs.borrow_mut().append(&mut new_canisters); // .extend_from_slice(&new_canisters) also works but it clones each item. .append moves each item
    });
}

#[export_name = "canister_update controller_see_new_canisters"]
pub fn controller_see_new_canisters<'a>() -> () {
    let ncs_pointer = NEW_CANISTERS.with(|ncs| {
        (&(*ncs.borrow())) as *const Vec<Principal> 
    });

    ic_cdk::api::call::reply::<(&'a Vec<Principal>,)>((unsafe { &*ncs_pointer },));

}



#[derive(CandidType, Deserialize)]
pub enum ControllerSeeNewCanisterStatusError {
    CanisterStatusCallError(String)
}

#[update]
pub async fn controller_see_new_canister_status(new_canister: Principal) -> Result<ManagementCanisterCanisterStatusRecord, ControllerSeeNewCanisterStatusError> {
    let canister_status_call: CallResult<(ManagementCanisterCanisterStatusRecord,)> = call(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "canister_status",
        (CanisterIdRecord { canister_id: new_canister },),
    ).await;
    let canister_status_record: ManagementCanisterCanisterStatusRecord = match canister_status_call {
        Ok((canister_status_record,)) => canister_status_record,
        Err(canister_status_call_error) => return Err(ControllerSeeNewCanisterStatusError::CanisterStatusCallError(format!("{:?}", canister_status_call_error)))
    };

    Ok(canister_status_record)

}


#[update]
pub fn controller_put_cbc(module: Vec<u8>) -> () {
    with_mut(&CYCLES_BANK_CODE, |cbc| {
        cbc.change_module(module);
    });
}


#[derive(CandidType, Deserialize)]
pub enum ControllerDepositCyclesError {
    DepositCyclesCallError(String)
}


#[update]
pub async fn controller_deposit_cycles(cycles: Cycles, topup_canister: Principal) -> Result<(), ControllerDepositCyclesError> {
    let put_cycles_call: CallResult<()> = call_with_payment128(
        MANAGEMENT_CANISTER_PRINCIPAL,
        "deposit_cycles",
        (CanisterIdRecord { canister_id: topup_canister },),
        cycles
    ).await;
    match put_cycles_call {
        Ok(_) => Ok(()),
        Err(put_cycles_call_error) => return Err(ControllerDepositCyclesError::DepositCyclesCallError(format!("{:?}", put_cycles_call_error)))
    }
}


#[derive(CandidType, Deserialize)]
pub struct Metrics {
    global_allocator_counter: usize,
    stable_size: u64,
    cycles_balance: u128,
    new_canisters_count: usize,
    cbc_hash: [u8; 32],
    users_count: usize,
    latest_known_cmc_rate: LatestKnownCmcRate,
    users_map_canisters_count: usize,

    


}


#[query]
pub fn controller_see_metrics() -> Metrics {
    Metrics {
        global_allocator_counter: get_allocated_bytes_count(),
        stable_size: ic_cdk::api::stable::stable64_size(),
        cycles_balance: ic_cdk::api::canister_balance128(),
        new_canisters_count: with(&NEW_CANISTERS, |nc| nc.len()),
        cbc_hash: with(&CYCLES_BANK_CODE, |cbc| *cbc.module_hash()),
        users_count: with(&USERS_DATA, |ud| ud.len() ),
        latest_known_cmc_rate: LATEST_KNOWN_CMC_RATE.with(|cr| cr.get()),
        users_map_canisters_count: with(&USERS_MAP_CANISTERS, |umc| umc.len()),





    }
}





