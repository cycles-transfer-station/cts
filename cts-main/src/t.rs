

use crate::*;






#[test]
fn test_backwards_cycles_to_icptokens() {
        
    let sample_xdr_per_myriad_per_icp_rate: u64 = 0.3;
    let sample_icptokens: IcpTokens = IcpTokens::from_e8s(464246642);

    assert_eq!(cycles_to_icptokens(icptokens_to_cycles(sample_icptokens, sample_xdr_per_myriad_per_icp_rate)), sample_icptokens);

}
