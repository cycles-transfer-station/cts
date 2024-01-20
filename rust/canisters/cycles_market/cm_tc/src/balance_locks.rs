
use super::*;

#[derive(CandidType, Deserialize)]
pub enum PutUserBalanceLockError {
    CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance,
    CyclesMarketIsBusy,
}

pub fn put_user_cycles_balance_lock(cm_data: &mut CMData, user: Principal) -> Result<(), PutUserBalanceLockError> {
    _put_user_balance_lock(&mut cm_data.mid_call_user_cycles_balance_locks, user)
}

pub fn put_user_token_balance_lock(cm_data: &mut CMData, user: Principal) -> Result<(), PutUserBalanceLockError> {
    _put_user_balance_lock(&mut cm_data.mid_call_user_token_balance_locks, user)
}

fn _put_user_balance_lock(cm_data_mid_call_balance_locks: &mut HashSet<Principal>, user: Principal) -> Result<(), PutUserBalanceLockError> {
    if cm_data_mid_call_balance_locks.contains(&user) {
        return Err(PutUserBalanceLockError::CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance);
    }
    if cm_data_mid_call_balance_locks.len() >= MAX_MID_CALL_USER_BALANCE_LOCKS {
        return Err(PutUserBalanceLockError::CyclesMarketIsBusy);
    }
    cm_data_mid_call_balance_locks.insert(user);
    Ok(())
}

pub fn remove_user_cycles_balance_lock(cm_data: &mut CMData, user: Principal) {
    cm_data.mid_call_user_cycles_balance_locks.remove(&user);
}

pub fn remove_user_token_balance_lock(cm_data: &mut CMData, user: Principal) {
    cm_data.mid_call_user_token_balance_locks.remove(&user);
}



impl From<PutUserBalanceLockError> for TradeError {
    fn from(x: PutUserBalanceLockError) -> TradeError {
        match x {
            PutUserBalanceLockError::CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance => TradeError::CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance,
            PutUserBalanceLockError::CyclesMarketIsBusy => TradeError::CyclesMarketIsBusy,
        }
    }
}

impl From<PutUserBalanceLockError> for TransferBalanceError {
    fn from(x: PutUserBalanceLockError) -> TransferBalanceError {
        match x {
            PutUserBalanceLockError::CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance => TransferBalanceError::CallerIsInTheMiddleOfADifferentCallThatLocksTheBalance,
            PutUserBalanceLockError::CyclesMarketIsBusy => TransferBalanceError::CyclesMarketIsBusy,
        }
    }
}
