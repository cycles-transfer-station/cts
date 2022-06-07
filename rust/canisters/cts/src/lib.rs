
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

// I think using the cycles_transferrer canister is a good way to do it.

// choice for users to download a signed data canister-signature of past trassactions. 
// choice for users to delete past transactions to re-claim storage-space  
// if a user requested cycles-transfer call takes-more than half an hour to come back, the user is not refunded any cycles the callee does'nt take

// user does the main operations through the user_canister.
// the user-lock is on the user-canister

// each method is a contract


// icp transfers , 0.10-xdr / 14-cents flat fee


// tegrate with the icscan.io


// 10 years save the user's-cycles-balance and icp-balance if the user-canister finishes.  










//#![allow(unused)] 
#![allow(non_camel_case_types)]

use std::{cell::{Cell, RefCell, RefMut}, collections::HashMap};

use cts_lib::{
    types::{
        Cycles,
        CyclesTransfer,
        CyclesTransferMemo,
        UserId,
        UserCanisterId,
        UsersMapCanisterId,
        management_canister::{
            ManagementCanisterInstallCodeMode,
            // ManagementCanisterCanisterSettings,
            ManagementCanisterOptionalCanisterSettings,
            ManagementCanisterCanisterStatusRecord,
            ManagementCanisterCanisterStatusVariant,
            CanisterIdRecord,
            ChangeCanisterSettingsRecord,
            
        },
        user_canister::{
            UserCanisterInit,
            CyclesTransferIntoUser,
        }
    },
    consts::{
        MANAGEMENT_CANISTER_ID,
    },
    fees::{
        CYCLES_BANK_COST,
        CYCLES_BANK_UPGRADE_COST,
        CYCLES_TRANSFER_FEE,
        CONVERT_ICP_FOR_THE_CYCLES_WITH_THE_CMC_RATE_FEE,
        
        
        
    },
    tools::{
        sha256,
        localkey_refcell::{
            self,
            with, 
            with_mut,
        },
        thirty_bytes_as_principal
    },
    ic_cdk::{
        self,
        api::{
            trap,
            caller, 
            time,
            id,
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
    },
    ic_cdk_macros::{
        update, 
        query, 
        init, 
        pre_upgrade, 
        post_upgrade
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
    },
    global_allocator_counter::get_allocated_bytes_count,
};


#[cfg(test)]
mod t;

mod tools;
use tools::{
    user_cycles_balance_topup_memo_bytes,
    check_user_icp_ledger_balance,
    main_cts_icp_id,
    CheckCurrentXdrPerMyriadPerIcpCmcRateError,
    CheckCurrentXdrPerMyriadPerIcpCmcRateSponse,
    check_current_xdr_permyriad_per_icp_cmc_rate,
    // icptokens_to_cycles,
    cycles_to_icptokens,
    get_new_canister,
    GetNewCanisterError,
    USER_CYCLES_BALANCE_TOPUP_MEMO_START,
    ledger_topup_cycles,
    LedgerTopupCyclesError,
    IcpXdrConversionRate,
    take_user_icp_ledger,
    ICP_LEDGER_CREATE_CANISTER_MEMO,
    CmcNotifyError,
    CmcNotifyCreateCanisterQuest,
    PutNewUserIntoAUsersMapCanisterError,
    put_new_user_into_a_users_map_canister,
    canister_code::CanisterCode,
    FindUserInTheUsersMapCanistersError,
    find_user_in_the_users_map_canisters
    




    
    
};

/*
mod stable;
use stable::{
    save_users_data,
    read_users_data,
    save_new_canisters,
    read_new_canisters

};
*/







pub const MINIMUM_CYCLES_TRANSFER_INTO_USER: Cycles = 50_000_000_000; // enough to pay for a find_and_lock_user-call.
pub const CYCLES_TRANSFER_INTO_USER_USER_NOT_FOUND_FEE: Cycles = (100_000 + 260_000 + 590_000) * with(&USERS_MAP_CANISTERS, |umcs| umcs.len()); // :do: clude wasm-instructions-counts
pub const CYCLES_PER_USER_PER_103_MiB_PER_YEAR: Cycles = 5_000_000_000_000;





pub const MAX_NEW_USERS: usize = 5000; // the max number of entries in the NEW_USERS-hashmap at the same-time
pub const MAX_USERS_MAP_CANISTERS: usize = 4; // can be 30-million at 1-gb, or 3-million at 0.1-gb,




thread_local! {

    pub static NEW_USERS: RefCell<HashMap<Principal, NewUserData>> = RefCell::new(Vec::new());
    pub static USERS_MAP_CANISTERS: RefCell<Vec<Principal>> = RefCell::new(vec![ic_cdk::api::id()]);
    pub static CREATE_NEW_USERS_MAP_CANISTER_LOCK: Cell<bool> = Cell::new(false);
    pub static LATEST_KNOWN_CMC_RATE: Cell<IcpXdrConversionRate> = Cell::new(IcpXdrConversionRate{ xdr_permyriad_per_icp: 0, timestamp_seconds: 0 });
    
    pub static CYCLES_BANK_CANISTER_CODE    : RefCell<Option<CanisterCode>> = RefCell::new(None);
    pub static USER_CANISTER_CODE           : RefCell<Option<CanisterCode>> = RefCell::new(None);
    pub static USERS_MAP_CANISTER_CODE      : RefCell<Option<CanisterCode>> = RefCell::new(None);
    
    pub static NEW_CANISTERS: RefCell<Vec<Principal>> = RefCell::new(Vec::new());
    
}






// if a user for the topup is not found, the cycles-transfer-station takes a fee for the user-lookup(:fee is with the base on how many users_map_canisters there are) and refunds the rest of the cycles. 
// make sure the minimum-in-cycles-transfer is more than the find_and_plus_user_cycles_balance_user_not_found_fee


#[export_name="canister_update cycles_transfer"]
pub async fn cycles_transfer() {
    
    let cycles_available: Cycles = msg_cycles_available128();
    
    if cycles_available < MINIMUM_CYCLES_TRANSFER_INTO_USER {
        trap(&format!("minimum cycles transfer into a user: {}", MINIMUM_CYCLES_TRANSFER_INTO_USER))
    }

    if arg_data_raw_size() > 100 {
        trap("arg_data_raw_size can be max 100 bytes")
    }
    
    let (ct,): (CyclesTransfer,) = arg_data::<(CyclesTransfer,)>();

    let user_id: UserId = match ct.memo {
        CyclesTransferMemo::Blob(memo_bytes) => {
            if memo_bytes.len() != 32 || &memo_bytes[..2] != USER_CYCLES_BALANCE_TOPUP_MEMO_START {
                trap("unknown cycles transfer memo")
            }
            thirty_bytes_as_principal(&memo_bytes[2..32].try_into().unwrap())
        },
        _ => trap("CyclesTransferMemo must be the Blob variant")
    };
    
    let original_caller: Principal = caller(); // before the first await
    let timestamp_nanos: u64 = time(); // before the first await

    let user_canister_id: UserCanisterId = match find_user_in_the_users_map_canisters(user_id).await {
        Ok((user_canister_id, users_map_canister_id)) => user_canister_id,
        Err(find_user_in_the_users_map_canisters_error) => match find_user_in_the_users_map_canisters_error {
            FindUserInTheUsersMapCanistersError::UserNotFound => {
                msg_cycles_accept128(CYCLES_TRANSFER_INTO_USER_USER_NOT_FOUND_FEE); // test that the cycles are taken on the reject.
                reject(&format!("User for the top up not found. {} cycles taken for a nonexistentuserfee", CYCLES_TRANSFER_INTO_USER_USER_NOT_FOUND_FEE));
                return;
            },
            FindUserInTheUsersMapCanistersError::UsersMapCanisterFindUserCallFail(umc_id, call_error) => {
                reject(&format!("User lookup error. umc_id: {}, umc_call_error: {:?}", umc_id, call_error)); // reject not trap because we are after an await here
                return;
            }
        }
    };
    
    // take a fee for the cycles_transfer_into_user? 
    
    match call::<()>(
        user_canister_id,
        "cts_cycles_transfer_into_user",
        (CyclesTransferIntoUser{ 
            canister: original_caller,
            cycles: cycles_available,
            timestamp_nanos: timestamp_nanos
        },),
    ).await {
        Ok(()) => {
            msg_cycles_accept128(cycles_available);
            reply::<()>(());
            return;
        },
        Err(call_error) => {
            reject(&format!("user-canister call-error. user_canister: {}, call-error: {:?}", user_canister_id, call_error)); // reject not trap becouse after an await
            return;
        }
    }
            
}










#[derive(CandidType, Deserialize)]
pub struct Fees {
    purchase_cycles_bank_cost_cycles: Cycles,
    purchase_cycles_bank_upgrade_cost_cycles: Cycles,
    purchase_cycles_transfer_cost_cycles: Cycles,
    convert_icp_for_the_cycles_with_the_cmc_rate_cost_cycles: Cycles,
    minimum_cycles_transfer_into_user: Cycles,
    cycles_transfer_into_user_user_not_found_fee_cycles: Cycles,
    
    
}

#[query]
pub fn see_fees() -> Fees {
    Fees {
        purchase_cycles_bank_cost_cycles: CYCLES_BANK_COST,
        purchase_cycles_bank_upgrade_cost_cycles: CYCLES_BANK_UPGRADE_COST,
        purchase_cycles_transfer_cost_cycles: CYCLES_TRANSFER_FEE,
        convert_icp_for_the_cycles_with_the_cmc_rate_cost_cycles: CONVERT_ICP_FOR_THE_CYCLES_WITH_THE_CMC_RATE_FEE,
        minimum_cycles_transfer_into_user: MINIMUM_CYCLES_TRANSFER_INTO_USER,
        cycles_transfer_into_user_user_not_found_fee_cycles: CYCLES_TRANSFER_INTO_USER_USER_NOT_FOUND_FEE,
        
    }
}








#[derive(CandidType, Deserialize)]
pub struct TopUpCyclesBalanceData {
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
            topup_cycles_transfer_memo: CyclesTransferMemo::Blob(user_cycles_balance_topup_memo_bytes(&user_id).to_vec())
        },
        topup_icp_balance: TopUpIcpBalanceData {
            topup_icp_id: cts_lib::tools::user_icp_id(id(), &user_id)
        }
    }
}










#[derive(Copy,Clone)]
struct NewUserData {
    lock_start_time_nanos: u64,
    lock: bool,
    
    // the options are for the memberance of the steps
    user_icp_ledger_balance: Option<IcpTokens>,
    current_xdr_icp_rate: Option<u64>,
    look_if_user_is_in_the_users_map_canisters: bool,
    create_user_canister_block_height: Option<IcpBlockHeight>,
    user_canister: Option<UserId>,
    user_canister_uninstall_code: bool,
    user_canister_install_code: bool,
    user_canister_status_record: Option<ManagementCanisterCanisterStatusRecord>,
    users_map_canister: Option<UsersMapCanisterId>,    
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
pub struct NewUserSuccessData {
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
                let nud: NewUserData = NewUserData::new();
                new_users.insert(user_id, nud);
                new_user_data = *nud; //copy
            }
        }
    });


    if new_user_data.user_icp_ledger_balance.is_none() || new_user_data.current_xdr_icp_rate.is_none() {

        let (
            check_user_icp_ledger_balance_sponse,
            check_current_xdr_permyriad_per_icp_cmc_rate_sponse,
        ): (
            CallResult<IcpTokens>,
            CheckCurrentXdrPerMyriadPerIcpCmcRateSponse
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


    if new_user_data.look_if_user_is_in_the_users_map_canisters == false {
        // check in the list of the users-whos cycles-balance is save but without a user-canister   
        
        match find_user_in_the_users_map_canisters(user_id).await {
            Ok((user_canister_id, users_map_canister_id)) => {
                with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
                return Err(NewUserError::FoundUserCanister(user_canister_id));
            },
            Err(find_user_error) => match find_user_error {
                FindUserInTheUsersMapCanistersError::UserNotFound => {
                    new_user_data.look_if_user_is_in_the_users_map_canisters = true;                    
                },
                FindUserInTheUsersMapCanistersError::UsersMapCanisterCallFail => {
                    new_user_data.lock = false;
                    write_new_user_data(&user_id, new_user_data);
                    return Err(NewUserError::MidCallError(NewUserMidCallError::UsersMapCanisterFindUserCallFail));
                }
            }
        };
        
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


    if new_user_data.user_canister.is_none() {
    
        let user_canister: Principal = match call::<(CmcNotifyCreateCanisterQuest,), (Result<Principal, CmcNotifyError>,)>(
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
                        CmcNotifyError::TransactionTooOld
                        | CmcNotifyError::InvalidTransaction
                        | CmcNotifyError::Refunded
                        => {
                            with_mut(&NEW_USERS, |nus| { nus.remove(user_id); });
                            return Err(NewUserError::CreateUserCanisterCmcNotifyError(cmc_notify_error));
                        },
                        | CmcNotifyError::Other
                        | CmcNotifyError::Processing
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
        
        new_user_data.user_canister = Some(user_canister);
        new_user_data.user_canister_uninstall_code = true; // because a fresh cmc canister is empty 
    }


    if new_user_data.user_canister_uninstall_code == false {
        
        match call::<()>(
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

        match call::<()>(
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
    
        match call::<()>(
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
    
    
    
    if new_user_data.users_map_canister.is_none() {
        
        let users_map_canister_id: UsersMapCanisterId = match put_new_user_into_a_users_map_canister(user_id, new_user_data.users_map_canister_data.unwrap().0).await {
            Ok(umcid) => umcid,
            Err(put_new_user_into_a_users_map_canister_error) => {
                new_user_data.lock = false;
                write_new_user_data(&user_id, new_user_data);
                return Err(NewUserError::MidCallError(NewUserMidCallError::PutNewUserIntoAUsersMapCanisterError(put_new_user_into_a_users_map_canister_error)));
            }
        };
        
        new_user_data.users_map_canister = Some(users_map_canister_id);
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
    with(&USERS_MAP_CANISTERS, |umcs| {
        ic_cdk::api::call::reply::<(&Vec<Principal>,)>((umcs,));
    });
}



#[update]
pub async fn find_user(user_id: Principal) -> Result<> {

}

































// test this!
#[no_mangle]
pub fn canister_inspect_message() {
    // caution: this function is only called for ingress messages 
    use ic_cdk::api::call::{method_name,accept_message};
    
    if caller() == Principal::anonymous() 
        && !["see_fees"].contains(&&method_name()[..])
        {
        trap("caller cannot be anonymous for this method.")
    }

    if &method_name()[..] == "cycles_transfer" {
        trap("caller must be a canister for this method.")
    }

    accept_message();
}

/*
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
*/



/*
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
*/

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





