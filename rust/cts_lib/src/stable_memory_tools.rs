
use std::cell::RefCell;
use std::thread::LocalKey;
use std::collections::BTreeMap;

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
    memory_manager::{MemoryManager, VirtualMemory},  
};
pub use ic_stable_structures::memory_manager::MemoryId;

use serde_bytes::{ByteBuf, Bytes};


const STABLE_MEMORY_HEADER_SIZE_BYTES: u64 = 1024;

type StateSnapshots = BTreeMap<MemoryId, (
    Vec<u8>,
    Box<dyn Fn(&[u8]) -> Result<(), String>>, // load data bytes function
    Box<dyn Fn() -> Result<Vec<u8>, String>>, // get data bytes function
)>;

thread_local!{
    
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    
    static STATE_SNAPSHOTS: RefCell<StateSnapshots> = RefCell::new(StateSnapshots::new());

}


pub fn get_stable_memory(memory_id: MemoryId) -> VirtualMemory<DefaultMemoryImpl> {
    with(&MEMORY_MANAGER, |mgr| mgr.get(memory_id))
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



pub fn init<Data: 'static + Serializable>(s: &'static LocalKey<RefCell<Data>>, memory_id: MemoryId) {
    with_mut(&STATE_SNAPSHOTS, |state_snapshots| {
        state_snapshots.insert(
            memory_id,
            (
                Vec::new(),
                Box::new(move |b| {
                    with_mut(s, |data| {
                        *data = <Data as Serializable>::backward(b)?;
                        Ok(())
                    })
                }),
                Box::new(move || { 
                    with(s, |data| {
                        <Data as Serializable>::forward(data)
                    })
                })
            )
        ); 
    });    
}

pub fn pre_upgrade() {
    with_mut(&STATE_SNAPSHOTS, |state_snapshots| {
        for (memory_id, d) in state_snapshots.iter_mut() {
            d.0 = d.2().unwrap();
            write_data_with_length_onto_the_stable_memory(
                &get_stable_memory(*memory_id/*.clone()*/),
                STABLE_MEMORY_HEADER_SIZE_BYTES,
                &d.0
            ).unwrap();
        }
    });
}

pub fn post_upgrade<Data, OldData, F>(s: &'static LocalKey<RefCell<Data>>, memory_id: MemoryId, opt_old_as_new_convert: Option<F>) 
    where 
        Data: 'static + Serializable,
        OldData: Serializable,
        F: Fn(OldData) -> Data
    {
                
    let stable_data: Vec<u8> = read_stable_memory_bytes_with_length(
        &get_stable_memory(memory_id),
        STABLE_MEMORY_HEADER_SIZE_BYTES,
    );

    with_mut(s, |data| {
        *data = match opt_old_as_new_convert {
            Some(ref old_as_new_convert) => old_as_new_convert(<OldData as Serializable>::backward(&stable_data).unwrap()),
            None => <Data as Serializable>::backward(&stable_data).unwrap(),
        };
    });
    
    // portant!
    init(s, memory_id);
    
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



fn write_data_with_length_onto_the_stable_memory(serialization_memory: &VirtualMemory<DefaultMemoryImpl>, stable_memory_offset: u64, data: &[u8]) -> Result<(), ()> {
    locate_minimum_memory(
        serialization_memory,
        stable_memory_offset + 8/*len of the data*/ + data.len() as u64
    )?; 
    serialization_memory.write(stable_memory_offset, &((data.len() as u64).to_be_bytes()));
    serialization_memory.write(stable_memory_offset + 8, data);
    Ok(())
}

fn read_stable_memory_bytes_with_length(serialization_memory: &VirtualMemory<DefaultMemoryImpl>, stable_memory_offset: u64) -> Vec<u8> {
    
    let mut data_len_u64_be_bytes: [u8; 8] = [0; 8];
    serialization_memory.read(stable_memory_offset, &mut data_len_u64_be_bytes);
    let data_len_u64: u64 = u64::from_be_bytes(data_len_u64_be_bytes); 
    
    let mut data: Vec<u8> = vec![0; data_len_u64.try_into().unwrap()]; 
    serialization_memory.read(stable_memory_offset + 8, &mut data);
    data
}









// ---- STATE-SNAPSHOT CONTROLLER METHODS ---------

#[export_name = "canister_update controller_create_state_snapshot"]
fn controller_create_state_snapshot() { //-> u64/*len of the state_snapshot*/ {
    caller_is_controller_gaurd(&caller());
        
    let memory_id: MemoryId = MemoryId::new(arg_data::<(u8,)>().0);
    
    let state_snapshot_len: u64 = with_mut(&STATE_SNAPSHOTS, |state_snapshots| {
        match state_snapshots.get_mut(&memory_id) {
            None => trap("no data associated with this memory_id"),
            Some(d) => {
                d.0 = d.2().unwrap();
                d.0.len() as u64
            }
        }
    });

    reply::<(u64,)>((state_snapshot_len,));
}

#[export_name = "canister_query controller_download_state_snapshot"]
fn controller_download_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    let chunk_size: usize = 1 * MiB as usize;
    let (chunk_i, memory_id) = arg_data::<(u64, u8)>();
    
    with(&STATE_SNAPSHOTS, |state_snapshots| {
        match state_snapshots.get(&MemoryId::new(memory_id)) {
            None => trap("no data associated with this memory_id"),
            Some(d) => {
                reply::<(Option<&Bytes/*&[u8]*/>,)>((d.0.chunks(chunk_size).nth(chunk_i as usize).map(Bytes::new),));
            }
        }
    });
}

#[export_name = "canister_update controller_clear_state_snapshot"]
fn controller_clear_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    let memory_id: MemoryId = MemoryId::new(arg_data::<(u8,)>().0);
    
    with_mut(&STATE_SNAPSHOTS, |state_snapshots| {
        match state_snapshots.get_mut(&memory_id) {
            None => trap("no data associated with this memory_id"),
            Some(d) => {
                d.0 = Vec::new();
            }
        }
    });
    
    reply::<()>(());
}

#[export_name = "canister_update controller_append_state_snapshot"]
fn controller_append_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    let (memory_id, mut bytes) = arg_data::<(u8, ByteBuf)>();
    
    with_mut(&STATE_SNAPSHOTS, |state_snapshots| {
        match state_snapshots.get_mut(&MemoryId::new(memory_id)) {
            None => trap("no data associated with this memory_id"),
            Some(d) => {
                d.0.append(&mut bytes);
            }
        }
    });
    
    reply::<()>(());
}

#[export_name = "canister_update controller_load_state_snapshot"]
fn controller_load_state_snapshot() {
    caller_is_controller_gaurd(&caller());
    
    let memory_id: MemoryId = MemoryId::new(arg_data::<(u8,)>().0);
    
    with(&STATE_SNAPSHOTS, |state_snapshots| {
        match state_snapshots.get(&memory_id) {
            None => trap("no data associated with this memory_id"),
            Some(d) => {
                d.1(&d.0).unwrap();
            }
        }
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

