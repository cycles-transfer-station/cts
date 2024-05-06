use crate::*;


const MAX_NUMBER_OF_LOG_SPACES_ON_HOLD: u64 = 200;

// for making sure there is space in the stable-memory to store logs of blocks that need multiple steps (async) to perform.

// for making this private

thread_local!{
    //this should be reset every upgrade. all async calls should finish before an upgrade using the stop_canister method.
    static LOGS_MEMORY_MANAGER: RefCell<LogsMemoryManager> = RefCell::new(LogsMemoryManager::new());
}


struct LogsMemoryManager {
    number_of_log_spaces_in_front_of_the_last_block: u64, // total number of additional spaces not yet commited to
    number_of_log_spaces_on_hold: u64                              // number of spaces with a specific-hold by a caller
}
impl LogsMemoryManager {
    fn new() -> Self {
        Self {
            number_of_log_spaces_in_front_of_the_last_block: 0,
            number_of_log_spaces_on_hold: 0,
        }
    }
}

#[derive(Debug)]
pub enum HoldSpaceForALogError {
    IcStableStructuresGrowFail(ic_stable_structures::GrowFailed),
    TooManyLogSpacesOnHold{ number_of_log_spaces_on_hold: u64 }     
}
impl From<ic_stable_structures::GrowFailed> for HoldSpaceForALogError {
    fn from(e: ic_stable_structures::GrowFailed) -> Self {
        Self::IcStableStructuresGrowFail(e)
    }
}

pub fn hold_space_for_a_log() -> Result<(), HoldSpaceForALogError> {
    with_mut(&LOGS_MEMORY_MANAGER, |lmm| {
        if lmm.number_of_log_spaces_on_hold >= MAX_NUMBER_OF_LOG_SPACES_ON_HOLD {
            return Err(HoldSpaceForALogError::TooManyLogSpacesOnHold{ number_of_log_spaces_on_hold: lmm.number_of_log_spaces_on_hold });
        }
        
        if lmm.number_of_log_spaces_on_hold < lmm.number_of_log_spaces_in_front_of_the_last_block {
            lmm.number_of_log_spaces_on_hold += 1;
            return Ok(()); 
        }
        
        with_mut(&LOGS, |logs| {
            logs.push(&Log::placeholder())?;                
            lmm.number_of_log_spaces_in_front_of_the_last_block += 1;            
            lmm.number_of_log_spaces_on_hold += 1;
            Ok(())
        })        
    })
} 
    
pub fn cancel_hold_space_for_a_log() {
    with_mut(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold = lmm.number_of_log_spaces_on_hold.saturating_sub(1);
    });
}

pub fn push_log(log: &Log, use_up_hold_space: bool) -> Result<(), ic_stable_structures::GrowFailed>{
    with_mut(&LOGS, |logs| {
        with_mut(&LOGS_MEMORY_MANAGER, |lmm| {
            if use_up_hold_space == true {
                if lmm.number_of_log_spaces_on_hold < 1 {
                    trap("this should not happen. must set use_up_hold_space: true only when there is already at least one space on hold");
                }
                logs.set((logs.len() - lmm.number_of_log_spaces_in_front_of_the_last_block) as u64, &log);
                lmm.number_of_log_spaces_on_hold -= 1;
                lmm.number_of_log_spaces_in_front_of_the_last_block -= 1;
            } else {                
                if lmm.number_of_log_spaces_on_hold == lmm.number_of_log_spaces_in_front_of_the_last_block {
                    logs.push(&Log::placeholder())?;
                    logs.set((logs.len() - 1 - lmm.number_of_log_spaces_in_front_of_the_last_block) as u64, &log);
                } else { 
                    // number_of_log_spaces_on_hold < lmm.number_of_log_spaces_in_front_of_the_last_block 
                    logs.set((logs.len() - lmm.number_of_log_spaces_in_front_of_the_last_block) as u64, &log);
                    lmm.number_of_log_spaces_in_front_of_the_last_block -= 1;              
                }
            }
            Ok(())
        })
    })
}

pub fn get_log(i: u64) -> Option<Log> {
    with(&LOGS, |logs| {
        let lmm_check = with(&LOGS_MEMORY_MANAGER, |lmm| {
            if i > logs.len() - 1 - lmm.number_of_log_spaces_in_front_of_the_last_block {
                return Err(());
            }
            Ok(())
        });
        if let Err(_) = lmm_check {
            return None;
        }
        logs.get(i)
    })
}

pub fn logs_len() -> u64 {
    with(&LOGS, |logs| {
        with(&LOGS_MEMORY_MANAGER, |lmm| {
            logs.len() - lmm.number_of_log_spaces_in_front_of_the_last_block
        })
    })
}






// --- TESTS ---

#[test]
fn test_lmm() {
    let mut c: u64 = 1;
    let mut create_test_log = || -> Log {
        let log = Log{
            ts: c,
            ..Log::placeholder()            
        };
        c += 1;
        log
    };
    
    
    hold_space_for_a_log().unwrap();
    
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 1
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 1
    }));
    
    hold_space_for_a_log().unwrap();
    hold_space_for_a_log().unwrap();
    hold_space_for_a_log().unwrap();
    
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 4
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 4
    }));
    
    cancel_hold_space_for_a_log();
    
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 3
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 4
    }));
        
    push_log(&create_test_log(), false).unwrap();
    
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 3
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 3
    }));
    assert!(with(&LOGS, |logs| {
        logs.len() == 4
    }));
 
    push_log(&create_test_log(), false).unwrap();
               
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 3
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 3
    }));
    assert!(with(&LOGS, |logs| {
        logs.len() == 5
    }));
    
    push_log(&create_test_log(), true).unwrap();
               
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 2
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 2
    }));
    assert!(with(&LOGS, |logs| {
        logs.len() == 5
    }));

    cancel_hold_space_for_a_log();
    cancel_hold_space_for_a_log();
            
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 0
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 2
    }));
    
    push_log(&create_test_log(), false).unwrap();
    push_log(&create_test_log(), false).unwrap();
               
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 0
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 0
    }));
    assert!(with(&LOGS, |logs| {
        logs.len() == 5
    }));
    
    push_log(&create_test_log(), false).unwrap();
               
    assert!(with(&LOGS_MEMORY_MANAGER, |lmm| {
        lmm.number_of_log_spaces_on_hold == 0
        && lmm.number_of_log_spaces_in_front_of_the_last_block == 0
    }));
    assert!(with(&LOGS, |logs| {
        logs.len() == 6
    }));
            
    for i in 0..6 {
        assert_eq!(
            get_log(i).unwrap(),
            Log{
                ts: i + 1,
                ..Log::placeholder()
            }
        );
    }
            
}