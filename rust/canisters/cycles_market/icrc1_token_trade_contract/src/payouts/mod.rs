

use crate::*;

mod cycles_payouts;
use cycles_payouts::*;


mod token_payouts;
use token_payouts::*;






pub async fn _do_payouts() {

    let mut void_cycles_positions_cycles_payouts_chunk: Vec<(VoidCyclesPositionId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new();
    let mut void_token_positions_token_payouts_chunk: Vec<(VoidTokenPositionId, _/*anonymous-future of the do_token_payout-async-function*/)> = Vec::new();
    let mut cycles_positions_purchases_cycles_payouts_chunk: Vec<(CyclesPositionPurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut cycles_positions_purchases_token_payouts_chunk: Vec<(CyclesPositionPurchaseId, _/*anonymous-future of the token_transfer-function*/)> = Vec::new();
    let mut token_positions_purchases_cycles_payouts_chunk: Vec<(TokenPositionPurchaseId, _/*anonymous-future of the do_cycles_payout-async-function*/)> = Vec::new(); 
    let mut token_positions_purchases_token_payouts_chunk: Vec<(TokenPositionPurchaseId, _/*anonymous-future of the token_transfer-function*/)> = Vec::new();

    with_mut(&CM_DATA, |cm_data| {
        let mut i: usize = 0;
        while i < cm_data.void_cycles_positions.len() && void_cycles_positions_cycles_payouts_chunk.len() < DO_VOID_CYCLES_POSITIONS_CYCLES_PAYOUTS_CHUNK_SIZE {
            let vcp: &mut VoidCyclesPosition = &mut cm_data.void_cycles_positions[i];
            if vcp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() {
                if time_nanos().saturating_sub(vcp.cycles_payout_data.cmcaller_cycles_payout_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK {
                    std::mem::drop(vcp);
                    cm_data.void_cycles_positions.remove(i);
                    continue;
                }
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
                if time_nanos().saturating_sub(vip.token_payout_data.cm_message_call_success_timestamp_nanos.unwrap()) > MAX_WAIT_TIME_NANOS_FOR_A_CM_CALLER_CALLBACK {
                    std::mem::drop(vip);
                    cm_data.void_token_positions.remove(i);
                    continue;
                }
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
        while i < cm_data.cycles_positions_purchases.len() {
            let cpp: &mut CyclesPositionPurchase = &mut cm_data.cycles_positions_purchases[i];                    
            if cpp.cycles_payout_data.is_complete() == false 
            && cpp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() == false
            && cpp.cycles_payout_lock == false
            && cycles_positions_purchases_cycles_payouts_chunk.len() < DO_CYCLES_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE {
                cpp.cycles_payout_lock = true;    
                cycles_positions_purchases_cycles_payouts_chunk.push(
                    (
                        cpp.id,
                        do_cycles_payout(cpp.clone())
                    )
                );
            }
            if cpp.token_payout_data.is_complete() == false
            && cpp.token_payout_data.is_waiting_for_the_cmcaller_callback() == false
            && cpp.token_payout_lock == false
            && cycles_positions_purchases_token_payouts_chunk.len() < DO_CYCLES_POSITIONS_PURCHASES_TOKEN_PAYOUTS_CHUNK_SIZE {
                cpp.token_payout_lock = true;
                cycles_positions_purchases_token_payouts_chunk.push(
                    (     
                        cpp.id,
                        do_token_payout(cpp.clone())                        
                    )
                );
            }
            i += 1;
        }
        
        let mut i: usize = 0;
        while i < cm_data.token_positions_purchases.len() {
            let ipp: &mut TokenPositionPurchase = &mut cm_data.token_positions_purchases[i];                    
            if ipp.cycles_payout_data.is_complete() == false 
            && ipp.cycles_payout_data.is_waiting_for_the_cycles_transferrer_transfer_cycles_callback() == false
            && ipp.cycles_payout_lock == false
            && token_positions_purchases_cycles_payouts_chunk.len() < DO_TOKEN_POSITIONS_PURCHASES_CYCLES_PAYOUTS_CHUNK_SIZE {
                ipp.cycles_payout_lock = true;
                token_positions_purchases_cycles_payouts_chunk.push(
                    (
                        ipp.id,
                        do_cycles_payout(ipp.clone())
                    )
                );
            }
            if ipp.token_payout_data.is_complete() == false
            && ipp.token_payout_data.is_waiting_for_the_cmcaller_callback() == false
            && ipp.token_payout_lock == false                                                        
            && token_positions_purchases_token_payouts_chunk.len() < DO_TOKEN_POSITIONS_PURCHASES_TOKEN_PAYOUTS_CHUNK_SIZE {
                ipp.token_payout_lock = true;
                token_positions_purchases_token_payouts_chunk.push(
                    (     
                        ipp.id,
                        do_token_payout(ipp.clone())
                    )
                );
            }
            i += 1;
        }
        
    });

    let (vcps_ids, vcps_do_cycles_payouts_futures): (Vec<VoidCyclesPositionId>, Vec<_/*do_cycles_payout-future*/>) = void_cycles_positions_cycles_payouts_chunk.into_iter().unzip();
    let (vips_ids, vips_do_token_payouts_futures): (Vec<VoidTokenPositionId>, Vec<_/*do_token_payout-future*/>) = void_token_positions_token_payouts_chunk.into_iter().unzip();
    let (cpps_cycles_payouts_ids, cpps_do_cycles_payouts_futures): (Vec<CyclesPositionPurchaseId>, Vec<_/*do_cycles_payout-future*/>) = cycles_positions_purchases_cycles_payouts_chunk.into_iter().unzip();
    let (cpps_token_payouts_ids, cpps_do_token_payouts_futures): (Vec<CyclesPositionPurchaseId>, Vec<_/*do_token_payout-future*/>) = cycles_positions_purchases_token_payouts_chunk.into_iter().unzip();
    let (ipps_cycles_payouts_ids, ipps_do_cycles_payouts_futures): (Vec<TokenPositionPurchaseId>, Vec<_/*do_cycles_payout-future*/>) = token_positions_purchases_cycles_payouts_chunk.into_iter().unzip();
    let (ipps_token_payouts_ids, ipps_do_token_payouts_futures): (Vec<TokenPositionPurchaseId>, Vec<_/*do_token_payout-future*/>) = token_positions_purchases_token_payouts_chunk.into_iter().unzip();
    
    let (
        vcps_do_cycles_payouts_rs,
        vips_do_token_payouts_rs,
        cpps_do_cycles_payouts_rs,
        cpps_do_token_payouts_rs,
        ipps_do_cycles_payouts_rs,
        ipps_do_token_payouts_rs
    ): (
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoTokenPayoutSponse>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoTokenPayoutSponse>,
        Vec<Result<DoCyclesPayoutSponse, DoCyclesPayoutError>>,
        Vec<DoTokenPayoutSponse>,
    ) = futures::join!(
        futures::future::join_all(vcps_do_cycles_payouts_futures),
        futures::future::join_all(vips_do_token_payouts_futures),
        futures::future::join_all(cpps_do_cycles_payouts_futures),
        futures::future::join_all(cpps_do_token_payouts_futures),
        futures::future::join_all(ipps_do_cycles_payouts_futures),
        futures::future::join_all(ipps_do_token_payouts_futures),
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
        for (cpp_id, do_cycles_payout_result) in cpps_cycles_payouts_ids.into_iter().zip(cpps_do_cycles_payouts_rs.into_iter()) {
            let cpp_cycles_positions_purchases_i: usize = {
                match cm_data.cycles_positions_purchases.binary_search_by_key(&cpp_id, |cpp| { cpp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let cpp: &mut CyclesPositionPurchase = &mut cm_data.cycles_positions_purchases[cpp_cycles_positions_purchases_i];
            cpp.cycles_payout_lock = false;
            handle_do_cycles_payout_result(&mut cpp.cycles_payout_data, do_cycles_payout_result);
        }
        for (cpp_id, do_token_payout_sponse) in cpps_token_payouts_ids.into_iter().zip(cpps_do_token_payouts_rs.into_iter()) {
            let cpp_cycles_positions_purchases_i: usize = {
                match cm_data.cycles_positions_purchases.binary_search_by_key(&cpp_id, |cpp| { cpp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let cpp: &mut CyclesPositionPurchase = &mut cm_data.cycles_positions_purchases[cpp_cycles_positions_purchases_i];
            cpp.token_payout_lock = false;
            handle_do_token_payout_sponse(&mut cpp.token_payout_data, do_token_payout_sponse);
        }
        for (ipp_id, do_cycles_payout_result) in ipps_cycles_payouts_ids.into_iter().zip(ipps_do_cycles_payouts_rs.into_iter()) {
            let ipp_token_positions_purchases_i: usize = {
                match cm_data.token_positions_purchases.binary_search_by_key(&ipp_id, |ipp| { ipp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let ipp: &mut TokenPositionPurchase = &mut cm_data.token_positions_purchases[ipp_token_positions_purchases_i];
            ipp.cycles_payout_lock = false;
            handle_do_cycles_payout_result(&mut ipp.cycles_payout_data, do_cycles_payout_result);
        }
        for (ipp_id, do_token_payout_sponse) in ipps_token_payouts_ids.into_iter().zip(ipps_do_token_payouts_rs.into_iter()) {
            let ipp_token_positions_purchases_i: usize = {
                match cm_data.token_positions_purchases.binary_search_by_key(&ipp_id, |ipp| { ipp.id }) {
                    Ok(i) => i,
                    Err(_) => { continue; }
                }
            };
            let ipp: &mut TokenPositionPurchase = &mut cm_data.token_positions_purchases[ipp_token_positions_purchases_i];
            ipp.token_payout_lock = false;
            handle_do_token_payout_sponse(&mut ipp.token_payout_data, do_token_payout_sponse);
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

fn handle_do_token_payout_sponse(tpd: &mut TokenPayoutData, do_token_payout_sponse: DoTokenPayoutSponse) {
    match do_token_payout_sponse {
        DoTokenPayoutSponse::TokenTransferError(TokenTransferErrorType) => {
            
        },
        DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageError(token_transfer, _cm_message_error_type) => {
            tpd.token_transfer = Some(token_transfer);
        },
        DoTokenPayoutSponse::TokenTransferSuccessAndCMMessageSuccess(token_transfer, cm_message_call_success_timestamp_nanos) => {
            tpd.token_transfer = Some(token_transfer);
            tpd.cm_message_call_success_timestamp_nanos = Some(cm_message_call_success_timestamp_nanos);
        },
        DoTokenPayoutSponse::NothingForTheDo => {},
    }
}
