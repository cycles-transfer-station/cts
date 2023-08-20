use crate::*;

mod cycles_payouts;
use cycles_payouts::*;

mod token_payouts;
use token_payouts::*;

use flush_logs::flush_trade_logs;






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
            flush_trade_logs().await;              
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
    let mut trade_logs_cycles_payouts_chunk: Vec<(PurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut trade_logs_token_payouts_chunk: Vec<(PurchaseId, _/*anonymous-future of the do_token_payout-async-function*/)> = Vec::new();
    
    with_mut(&CM_DATA, |cm_data| {
        let mut i: usize = 0;
        while i < cm_data.void_cycles_positions.len() && void_cycles_positions_cycles_payouts_chunk.len() < DO_VOID_CYCLES_POSITIONS_CYCLES_PAYOUTS_CHUNK_SIZE {
            let vcp: &mut VoidCyclesPosition = &mut cm_data.void_cycles_positions[i];
            if vcp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() {
                // skip
            } else if vcp.cycles_payout_lock == true { 
                // skip
            } else {
                vcp.cycles_payout_lock = true;
                void_cycles_positions_cycles_payouts_chunk.push(
                    (
                        vcp.position_id,
                        do_cycles_payout(vcp.clone())
                    )
                );
            }
            i += 1;
        }
        
        let mut i: usize = 0;
        while i < cm_data.void_token_positions.len() && void_token_positions_token_payouts_chunk.len() < DO_VOID_TOKEN_POSITIONS_TOKEN_PAYOUTS_CHUNK_SIZE {
            let vip: &mut VoidTokenPosition = &mut cm_data.void_token_positions[i];
            if vip.token_payout_data.is_waiting_for_the_cmcaller_callback() {
                // skip
            } else if vip.token_payout_lock == true { 
                // skip
            } else {
                vip.token_payout_lock = true;
                void_token_positions_token_payouts_chunk.push(
                    (
                        vip.position_id,
                        do_token_payout(vip.clone())
                    )
                );
            }
            i += 1;
        }


        let mut i: usize = 0;
        while i < cm_data.trade_logs.len() {
            let tl: &mut TradeLog = &mut cm_data.trade_logs[i];                    
            if tl.cycles_payout_data.is_complete() == false 
            && tl.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() == false
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
            && tl.token_payout_data.is_waiting_for_the_cmcaller_callback() == false
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

    let (vcps_ids, vcps_do_cycles_payouts_futures): (Vec<VoidCyclesPositionId>, Vec<_/*do_cycles_payout-future*/>) = void_cycles_positions_cycles_payouts_chunk.into_iter().unzip();
    let (vips_ids, vips_do_token_payouts_futures): (Vec<VoidTokenPositionId>, Vec<_/*do_token_payout-future*/>) = void_token_positions_token_payouts_chunk.into_iter().unzip();
    let (tls_cycles_payouts_ids, tls_do_cycles_payouts_futures): (Vec<PurchaseId>, Vec<_/*do_cycles_payout-future*/>) = trade_logs_cycles_payouts_chunk.into_iter().unzip();
    let (tls_token_payouts_ids, tls_do_token_payouts_futures): (Vec<PurchaseId>, Vec<_/*do_token_payout-future*/>) = trade_logs_token_payouts_chunk.into_iter().unzip();
    
    let (
        vcps_do_cycles_payouts_rs,
        vips_do_token_payouts_rs,
        tls_do_cycles_payouts_rs,
        tls_do_token_payouts_rs,
    ): (
        Vec<DoCyclesPayoutResult>,
        Vec<DoTokenPayoutSponse>,
        Vec<DoCyclesPayoutResult>,
        Vec<DoTokenPayoutSponse>,
    ) = futures::join!(
        futures::future::join_all(vcps_do_cycles_payouts_futures),
        futures::future::join_all(vips_do_token_payouts_futures),
        futures::future::join_all(tls_do_cycles_payouts_futures),
        futures::future::join_all(tls_do_token_payouts_futures),
    );

    with_mut(&CM_DATA, |cm_data| {
        for (vcp_id, do_cycles_payout_result) in vcps_ids.into_iter().zip(vcps_do_cycles_payouts_rs.into_iter()) {      
            let vcp_void_cycles_positions_i: usize = {
                match cm_data.void_cycles_positions.binary_search_by_key(&vcp_id, |vcp| { vcp.position_id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }  
            };
            let vcp: &mut VoidCyclesPosition = &mut cm_data.void_cycles_positions[vcp_void_cycles_positions_i];
            vcp.cycles_payout_lock = false;
            handle_do_cycles_payout_result(&mut vcp.cycles_payout_data, do_cycles_payout_result);
            if vcp.cycles_payout_data.is_complete() {
                std::mem::drop(vcp);
                cm_data.void_cycles_positions.remove(vcp_void_cycles_positions_i);
            }
        }
        for (vip_id, do_token_payout_sponse) in vips_ids.into_iter().zip(vips_do_token_payouts_rs.into_iter()) {      
            let vip_void_token_positions_i: usize = {
                match cm_data.void_token_positions.binary_search_by_key(&vip_id, |vip| { vip.position_id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }  
            };
            let vip: &mut VoidTokenPosition = &mut cm_data.void_token_positions[vip_void_token_positions_i];
            vip.token_payout_lock = false;
            handle_do_token_payout_sponse(&mut vip.token_payout_data, do_token_payout_sponse);
            if vip.token_payout_data.is_complete() {
                std::mem::drop(vip);
                cm_data.void_token_positions.remove(vip_void_token_positions_i);
            }
        }
        for (tl_id, do_cycles_payout_result) in tls_cycles_payouts_ids.into_iter().zip(tls_do_cycles_payouts_rs.into_iter()) {
            let tl_trade_logs_i: usize = {
                match cm_data.trade_logs.binary_search_by_key(&tl_id, |tl| { tl.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let tl: &mut TradeLog = &mut cm_data.trade_logs[tl_trade_logs_i];
            tl.cycles_payout_lock = false;
            handle_do_cycles_payout_result(&mut tl.cycles_payout_data, do_cycles_payout_result);
        }
        for (tl_id, do_token_payout_sponse) in tls_token_payouts_ids.into_iter().zip(tls_do_token_payouts_rs.into_iter()) {
            let tl_trade_logs_i: usize = {
                match cm_data.trade_logs.binary_search_by_key(&tl_id, |tl| { tl.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let tl: &mut TradeLog = &mut cm_data.trade_logs[tl_trade_logs_i];
            tl.token_payout_lock = false;
            handle_do_token_payout_sponse(&mut tl.token_payout_data, do_token_payout_sponse);
        }        
    });
    
}



fn handle_do_cycles_payout_result(cpd: &mut CyclesPayoutData, do_cycles_payout_result: DoCyclesPayoutResult) {
    if let Ok(do_cycles_payout_sponse) = do_cycles_payout_result {  
        match do_cycles_payout_sponse {
            DoCyclesPayoutSponse::CMCallerCyclesPayoutCallSuccessTimestampNanos(opt_timestamp_ns) => {
                cpd.cmcaller_cycles_payout_call_success_timestamp_nanos = opt_timestamp_ns;                            
            },
            DoCyclesPayoutSponse::ManagementCanisterPositCyclesCallSuccess(management_canister_posit_cycles_call_success) => {
                cpd.management_canister_posit_cycles_call_success = management_canister_posit_cycles_call_success;
            },
            DoCyclesPayoutSponse::NothingToDo => {}
        }
    }
}

// we use this function (instead of replacing the whole token_payout_data) 
// cause the cm_caller-cm_call manual-callback can come back before this output is put back on the purchase/vcp. 
// so we use this function so that only the fields that the do_token_payout fn sets get re-place.
fn handle_do_token_payout_sponse(tpd: &mut TokenPayoutData, sponse: TokenPayoutData) {
    tpd.token_transfer = sponse.token_transfer;
    tpd.token_fee_collection = sponse.token_fee_collection;
    tpd.cm_message_call_success_timestamp_nanos = sponse.cm_message_call_success_timestamp_nanos;
}
