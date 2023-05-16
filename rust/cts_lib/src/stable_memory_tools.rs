
use std::cell::RefCell;
use std::thread::LocalKey;


use crate::{
    ic_cdk::{
        caller,
        trap,
        export::{
            candid::{CandidType, Deserialize, encode_one, decode_one, error::Error as CandidError},
            Principal,
        },
        api::{
            is_controller,
            call::{
                reply,
                arg_data
            }
        },
        update,
        query,
    },
    consts::{
        WASM_PAGE_SIZE_BYTES,
        MiB,
    },
    tools::{
        localkey::refcell::{with, with_mut},
        caller_is_controller_gaurd,
    }
};

use ic_stable_structures::{
    Memory,
    DefaultMemoryImpl, 
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    
};


thread_local!{
     static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    
    static STATE_SNAPSHOT: RefCell<Vec<u8>> = RefCell::new(Vec::new());

}


pub fn get_memory(memory_id: MemoryId) -> VirtualMemory<DefaultMemoryImpl> {
    with(&MEMORY_MANAGER, |mgr| mgr.get(memory_id))
}



// <T: CandidType + for<'a> Deserialize<'a>>
pub fn create_state_snapshot<T: CandidType + for<'a> Deserialize<'a>>(data: &T) -> Result<u64, CandidError> {
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = encode_one(data)?; 
        Ok(state_snapshot.len() as u64)
    })
}



pub fn load_state_snapshot<OldData, Data, F>(opt_old_to_new_convert: Option<F>) -> Result<Data, CandidError>
    where 
        OldData: CandidType + for<'a> Deserialize<'a>,
        Data: CandidType + for<'a> Deserialize<'a>,
        F: FnOnce(OldData) -> Data
    {
    
    let data_of_the_state_snapshot: Data = with(&STATE_SNAPSHOT, |state_snapshot| { 
        match decode_one::<Data>(state_snapshot) {
            Ok(data) => Ok(data),
            Err(e) => match opt_old_to_new_convert {
                None => return Err(e),
                Some(old_to_new_convert) => {
                    let old_data: OldData = decode_one::<OldData>(state_snapshot)?;
                    let new_data: Data = old_to_new_convert(old_data);
                    Ok(new_data)
                }    
            }
        }
    })?;
    
    Ok(data_of_the_state_snapshot)
}


pub fn locate_minimum_memory(memory: &VirtualMemory<DefaultMemoryImpl>, want_memory_size_bytes: u64) -> Result<(),()> {
    let memory_size_wasm_pages: u64 = memory.size();
    let memory_size_bytes: u64 = memory_size_wasm_pages * WASM_PAGE_SIZE_BYTES as u64;
    
    if memory_size_bytes < want_memory_size_bytes {
        let grow_result: i64 = memory.grow(((want_memory_size_bytes - memory_size_bytes) / WASM_PAGE_SIZE_BYTES as u64) + 1);
        if grow_result == -1 {
            return Err(());
        }
    }
    
    Ok(())
}



pub fn write_state_snapshot_with_length_onto_the_stable_memory(serialization_memory: &VirtualMemory<DefaultMemoryImpl>, offset: u64) -> Result<(), ()> {

   
    with(&STATE_SNAPSHOT, |state_snapshot| {
        locate_minimum_memory(
            serialization_memory,
            offset + 8/*len of the data*/ + state_snapshot.len() as u64
        ); 
        serialization_memory.write(offset, &((state_snapshot.len() as u64).to_be_bytes()));
        serialization_memory.write(offset + 8, state_snapshot);
        Ok(())
    })?;
    Ok(())
}

pub fn read_stable_memory_bytes_with_length_onto_the_state_snapshot(serialization_memory: &VirtualMemory<DefaultMemoryImpl>, offset: u64) {
    
    let mut data_len_u64_be_bytes: [u8; 8] = [0; 8];
    serialization_memory.read(offset, &mut data_len_u64_be_bytes);
    let data_len_u64: u64 = u64::from_be_bytes(data_len_u64_be_bytes); 
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = vec![0; data_len_u64.try_into().unwrap()]; 
        serialization_memory.read(offset + 8, state_snapshot);
    });
}











#[export_name = "canister_query controller_download_state_snapshot"]
pub fn controller_download_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT, |state_snapshot| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&[u8]>,)>((state_snapshot.chunks(chunk_size).nth(chunk_i as usize),));
    });
}



#[export_name = "canister_update controller_clear_state_snapshot"]
pub fn controller_clear_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = Vec::new();
    });
}

#[export_name = "canister_update controller_append_state_snapshot"]
pub fn controller_append_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.append(&mut arg_data::<(Vec<u8>,)>().0);
    });
}


