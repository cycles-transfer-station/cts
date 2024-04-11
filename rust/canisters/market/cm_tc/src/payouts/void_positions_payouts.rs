use super::*;


pub fn void_positions_payouts<VoidPosition: VoidPositionTrait, DoPayoutFuture: Future<Output=Option<PayoutData>>, F: Fn(DoPayoutQuest)->DoPayoutFuture>(
    void_positions: &mut Vec<VoidPosition>, 
    do_payout_fn: F,
    update_storage_positions_yes_or_no: bool,
) 
-> 
(
    Vec<(PositionId, impl Future<Output=Option<PayoutData>>)>/*payouts_chunk*/, 
    Vec<(PositionId, impl Future<Output=DoUpdateStoragePositionResult>)>/*update_storage_positions_chunk*/
) 
{
    let mut payouts_chunk: Vec<(PositionId, _)> = Vec::new();
    let mut update_storage_positions_chunk: Vec<(PositionId, _)> = Vec::new();
    
    let mut i: usize = 0;
    while i < void_positions.len() 
    && (payouts_chunk.len() < DO_VOID_POSITIONS_PAYOUTS_CHUNK_SIZE || update_storage_positions_chunk.len() < DO_VOID_POSITIONS_UPDATE_STORAGE_POSITION_CHUNK_SIZE) {
        let vp: &mut VoidPosition = &mut void_positions[i];
        
        if payouts_chunk.len() < DO_VOID_POSITIONS_PAYOUTS_CHUNK_SIZE
        && vp.payout_data().is_none()
        && *vp.payout_lock() == false {
            *vp.payout_lock() = true;
            payouts_chunk.push((
                vp.position_id(),
                do_payout_fn(DoPayoutQuest{
                    payee: IcrcId{ owner: vp.positor(), subaccount: vp.return_to_subaccount() },
                    trade_mount: vp.quantity(),
                    cts_payout_fee: 0,
                    memo: create_void_token_position_transfer_memo(vp.position_id())
                })
            ));
        }
        
        if update_storage_positions_yes_or_no == true
        && update_storage_positions_chunk.len() < DO_VOID_POSITIONS_UPDATE_STORAGE_POSITION_CHUNK_SIZE
        && vp.payout_data().is_some() // make sure the payout is complete before updating the storage-position-log. // the void-position-payout updates the position-log dust_collection and void_token_position_payout_ledger_transfer_fee fields.  
        && vp.update_storage_position_data().status == false 
        && vp.update_storage_position_data().lock == false {
            vp.update_storage_position_data_mut().lock = true;
            update_storage_positions_chunk.push((
                vp.position_id(),
                do_update_storage_position(vp.position_id(), vp.update_storage_position_data().update_storage_position_log.stable_memory_serialize())
            ));
        }
        
        i += 1;
    }
    
    (payouts_chunk, update_storage_positions_chunk)
}
