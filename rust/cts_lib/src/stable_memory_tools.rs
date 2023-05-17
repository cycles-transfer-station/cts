
use std::cell::RefCell;
use std::thread::LocalKey;


use crate::{
    ic_cdk::{
        caller,
        trap,
        export::{
            candid::{CandidType, Deserialize, encode_one, decode_one, error::Error as CandidError},
        },
        api::{
            call::{
                reply,
                arg_data
            }
        },
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


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;
const STABLE_MEMORY_ID_STATE_SNAPSHOT_SERIALIZATION: MemoryId = MemoryId::new(0);

thread_local!{
    
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    
    static STATE_SNAPSHOT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    
    static PUT_DATA_BYTES_FUNCTION: RefCell<Box<dyn Fn(&[u8]) -> Result<(), CandidError>>> = RefCell::new(Box::new(|_b| { trap("call the stable_memory_tools::set_data function") }));
    static GET_DATA_BYTES_FUNCTION: RefCell<Box<dyn Fn() -> Result<Vec<u8>, CandidError>>> = RefCell::new(Box::new(|| { trap("call the stable_memory_tools::set_data function") }));


}


pub fn get_stable_memory(memory_id: MemoryId) -> VirtualMemory<DefaultMemoryImpl> {
    with(&MEMORY_MANAGER, |mgr| mgr.get(memory_id))
}



fn get_state_snapshot_stable_memory() -> VirtualMemory<DefaultMemoryImpl> {
    get_stable_memory(STABLE_MEMORY_ID_STATE_SNAPSHOT_SERIALIZATION)
}




pub fn set_data<Data, OldData, F>(s: &'static LocalKey<RefCell<Data>>, old_to_new_convert: F) 
    where 
        Data: 'static + CandidType + for<'a> Deserialize<'a>,
        OldData: CandidType + for<'a> Deserialize<'a>,
        F: 'static + Fn(OldData) -> Option<Data> // return type here as option instead of set_data...(... , opt_old_to_new_convert: Option<F>) bc Option<Fn> cannot be moved into the put_data_bytes_fn Fn() closure bc Option<Fn> doesn't implement copy and Fn() closures can be called many times. but Fn itself without the option can be moved into the put_data_bytes_fn Fn closure
    {
    
    with_mut(&PUT_DATA_BYTES_FUNCTION, |put_data_bytes_fn| {
        *put_data_bytes_fn = Box::new(move |b| {
            with_mut(s, |data| {
                *data = match decode_one::<Data>(b) {
                    Ok(d) => d,
                    Err(e) => {
                        let old_data: OldData = decode_one::<OldData>(b)?;
                        let new_data: Data = old_to_new_convert(old_data).ok_or(e)?;
                        new_data
                    }
                };
                Ok(())
            })
        }); 
    });
    
    with_mut(&GET_DATA_BYTES_FUNCTION, |get_data_bytes_fn| {
        *get_data_bytes_fn = Box::new(move || { 
            with(s, |data| {
                encode_one(data)      
            })
        }); 
    });
}

pub fn pre_upgrade() {
    create_state_snapshot().unwrap();
    write_state_snapshot_with_length_onto_the_stable_memory(
        &get_state_snapshot_stable_memory(),
        STABLE_MEMORY_HEADER_SIZE_BYTES
    ).unwrap();
}

pub fn post_upgrade() {
    read_stable_memory_bytes_with_length_onto_the_state_snapshot(
        &get_state_snapshot_stable_memory(),
        STABLE_MEMORY_HEADER_SIZE_BYTES
    );
    load_state_snapshot().unwrap();    
}






fn create_state_snapshot() -> Result<u64, CandidError> {
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = with(&GET_DATA_BYTES_FUNCTION, |f| { f() })?;
        Ok(state_snapshot.len() as u64)
    })
}



fn load_state_snapshot() -> Result<(), CandidError> {   
    with(&STATE_SNAPSHOT, |state_snapshot| {
        with(&PUT_DATA_BYTES_FUNCTION, |f| {
            f(state_snapshot)
        })
    })
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



fn write_state_snapshot_with_length_onto_the_stable_memory(serialization_memory: &VirtualMemory<DefaultMemoryImpl>, offset: u64) -> Result<(), ()> {
    with(&STATE_SNAPSHOT, |state_snapshot| {
        locate_minimum_memory(
            serialization_memory,
            offset + 8/*len of the data*/ + state_snapshot.len() as u64
        )?; 
        serialization_memory.write(offset, &((state_snapshot.len() as u64).to_be_bytes()));
        serialization_memory.write(offset + 8, state_snapshot);
        Ok(())
    })
}

fn read_stable_memory_bytes_with_length_onto_the_state_snapshot(serialization_memory: &VirtualMemory<DefaultMemoryImpl>, offset: u64) {
    
    let mut data_len_u64_be_bytes: [u8; 8] = [0; 8];
    serialization_memory.read(offset, &mut data_len_u64_be_bytes);
    let data_len_u64: u64 = u64::from_be_bytes(data_len_u64_be_bytes); 
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = vec![0; data_len_u64.try_into().unwrap()]; 
        serialization_memory.read(offset + 8, state_snapshot);
    });
}









// ---- STATE-SNAPSHOT CONTROLLER METHODS ---------

#[export_name = "canister_update controller_create_state_snapshot"]
pub fn controller_create_state_snapshot() { //-> u64/*len of the state_snapshot*/ {
    caller_is_controller_gaurd(&caller());
        
    reply::<(u64,)>((create_state_snapshot().unwrap(),));
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

#[export_name = "canister_update controller_load_state_snapshot"]
pub fn controller_load_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    load_state_snapshot().unwrap();
}
