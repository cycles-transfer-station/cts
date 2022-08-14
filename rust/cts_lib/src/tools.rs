use sha2::Digest;
use crate::{
    ic_cdk::export::Principal,
    ic_ledger_types::{
        IcpIdSub,
        IcpId,
        IcpTokens
    },
    consts::{
        CYCLES_PER_XDR
    }
};




pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher: sha2::Sha256 = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}



pub mod localkey {
    pub mod refcell {
        use std::{
            cell::RefCell,
            thread::LocalKey,
        };

        pub fn with<T: 'static, R, F>(s: &'static LocalKey<RefCell<T>>, f: F) -> R
        where 
            F: FnOnce(&T) -> R 
        {
            s.with(|b| {
                f(&*b.borrow())
            })
        }
        
        pub fn with_mut<T: 'static, R, F>(s: &'static LocalKey<RefCell<T>>, f: F) -> R
        where 
            F: FnOnce(&mut T) -> R 
        {
            s.with(|b| {
                f(&mut *b.borrow_mut())
            })
        }
        /*
        pub unsafe fn get<T: 'static>(s: &'static LocalKey<RefCell<T>>) -> &T {
            let pointer: *const T = with(s, |i| { i as *const T });
            &*pointer
        }
        
        pub unsafe fn get_mut<T: 'static>(s: &'static LocalKey<RefCell<T>>) -> &mut T {
            let pointer: *mut T = with_mut(s, |i| { i as *mut T });
            &mut *pointer
        }
        */
    }
    pub mod cell {
        use std::{
            cell::Cell,
            thread::LocalKey
        };
        pub fn get<T: 'static + Copy>(s: &'static LocalKey<Cell<T>>) -> T {
            s.with(|c| { c.get() })
        }
        pub fn set<T: 'static + Copy>(s: &'static LocalKey<Cell<T>>, v: T) {
            s.with(|c| { c.set(v); });
        }
        
    }
}







pub fn principal_as_thirty_bytes(p: &Principal) -> [u8; 30] {
    let mut bytes: [u8; 30] = [0; 30];
    let p_bytes: &[u8] = p.as_slice();
    bytes[0] = p_bytes.len() as u8; 
    bytes[1 .. p_bytes.len() + 1].copy_from_slice(p_bytes); 
    bytes
}

pub fn thirty_bytes_as_principal(bytes: &[u8; 30]) -> Principal {
    Principal::from_slice(&bytes[1..1 + bytes[0] as usize])
} 


#[test]
fn thirty_bytes_principal() {
    let test_principal: Principal = Principal::from_slice(&[0,1,2,3,4,5,6,7,8,9]);
    assert_eq!(test_principal, thirty_bytes_as_principal(&principal_as_thirty_bytes(&test_principal)));
}










pub fn principal_icp_subaccount(principal: &Principal) -> IcpIdSub {
    let mut sub_bytes = [0u8; 32];
    sub_bytes[..30].copy_from_slice(&principal_as_thirty_bytes(principal));
    IcpIdSub(sub_bytes)
}

pub fn user_icp_id(cts_id: &Principal, user_id: &Principal) -> IcpId {
    IcpId::new(cts_id, &principal_icp_subaccount(user_id))
}









pub fn icptokens_to_cycles(icpts: IcpTokens, xdr_permyriad_per_icp: u64) -> u128 {
    icpts.e8s() as u128 
    * xdr_permyriad_per_icp as u128 
    * CYCLES_PER_XDR 
    / (IcpTokens::SUBDIVIDABLE_BY as u128 * 10_000)
}

pub fn cycles_to_icptokens(cycles: u128, xdr_permyriad_per_icp: u64) -> IcpTokens {
    IcpTokens::from_e8s(
        ( cycles
        * (IcpTokens::SUBDIVIDABLE_BY as u128 * 10_000)
        / CYCLES_PER_XDR
        / xdr_permyriad_per_icp as u128 ) as u64    
    )
}


#[test]
fn test_icp_cycles_transform() {
    let t: IcpTokens = IcpTokens::from_e8s(0123456789);
    let xdr_permyriad_per_icp: u64 = 45628;
    assert_eq!(t, cycles_to_icptokens(icptokens_to_cycles(t, xdr_permyriad_per_icp), xdr_permyriad_per_icp));
}









