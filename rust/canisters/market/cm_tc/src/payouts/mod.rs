mod update_storage_positions;
use update_storage_positions::DoUpdateStoragePositionResult;

mod void_positions_payouts;
use void_positions_payouts::void_positions_payouts;

mod do_payout;
use do_payout::{
    do_cycles_payout,
    do_token_payout,
    DoPayoutQuest,
};

use std::collections::BTreeMap;

use crate::{
    CM_DATA,
    POSITIONS_STORAGE_DATA,
    TRADES_STORAGE_DATA,
    DO_TRADE_LOGS_CYCLES_PAYOUTS_CHUNK_SIZE,
    DO_TRADE_LOGS_TOKEN_PAYOUTS_CHUNK_SIZE,
    flush_logs::flush_logs,
    transfer_memo::create_trade_transfer_memo,
    traits::VoidPositionTrait,
};

use cts_lib::{
    icrc::IcrcId,
    types::cm::tc::{
        VoidCyclesPositionId,
        VoidTokenPositionId,
        PositionId,
        PurchaseId,
        PositionKind,
        PayoutData,
        TradeLogTemporaryData,
        storage_logs::{
            StorageLogTrait,
            trade_log::TradeLog,
        }
    },
    tools::{
        call_error_as_u32_and_string,
        localkey::refcell::{with, with_mut},
    }
};

use ic_cdk::{
    call,
    api::call::reply,

};



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
                            trades_storage_data.storage_buffer.extend(cm_data.trade_logs.pop_front().unwrap().log.stable_memory_serialize());
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
            ic_cdk::print(&format!("payout error: {:?}", call_error));
            with_mut(&CM_DATA, |cm_data| {
                cm_data.do_payouts_errors.push(call_error_as_u32_and_string(call_error));
            });
        }
    }
    
    if with(&CM_DATA, |cm_data| {
        cm_data.void_cycles_positions.len() != 0
        || cm_data.void_token_positions.len() != 0
        || cm_data.trade_logs.len() != 0
    }) {
        fn spawn_do_payouts() { ic_cdk::spawn(do_payouts()); }
        ic_cdk_timers::set_timer(core::time::Duration::from_secs(30), spawn_do_payouts);
    }
}

#[export_name = "canister_update do_payouts_public_method"]
pub extern "C" fn do_payouts_public_method() {
    ic_cdk::spawn(async {
        _do_payouts().await;
        reply::<()>(());    
    });
}





async fn _do_payouts() {

    let mut void_cycles_positions_cycles_payouts_chunk: Vec<(VoidCyclesPositionId, _)> = Vec::new();
    let mut void_token_positions_token_payouts_chunk: Vec<(VoidTokenPositionId, _)> = Vec::new();
    let mut void_cycles_positions_update_storage_positions_chunk: Vec<(VoidCyclesPositionId, _)> = Vec::new();
    let mut void_token_positions_update_storage_positions_chunk: Vec<(VoidTokenPositionId, _)> = Vec::new();    
    let mut trade_logs_cycles_payouts_chunk: Vec<(PurchaseId, _)> = Vec::new(); 
    let mut trade_logs_token_payouts_chunk: Vec<(PurchaseId, _)> = Vec::new();
    
    with_mut(&CM_DATA, |cm_data| {
        
        let update_storage_positions_yes_or_no: bool = with(&POSITIONS_STORAGE_DATA, |positions_storage_data| { !positions_storage_data.storage_flush_lock }); 
        
        (void_cycles_positions_cycles_payouts_chunk, void_cycles_positions_update_storage_positions_chunk) 
            = void_positions_payouts(&mut cm_data.void_cycles_positions, do_cycles_payout, update_storage_positions_yes_or_no);
        
        (void_token_positions_token_payouts_chunk, void_token_positions_update_storage_positions_chunk) 
            = void_positions_payouts(&mut cm_data.void_token_positions, do_token_payout, update_storage_positions_yes_or_no);
                        
        if void_cycles_positions_update_storage_positions_chunk.len() > 0 
        || void_token_positions_update_storage_positions_chunk.len() > 0 {
            with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| { 
                positions_storage_data.storage_flush_lock = true; 
            });
        }
        
        let mut i: usize = 0;
        while i < cm_data.trade_logs.len() {
            let (tl, tl_temp): (&mut TradeLog, &mut TradeLogTemporaryData) = {
                let tl_and_temp = &mut cm_data.trade_logs[i];
                (&mut tl_and_temp.log, &mut tl_and_temp.temporary_data)
            };
            if tl.cycles_payout_data.is_none() 
            && tl_temp.cycles_payout_lock == false
            && trade_logs_cycles_payouts_chunk.len() < DO_TRADE_LOGS_CYCLES_PAYOUTS_CHUNK_SIZE {
                tl_temp.cycles_payout_lock = true;    
                trade_logs_cycles_payouts_chunk.push(
                    (
                        tl.id,
                        do_cycles_payout(DoPayoutQuest{
                            payee: IcrcId{
                                owner: match tl.matchee_position_kind { 
                                    PositionKind::Cycles => tl.matcher_position_positor,
                                    PositionKind::Token => tl.matchee_position_positor,
                                },
                                subaccount: tl_temp.payout_cycles_to_subaccount,
                            },
                            trade_mount: tl.cycles,
                            cts_payout_fee: tl.cycles_payout_fee,
                            memo: create_trade_transfer_memo(tl.id),
                        })
                    )
                );
            }
            if tl.token_payout_data.is_none()
            && tl_temp.token_payout_lock == false
            && trade_logs_token_payouts_chunk.len() < DO_TRADE_LOGS_TOKEN_PAYOUTS_CHUNK_SIZE {
                tl_temp.token_payout_lock = true;
                trade_logs_token_payouts_chunk.push(
                    (
                        tl.id,
                        do_token_payout(DoPayoutQuest{
                            payee: IcrcId{
                                owner: match tl.matchee_position_kind { 
                                    PositionKind::Cycles => tl.matchee_position_positor,
                                    PositionKind::Token => tl.matcher_position_positor,
                                },
                                subaccount: tl_temp.payout_tokens_to_subaccount,
                            },
                            trade_mount: tl.tokens,
                            cts_payout_fee: tl.tokens_payout_fee,
                            memo: create_trade_transfer_memo(tl.id)
                        })
                    )
                );
            }
            i += 1;
        }
        
    });

    let (vcps_ids_cycles_payouts, vcps_do_cycles_payouts_futures): (Vec<VoidCyclesPositionId>, Vec<_>) = void_cycles_positions_cycles_payouts_chunk.into_iter().unzip();
    let (vips_ids_token_payouts, vips_do_token_payouts_futures): (Vec<VoidTokenPositionId>, Vec<_>) = void_token_positions_token_payouts_chunk.into_iter().unzip();
    
    let (vcps_ids_update_storage_positions, vcps_do_update_storage_positions_futures): (Vec<VoidCyclesPositionId>, Vec<_>) = void_cycles_positions_update_storage_positions_chunk.into_iter().unzip();
    let (vips_ids_update_storage_positions, vips_do_update_storage_positions_futures): (Vec<VoidTokenPositionId>, Vec<_>) = void_token_positions_update_storage_positions_chunk.into_iter().unzip();
    
    let (tls_cycles_payouts_ids, tls_do_cycles_payouts_futures): (Vec<PurchaseId>, Vec<_>) = trade_logs_cycles_payouts_chunk.into_iter().unzip();
    let (tls_token_payouts_ids, tls_do_token_payouts_futures): (Vec<PurchaseId>, Vec<_>) = trade_logs_token_payouts_chunk.into_iter().unzip();
    
    let (
        vcps_do_cycles_payouts_rs,
        vips_do_token_payouts_rs,
        vcps_do_update_storage_positions_rs,
        vips_do_update_storage_positions_rs,
        tls_do_cycles_payouts_rs,
        tls_do_token_payouts_rs,
    ): (
        Vec<Option<PayoutData>>,
        Vec<Option<PayoutData>>,
        Vec<DoUpdateStoragePositionResult>,
        Vec<DoUpdateStoragePositionResult>,
        Vec<Option<PayoutData>>,
        Vec<Option<PayoutData>>,
    ) = futures::join!(
        futures::future::join_all(vcps_do_cycles_payouts_futures),
        futures::future::join_all(vips_do_token_payouts_futures),
        futures::future::join_all(vcps_do_update_storage_positions_futures),
        futures::future::join_all(vips_do_update_storage_positions_futures),
        futures::future::join_all(tls_do_cycles_payouts_futures),
        futures::future::join_all(tls_do_token_payouts_futures),
    );

    if vcps_ids_update_storage_positions.len() > 0 
    || vips_ids_update_storage_positions.len() > 0 {
        with_mut(&POSITIONS_STORAGE_DATA, |positions_storage_data| { 
            positions_storage_data.storage_flush_lock = false; 
        });
    }

    with_mut(&CM_DATA, |cm_data| {
        fn _handle_vps<VoidPosition: VoidPositionTrait, DoOutput, F: Fn(&mut VoidPosition, DoOutput)->()>(
            vps_ids: Vec<PositionId>,
            vps_do_rs: Vec<DoOutput>,
            void_positions: &mut BTreeMap<PositionId, VoidPosition>, 
            handle_output: F
        ) {
            for (vp_id, do_output) in vps_ids.into_iter().zip(vps_do_rs.into_iter()) {      
                let vp: &mut VoidPosition = match void_positions.get_mut(&vp_id) {
                    Some(vp) => vp,
                    None => continue,
                };
                handle_output(vp, do_output);
                if vp.can_remove() {
                    void_positions.remove(&vp_id);
                }
            }
        }
        fn handle_vps_payouts<VoidPosition: VoidPositionTrait>(
            vps_ids_payouts: Vec<PositionId>,
            vps_do_payouts_rs: Vec<Option<PayoutData>>,
            void_positions: &mut BTreeMap<PositionId, VoidPosition>, 
        ) {
            _handle_vps(
                vps_ids_payouts,
                vps_do_payouts_rs,
                void_positions,
                |vp, do_payout_output| {
                    *vp.payout_lock() = false;
                    *vp.payout_data_mut() = do_payout_output;
                    if let Some(ref pd) = do_payout_output {
                        vp.update_storage_position_data_mut().update_storage_position_log.void_position_payout_dust_collection = pd.did_transfer == false;     
                        vp.update_storage_position_data_mut().update_storage_position_log.void_position_payout_ledger_transfer_fee = pd.ledger_transfer_fee as u64;                        
                    } 
                }
            );
        }
        handle_vps_payouts(
            vcps_ids_cycles_payouts,
            vcps_do_cycles_payouts_rs,
            &mut cm_data.void_cycles_positions,
        );
        handle_vps_payouts(
            vips_ids_token_payouts,
            vips_do_token_payouts_rs,
            &mut cm_data.void_token_positions,
        );
        fn handle_vps_update_storage_positions<VoidPosition: VoidPositionTrait>(
            vps_ids_update_storage_positions: Vec<PositionId>,
            vps_do_update_storage_positions_rs: Vec<DoUpdateStoragePositionResult>,
            void_positions: &mut BTreeMap<PositionId, VoidPosition>, 
        ) {
            _handle_vps(
                vps_ids_update_storage_positions,
                vps_do_update_storage_positions_rs,
                void_positions,
                |vp, do_update_storage_position_output| {
                    vp.update_storage_position_data_mut().lock = false;
                    vp.update_storage_position_data_mut().status = do_update_storage_position_output.is_ok();
                }
            );
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
        
        let mut tl_payouts: BTreeMap<PurchaseId, (Option<Option<PayoutData>>, Option<Option<PayoutData>>)> = BTreeMap::new();
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
        for (tl_id, (possible_cycles_payout_output, possible_token_payout_output)) in tl_payouts.into_iter() {
            let tl_trade_logs_i: usize = match cm_data.trade_logs.binary_search_by_key(&tl_id, |tl| { tl.log.id }) {
                Ok(i) => i,
                Err(_) => { continue; }    
            };            
            let (tl, tl_temp): (&mut TradeLog, &mut TradeLogTemporaryData) = {
                let tl_and_temp = &mut cm_data.trade_logs[tl_trade_logs_i];
                (&mut tl_and_temp.log, &mut tl_and_temp.temporary_data)
            };
            if let Some(cycles_payout_data) = possible_cycles_payout_output {
                tl_temp.cycles_payout_lock = false;
                tl.cycles_payout_data = cycles_payout_data;    
            } 
            if let Some(token_payout_data) = possible_token_payout_output {
                tl_temp.token_payout_lock = false;
                tl.token_payout_data = token_payout_data;    
            }
        }
    });
    
}



