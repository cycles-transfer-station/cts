
use std::cell::RefCell;
use std::thread::LocalKey;


use crate::{
    ic_cdk::{
        caller,
        trap,
        export::{
            candid::{CandidType, Deserialize, encode_one, decode_one},
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

use serde_bytes::{ByteBuf, Bytes};


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;
const STABLE_MEMORY_ID_STATE_SNAPSHOT_SERIALIZATION: MemoryId = MemoryId::new(0);

thread_local!{
    
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    
    static STATE_SNAPSHOT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    
    static LOAD_DATA_BYTES_FUNCTION: RefCell<Box<dyn Fn(&[u8]) -> Result<(), String>>> = RefCell::new(Box::new(|_b| { trap("call the stable_memory_tools init and post_upgrade functions") }));
    static GET_DATA_BYTES_FUNCTION: RefCell<Box<dyn Fn() -> Result<Vec<u8>, String>>> = RefCell::new(Box::new(|| { trap("call the stable_memory_tools init and post_upgrade functions") }));


}


pub fn get_stable_memory(memory_id: MemoryId) -> VirtualMemory<DefaultMemoryImpl> {
    with(&MEMORY_MANAGER, |mgr| mgr.get(memory_id))
}



fn get_state_snapshot_stable_memory() -> VirtualMemory<DefaultMemoryImpl> {
    get_stable_memory(STABLE_MEMORY_ID_STATE_SNAPSHOT_SERIALIZATION)
}


pub trait Serializable {
    fn forward(&self) -> Result<Vec<u8>, String>;
    fn backward(b: &[u8]) -> Result<Self, String> where Self: Sized;     
}

impl<T: CandidType + for<'a> Deserialize<'a>> Serializable for T {
    fn forward(&self) -> Result<Vec<u8>, String> {
        encode_one(self).map_err(|e| format!("{:?}", e))
    }
    fn backward(b: &[u8]) -> Result<Self, String> {
        decode_one::<T>(b).map_err(|e| format!("{:?}", e))
    }
}



pub fn init<Data: 'static + Serializable>(s: &'static LocalKey<RefCell<Data>>) {
    with_mut(&LOAD_DATA_BYTES_FUNCTION, |load_data_bytes_fn| {
        *load_data_bytes_fn = Box::new(move |b| {
            with_mut(s, |data| {
                *data = <Data as Serializable>::backward(b)?;
                Ok(())
            })
        });
    });
    with_mut(&GET_DATA_BYTES_FUNCTION, |get_data_bytes_fn| {
        *get_data_bytes_fn = Box::new(move || { 
            with(s, |data| {
                <Data as Serializable>::forward(data)
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

pub fn post_upgrade<Data, OldData, F>(s: &'static LocalKey<RefCell<Data>>, opt_old_as_new_convert: Option<F>) 
    where 
        Data: 'static + Serializable,
        OldData: Serializable,
        F: Fn(OldData) -> Data
    {

    read_stable_memory_bytes_with_length_onto_the_state_snapshot(
        &get_state_snapshot_stable_memory(),
        STABLE_MEMORY_HEADER_SIZE_BYTES
    );

    with(&STATE_SNAPSHOT, |state_snapshot| {
        with_mut(s, |data| {
            *data = match opt_old_as_new_convert {
                Some(ref old_as_new_convert) => old_as_new_convert(<OldData as Serializable>::backward(state_snapshot).unwrap()),
                None => <Data as Serializable>::backward(state_snapshot).unwrap(),
            };
        });
    });
    
    // portant!
    init(s);
    
}






fn create_state_snapshot() -> Result<u64, String> {
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = with(&GET_DATA_BYTES_FUNCTION, |f| { f() })?;
        Ok(state_snapshot.len() as u64)
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
fn controller_create_state_snapshot() { //-> u64/*len of the state_snapshot*/ {
    caller_is_controller_gaurd(&caller());
        
    reply::<(u64,)>((create_state_snapshot().unwrap(),));
}

#[export_name = "canister_query controller_download_state_snapshot"]
fn controller_download_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    let chunk_size: usize = 1 * MiB as usize;
    with(&STATE_SNAPSHOT, |state_snapshot| {
        let (chunk_i,): (u64,) = arg_data::<(u64,)>(); // starts at 0
        reply::<(Option<&Bytes/*&[u8]*/>,)>((state_snapshot.chunks(chunk_size).nth(chunk_i as usize).map(Bytes::new),));
    });
}

#[export_name = "canister_update controller_clear_state_snapshot"]
fn controller_clear_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        *state_snapshot = Vec::new();
    });
    
    reply::<()>(());
}

#[export_name = "canister_update controller_append_state_snapshot"]
fn controller_append_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    with_mut(&STATE_SNAPSHOT, |state_snapshot| {
        state_snapshot.append(&mut arg_data::<(ByteBuf,)>().0);
    });
    
    reply::<()>(());
}

#[export_name = "canister_update controller_load_state_snapshot"]
fn controller_load_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    with(&STATE_SNAPSHOT, |state_snapshot| {
        with(&LOAD_DATA_BYTES_FUNCTION, |f| {
            f(state_snapshot).unwrap()
        })
    });
    
    reply::<()>(());
}


// ----------- STABLE-MEMORY CONTROLLER METHODS -----------

#[export_name = "canister_query controller_stable_memory_read"]
fn controller_stable_memory_read() {
    caller_is_controller_gaurd(&caller());
    
    let (memory_id, offset, length) = arg_data::<(u8, u64, u64)>();
    
    let mut b: Vec<u8> = vec![0; length.try_into().unwrap()];
    
    get_stable_memory(MemoryId::new(memory_id)).read(offset, &mut b);
    
    reply::<(ByteBuf,)>((ByteBuf::from(b),));
    
}

#[export_name = "canister_update controller_stable_memory_write"]
fn controller_stable_memory_write() {
    caller_is_controller_gaurd(&caller());

    let (memory_id, offset, b) = arg_data::<(u8, u64, ByteBuf)>();
        
    get_stable_memory(MemoryId::new(memory_id)).write(offset, &b);
    
    reply::<()>(());
    
}


#[export_name = "canister_query controller_stable_memory_size"]
fn controller_stable_memory_size() {
    caller_is_controller_gaurd(&caller());

    let (memory_id,) = arg_data::<(u8,)>();
        
    reply::<(u64,)>((get_stable_memory(MemoryId::new(memory_id)).size(),));
    
}


#[export_name = "canister_update controller_stable_memory_grow"]
fn controller_stable_memory_grow() {
    caller_is_controller_gaurd(&caller());

    let (memory_id, pages) = arg_data::<(u8, u64)>();
        
    reply::<(i64,)>((get_stable_memory(MemoryId::new(memory_id)).grow(pages),));
    
}

