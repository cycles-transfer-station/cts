use sha2::Digest;


pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher: sha2::Sha256 = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}



pub mod localkey_refcell {
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
    
    pub unsafe fn get<T: 'static>(s: &'static LocalKey<RefCell<T>>) -> &T {
        let pointer: *const T = with(s, |i| { i as *const T });
        &*pointer
    }
    
    pub unsafe fn get_mut<T: 'static>(s: &'static LocalKey<RefCell<T>>) -> &mut T {
        let pointer: *mut T = with_mut(s, |i| { i as *mut T });
        &mut *pointer
    }
    
}


