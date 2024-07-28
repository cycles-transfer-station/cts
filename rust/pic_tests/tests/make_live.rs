use pic_tools::*;
use cts_lib::consts::TRILLION;

// must be a test, does not work as a package-binary for some reason.
#[test]
#[ignore]
fn make_live_go() {
    let mut pic = set_up();
    let icp_tc = set_up_tc(&pic);

    let (_ledger1, tc1) = set_up_new_ledger_and_tc(&pic);
    let (_ledger2, tc2) = set_up_new_ledger_and_tc(&pic);
    let (_ledger3, tc3) = set_up_new_ledger_and_tc(&pic);

    // for some reason goes down by 13T after make_live so we add enough here.
    for c in [icp_tc, tc1, tc2, tc3] {
        pic.add_cycles(c, 50 * TRILLION);
    }
    
    let url = pic.make_live(Some(8080));
    println!("{}", url);    

    loop {
        std::thread::sleep(std::time::Duration::from_secs(40));
        // keep it live
        pic.cycle_balance(SNS_ROOT);
    }
}
