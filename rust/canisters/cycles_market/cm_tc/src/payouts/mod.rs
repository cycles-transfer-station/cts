use crate::*;

mod cycles_payouts;
use cycles_payouts::*;

mod token_payouts;
use token_payouts::*;

mod update_storage_positions;
use update_storage_positions::*;

use flush_logs::flush_logs;

use std::future::Future;
use std::collections::BTreeMap;





pub async fn do_payouts() {
    
    if with(&CM_DATA, |cm_data| {
        cm_data.void_cycles_positions.len() == 0
        && cm_data.void_token_positions.len() == 0
        && cm_data.trade_logs.len() == 0
    }) { return; }

    match call::<(),()>(
        ic_cdk::api::id(),
        "do_payouts_public_method",
        (),
    ).await {
        Ok(()) => {
            with_mut(&CM_DATA, |cm_data| {
                with_mut(&TRADES_STORAGE_DATA, |trades_storage_data| {
                    while cm_data.trade_logs.len() > 0 {
                        if cm_data.trade_logs[0].can_move_into_the_stable_memory_for_the_long_term_storage() == true {
                            trades_storage_data.storage_buffer.extend(cm_data.trade_logs.pop_front().unwrap().stable_memory_serialize());
                        } else {
                            break; // bc want to save into the stable-memory in the correct sequence.
                        }
                    }
                })
            });
            futures::join!(
                flush_logs(&TRADES_STORAGE_DATA),
                flush_logs(&POSITIONS_STORAGE_DATA)    
            );       
        },
        Err(call_error) => {
            with_mut(&CM_DATA, |cm_data| {
                cm_data.do_payouts_errors.push(call_error_as_u32_and_string(call_error));
            });
        }
    }
}

#[export_name = "canister_update do_payouts_public_method"]
pub extern "C" fn do_payouts_public_method() {
    let caller: Principal = caller();
    if ic_cdk::api::id() != caller && is_controller(&caller) == false {
        trap("caller without the authorization.");
    }
    
    ic_cdk::spawn(_do_payouts());
    reply::<()>(());
}





async fn _do_payouts() {

    let mut void_cycles_positions_cycles_payouts_chunk: Vec<(VoidCyclesPositionId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new();
    let mut void_token_positions_token_payouts_chunk: Vec<(VoidTokenPositionId, _/*anonymous-future of the do_token_payout-async-function*/)> = Vec::new();
    let mut void_cycles_positions_update_storage_positions_chunk: Vec<(VoidCyclesPositionId, _)> = Vec::new();
    let mut void_token_positions_update_storage_positions_chunk: Vec<(VoidTokenPositionId, _)> = Vec::new();    
    let mut trade_logs_cycles_payouts_chunk: Vec<(PurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut trade_logs_token_payouts_chunk: Vec<(PurchaseId, _/*anonymous-future of the do_token_payout-async-function*/)> = Vec::new();
    
    with_mut(&CM_DATA, |cm_data| {
        fn void_positions_payouts<VoidPosition: VoidPositionTrait, DoPayoutFuture, F: Fn(VoidPosition)->DoPayoutFuture>(
            void_positions: &mut Vec<VoidPosition>, 
            do_payout: F
        ) 
        -> 
        (
            Vec<(PositionId, DoPayoutFuture)>/*payouts_chunk*/, 
            Vec<(PositionId, impl Future<Output=DoUpdateStoragePositionResult>)>/*update_storage_positions_chunk*/
        ) 
        {
            let mut payouts_chunk: Vec<(PositionId, _)> = Vec::new();
            let mut update_storage_positions_chunk: Vec<(PositionId, _)> = Vec::new();
            
            let positions_storage_flush_lock: bool = with(&POSITIONS_STORAGE_DATA, |positions_storage_data| { positions_storage_data.storage_flush_lock });
            
            let mut i: usize = 0;
            while i < void_positions.len() 
            && (payouts_chunk.len() < DO_VOID_POSITIONS_PAYOUTS_CHUNK_SIZE || update_storage_positions_chunk.len() < DO_VOID_POSITIONS_UPDATE_STORAGE_POSITION_CHUNK_SIZE) {
                let vp: &mut VoidPosition = &mut void_positions[i];
                
                if payouts_chunk.len() < DO_VOID_POSITIONS_PAYOUTS_CHUNK_SIZE
                && vp.payout_data().is_complete() == false
                && *vp.payout_lock() == false {
                    *vp.payout_lock() = true;
                    payouts_chunk.push((
                        vp.position_id(),
                        do_payout(vp.clone())
                    ));
                }
                
                if positions_storage_flush_lock == false
                && update_storage_positions_chunk.len() < DO_VOID_POSITIONS_UPDATE_STORAGE_POSITION_CHUNK_SIZE
                && vp.update_storage_position_data().status == false 
                && vp.update_storage_position_data().lock == false {
                    vp.update_storage_position_data().lock = true;
                    update_storage_positions_chunk.push((
                        vp.position_id(),
                        do_update_storage_position(vp.position_id(), vp.update_storage_position_data().update_storage_position_log_serialization_b.clone())
                    ));
                }
                i += 1;
            }
            
            (payouts_chunk, update_storage_positions_chunk)
        }
        
        (void_cycles_positions_cycles_payouts_chunk, void_cycles_positions_update_storage_positions_chunk) = void_positions_payouts(&mut cm_data.void_cycles_positions, do_cycles_payout);
        (void_token_positions_token_payouts_chunk, void_token_positions_update_storage_positions_chunk) = void_positions_payouts(&mut cm_data.void_token_positions, do_token_payout);
        
        let mut i: usize = 0;
        while i < cm_data.trade_logs.len() {
            let tl: &mut TradeLog = &mut cm_data.trade_logs[i];                    
            if tl.cycles_payout_data.is_complete() == false 
            && tl.cycles_payout_lock == false
            && trade_logs_cycles_payouts_chunk.len() < DO_TRADE_LOGS_CYCLES_PAYOUTS_CHUNK_SIZE {
                tl.cycles_payout_lock = true;    
                trade_logs_cycles_payouts_chunk.push(
                    (
                        tl.id,
                        do_cycles_payout(tl.clone())
                    )
                );
            }
            if tl.token_payout_data.is_complete() == false
            && tl.token_payout_lock == false
            && trade_logs_token_payouts_chunk.len() < DO_TRADE_LOGS_TOKEN_PAYOUTS_CHUNK_SIZE {
                tl.token_payout_lock = true;
                trade_logs_token_payouts_chunk.push(
                    (
                        tl.id,
                        do_token_payout(tl.clone())                        
                    )
                );
            }
            i += 1;
        }
        
    });

    let (vcps_ids_cycles_payouts, vcps_do_cycles_payouts_futures): (Vec<VoidCyclesPositionId>, Vec<_/*do_cycles_payout-future*/>) = void_cycles_positions_cycles_payouts_chunk.into_iter().unzip();
    let (vips_ids_token_payouts, vips_do_token_payouts_futures): (Vec<VoidTokenPositionId>, Vec<_/*do_token_payout-future*/>) = void_token_positions_token_payouts_chunk.into_iter().unzip();
    
    let (vcps_ids_update_storage_positions, vcps_do_update_storage_positions_futures): (Vec<VoidCyclesPositionId>, Vec<_/*do_update_storage_position-future*/>) = void_cycles_positions_update_storage_positions_chunk.into_iter().unzip();
    let (vips_ids_update_storage_positions, vips_do_update_storage_positions_futures): (Vec<VoidTokenPositionId>, Vec<_/*do_update_storage_position-future*/>) = void_token_positions_update_storage_positions_chunk.into_iter().unzip();
    
    let (tls_cycles_payouts_ids, tls_do_cycles_payouts_futures): (Vec<PurchaseId>, Vec<_/*do_cycles_payout-future*/>) = trade_logs_cycles_payouts_chunk.into_iter().unzip();
    let (tls_token_payouts_ids, tls_do_token_payouts_futures): (Vec<PurchaseId>, Vec<_/*do_token_payout-future*/>) = trade_logs_token_payouts_chunk.into_iter().unzip();
    
    let (
        vcps_do_cycles_payouts_rs,
        vips_do_token_payouts_rs,
        vcps_do_update_storage_positions_rs,
        vips_do_update_storage_positions_rs,
        tls_do_cycles_payouts_rs,
        tls_do_token_payouts_rs,
    ): (
        Vec<DoCyclesPayoutResult>,
        Vec<DoTokenPayoutSponse>,
        Vec<DoUpdateStoragePositionResult>,
        Vec<DoUpdateStoragePositionResult>,
        Vec<DoCyclesPayoutResult>,
        Vec<DoTokenPayoutSponse>,
    ) = futures::join!(
        futures::future::join_all(vcps_do_cycles_payouts_futures),
        futures::future::join_all(vips_do_token_payouts_futures),
        futures::future::join_all(vcps_do_update_storage_positions_futures),
        futures::future::join_all(vips_do_update_storage_positions_futures),
        futures::future::join_all(tls_do_cycles_payouts_futures),
        futures::future::join_all(tls_do_token_payouts_futures),
    );

    with_mut(&CM_DATA, |cm_data| {
        fn handle_vps_payouts<VoidPosition: VoidPositionTrait, DoPayoutOutput, F: Fn(&mut VoidPosition::PayoutData, DoPayoutOutput)->()>(
            vps_ids_payouts: Vec<PositionId>,
            vps_do_payouts_rs: Vec<DoPayoutOutput>,
            void_positions: &mut Vec<VoidPosition>, 
            handle_payout_output: F
        ) {
            for (vp_id, do_payout_output) in vps_ids_payouts.into_iter().zip(vps_do_payouts_rs.into_iter()) {      
                let vp_void_positions_i: usize = match void_positions.binary_search_by_key(&vp_id, |vp| { vp.position_id() }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                };
                let vp: &mut VoidPosition = &mut void_positions[vp_void_positions_i];
                *vp.payout_lock() = false;
                handle_payout_output(vp.payout_data(), do_payout_output);
                if vp.can_remove() {
                    void_positions.remove(vp_void_positions_i);
                }
            }
        }
        handle_vps_payouts(
            vcps_ids_cycles_payouts,
            vcps_do_cycles_payouts_rs,
            &mut cm_data.void_cycles_positions,
            handle_do_cycles_payout_result
        );
        handle_vps_payouts(
            vips_ids_token_payouts,
            vips_do_token_payouts_rs,
            &mut cm_data.void_token_positions,
            handle_do_token_payout_sponse
        );
        
        fn handle_vps_update_storage_positions<VoidPosition: VoidPositionTrait>(
            vps_ids_update_storage_positions: Vec<PositionId>,
            vps_do_update_storage_positions_rs: Vec<DoUpdateStoragePositionResult>,
            void_positions: &mut Vec<VoidPosition>, 
        ) {
            for (vp_id, do_update_storage_position_result) in vps_ids_update_storage_positions.into_iter().zip(vps_do_update_storage_positions_rs.into_iter()) {      
                let vp_void_positions_i: usize = match void_positions.binary_search_by_key(&vp_id, |vp| { vp.position_id() }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                };
                let vp: &mut VoidPosition = &mut void_positions[vp_void_positions_i];
                vp.update_storage_position_data().lock = false;
                vp.update_storage_position_data().status = do_update_storage_position_result.is_ok();
                if vp.can_remove() {
                    void_positions.remove(vp_void_positions_i);
                }
            }
        }
        handle_vps_update_storage_positions(
            vcps_ids_update_storage_positions,
            vcps_do_update_storage_positions_rs,
            &mut cm_data.void_cycles_positions,
        );
        handle_vps_update_storage_positions(
            vips_ids_update_storage_positions,
            vips_do_update_storage_positions_rs,
            &mut cm_data.void_token_positions,
        );
        
        let mut tl_payouts: BTreeMap<PurchaseId, (Option<DoCyclesPayoutResult>, Option<DoTokenPayoutSponse>)> = BTreeMap::new();
        for (tl_id, do_cycles_payout_result) in tls_cycles_payouts_ids.into_iter().zip(tls_do_cycles_payouts_rs.into_iter()) {
            tl_payouts.entry(tl_id)
            .or_default()
            .0 = Some(do_cycles_payout_result);
        } 
        for (tl_id, do_token_payout_sponse) in tls_token_payouts_ids.into_iter().zip(tls_do_token_payouts_rs.into_iter()) {
            tl_payouts.entry(tl_id)
            .or_default()
            .1 = Some(do_token_payout_sponse);
        }
        for (tl_id, (opt_cycles_payout_output, opt_token_payout_output)) in tl_payouts.into_iter() {
            let tl_trade_logs_i: usize = match cm_data.trade_logs.binary_search_by_key(&tl_id, |tl| { tl.id }) {
                Ok(i) => i,
                Err(_) => { continue; }    
            };
            let tl: &mut TradeLog = &mut cm_data.trade_logs[tl_trade_logs_i];
            if let Some(cycles_payout_output) = opt_cycles_payout_output {
                tl.cycles_payout_lock = false;
                handle_do_cycles_payout_result(&mut tl.cycles_payout_data, cycles_payout_output);    
            } 
            if let Some(token_payout_output) = opt_token_payout_output {
                tl.token_payout_lock = false;
                handle_do_token_payout_sponse(&mut tl.token_payout_data, token_payout_output);    
            }
        }
    });
    
}



fn handle_do_cycles_payout_result(cpd: &mut CyclesPayoutData, do_cycles_payout_result: DoCyclesPayoutResult) {
    if let Ok(do_cycles_payout_sponse) = do_cycles_payout_result {  
        match do_cycles_payout_sponse {
            DoCyclesPayoutSponse::CyclesPayoutSuccess => {
                cpd.cycles_payout = true;                            
            },
            DoCyclesPayoutSponse::NothingToDo => {}
        }
    }
}

fn handle_do_token_payout_sponse(tpd: &mut TokenPayoutData, sponse: TokenPayoutData) {
    tpd.token_transfer = sponse.token_transfer;
    tpd.token_fee_collection = sponse.token_fee_collection;
    tpd.cm_message_call = sponse.cm_message_call;
}
