

use crate::*;






#[test]
fn test_backwards_cycles_to_icptokens() {
        
    let sample_xdr_per_myriad_per_icp_rate: u64 = 50;
    let sample_icptokens: IcpTokens = IcpTokens::from_e8s(464246642);

    assert_eq!(cycles_to_icptokens(icptokens_to_cycles(sample_icptokens, sample_xdr_per_myriad_per_icp_rate), sample_xdr_per_myriad_per_icp_rate), sample_icptokens);


    println!("icp: {}", sample_icptokens);
    
    let cycles: u128 = icptokens_to_cycles(sample_icptokens, sample_xdr_per_myriad_per_icp_rate);
    println!("cycles: {}", cycles);
    
    let back_to_icp: IcpTokens = cycles_to_icptokens(cycles, sample_xdr_per_myriad_per_icp_rate);
    println!("back to icp: {}", back_to_icp);

    assert_eq!(sample_icptokens, back_to_icp);

}


