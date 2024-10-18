use std::borrow::Cow;


type LogPointer = u32; // can handle up to 4_000_000_000 blocks so will change to u64 once we get there. 

pub struct StorableLogsPointers(Cow<'_, [u8]>);

impl Storable for StorableLogsPointers {
    const BOUND: Bound = Bound::Unbounded;
    fn to_bytes(&self) -> Cow<[u8]> {
        self.0
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(bytes)
    }
}

impl StorableLogsPointers {
    pub fn get_account_transactions(user: Principal, start_at: ) -> &[LogPointer] {
        
    }
}