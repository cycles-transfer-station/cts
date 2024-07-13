use ic_cdk::{
    update,
    query,
    init,
    pre_upgrade,
    post_upgrade
};
use cts_lib::{
    consts::{
        SECONDS_IN_A_DAY,
    },
};

struct FuelerData {
    sns_root: Principal, // use the sns_root to find the canisters that the sns-root controlls.
    cm_main: Principal, // use the cm_main to get the cycles-balances of the cm_tcs
}

const FUELER_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);

const RHYTHM: Duration = Duration::from_secs(SECONDS_IN_A_DAY * 3);


thread_local! {
    static FUELER_DATA: RefCell<FuelerData> = RefCell::new(FuelerData::new); 
}

//pub struct FuelerInit {}


#[init]
fn init(q: FuelerData) {
    with_mut(&FUELER_DATA, |fueler_data| {
        *fueler_data = q;
    });
    canister_tools::init(&FUELER_DATA, FUELER_DATA_MEMORY_ID);    
    start_timer();
}

#[pre_upgrade]
fn pre_upgrade() {
    canister_tools::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    canister_tools::post_upgrade(&FUELER_DATA, FUELER_DATA_MEMORY_ID, None::<fn(FuelerData) -> FuelerData>);
    start_timer();
}

fn start_timer() {
    ic_cdk_timers::set_timer_interval(
        RHYTHM,
        || ic_cdk::spawn(fuel());
    );
}

async fn fuel () {
    
    let sns_root_controlled_canisters: HashSet<Principal>;
        with(&FUELER_DATA, |d| d.sns_root),
        "list_sns_canisters",
        ()
    match ic_cdk::call::<(), ()>(
        
    ).await {
    
    }    
    
    let mut sns_root_call_futures: Vec<(Principal/*the canister being called about*/, _/*call-future*/)> = Vec::new();
    with(&FUELER_DATA, |fueler_data| {
        for canister in fueler_data.sns_root_controlled_canisters.iter() {
            let call_future = ic_cdk::call::<(), ()>(
            
            );
        }     
    
    });
    
    
    
}
