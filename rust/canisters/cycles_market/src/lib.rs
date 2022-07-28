// icp-payout fee fix[ed]-cycles-cost using the conversion rate

// when do icp-payout, take the fee first, then complete the payout. 

// opt param: fee-in-the-cycles-count when icp-payout 

// certified-data-query of the current (o)pen-positions on the market


// trade-fee in the cycles: 200_000_000_000 for both parties.
// payout-fee in the cycles: 50_000_000_000



use cts_lib::{
    ic_cdk::{
    
    },
    ic_cdk_macros::{
        update,
        query,
    },
    
};




pub const ICP_PAYOUT_FEE: IcpTokens = IcpTokens::from_e8s(30000);// calculate through the xdr conversion rate ? // 100_000_000_000-cycles























