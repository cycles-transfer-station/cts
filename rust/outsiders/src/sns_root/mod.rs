// dfinity/ic commit: e790c6636115482db53ca3daa2f1900202ab04cf, module-hash: 12b6bba135b8bcff8a1384f15d202dd4f6e7bbbf0554994d5da4949125b6fdaa, nns-proposal: 129632 

// This is an experimental feature to generate Rust binding from Candid.
// You may want to manually adjust some of the types.
#![allow(dead_code, unused_imports)]
use candid::{self, CandidType, Deserialize, Principal};
use ic_cdk::api::call::CallResult as Result;

#[derive(CandidType, Deserialize)]
pub struct SnsRootCanister {
  pub dapp_canister_ids: Vec<Principal>,
  pub testflight: bool,
  pub latest_ledger_archive_poll_timestamp_seconds: Option<u64>,
  pub archive_canister_ids: Vec<Principal>,
  pub governance_canister_id: Option<Principal>,
  pub index_canister_id: Option<Principal>,
  pub swap_canister_id: Option<Principal>,
  pub ledger_canister_id: Option<Principal>,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterIdRecord { pub canister_id: Principal }
#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum CanisterStatusType {
  #[serde(rename="stopped")]
  Stopped,
  #[serde(rename="stopping")]
  Stopping,
  #[serde(rename="running")]
  Running,
}
#[derive(CandidType, Deserialize)]
pub struct DefiniteCanisterSettings {
  pub freezing_threshold: Option<candid::Nat>,
  pub controllers: Vec<Principal>,
  pub reserved_cycles_limit: Option<candid::Nat>,
  pub memory_allocation: Option<candid::Nat>,
  pub compute_allocation: Option<candid::Nat>,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterStatusResult {
  pub status: CanisterStatusType,
  pub memory_size: candid::Nat,
  pub cycles: candid::Nat,
  pub settings: DefiniteCanisterSettings,
  pub idle_cycles_burned_per_day: Option<candid::Nat>,
  pub module_hash: Option<serde_bytes::ByteBuf>,
  pub reserved_cycles: Option<candid::Nat>,
}
#[derive(CandidType, Deserialize)]
pub enum CanisterInstallMode {
  #[serde(rename="reinstall")]
  Reinstall,
  #[serde(rename="upgrade")]
  Upgrade,
  #[serde(rename="install")]
  Install,
}
#[derive(CandidType, Deserialize)]
pub struct ChangeCanisterRequest {
  pub arg: serde_bytes::ByteBuf,
  pub wasm_module: serde_bytes::ByteBuf,
  pub stop_before_installing: bool,
  pub mode: CanisterInstallMode,
  pub canister_id: Principal,
  pub memory_allocation: Option<candid::Nat>,
  pub compute_allocation: Option<candid::Nat>,
}
#[derive(CandidType, Deserialize)]
pub struct GetSnsCanistersSummaryRequest {
  pub update_canister_list: Option<bool>,
}
#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct DefiniteCanisterSettingsArgs {
  pub freezing_threshold: candid::Nat,
  pub controllers: Vec<Principal>,
  pub memory_allocation: candid::Nat,
  pub compute_allocation: candid::Nat,
}
#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct CanisterStatusResultV2 {
  pub status: CanisterStatusType,
  pub memory_size: candid::Nat,
  pub cycles: u128,
  pub settings: DefiniteCanisterSettingsArgs,
  pub idle_cycles_burned_per_day: candid::Nat,
  pub module_hash: Option<serde_bytes::ByteBuf>,
}
#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct CanisterSummary {
  pub status: Option<CanisterStatusResultV2>,
  pub canister_id: Option<Principal>,
}
#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct GetSnsCanistersSummaryResponse {
  pub root: Option<CanisterSummary>,
  pub swap: Option<CanisterSummary>,
  pub ledger: Option<CanisterSummary>,
  pub index: Option<CanisterSummary>,
  pub governance: Option<CanisterSummary>,
  pub dapps: Vec<CanisterSummary>,
  pub archives: Vec<CanisterSummary>,
}
#[derive(CandidType, Deserialize)]
pub struct ListSnsCanistersArg {}
#[derive(CandidType, Deserialize)]
pub struct ListSnsCanistersResponse {
  pub root: Option<Principal>,
  pub swap: Option<Principal>,
  pub ledger: Option<Principal>,
  pub index: Option<Principal>,
  pub governance: Option<Principal>,
  pub dapps: Vec<Principal>,
  pub archives: Vec<Principal>,
}
#[derive(CandidType, Deserialize)]
pub struct ManageDappCanisterSettingsRequest {
  pub freezing_threshold: Option<u64>,
  pub canister_ids: Vec<Principal>,
  pub reserved_cycles_limit: Option<u64>,
  pub log_visibility: Option<i32>,
  pub wasm_memory_limit: Option<u64>,
  pub memory_allocation: Option<u64>,
  pub compute_allocation: Option<u64>,
}
#[derive(CandidType, Deserialize)]
pub struct ManageDappCanisterSettingsResponse {
  pub failure_reason: Option<String>,
}
#[derive(CandidType, Deserialize)]
pub struct RegisterDappCanisterRequest { pub canister_id: Option<Principal> }
#[derive(CandidType, Deserialize)]
pub struct RegisterDappCanisterRet {}
#[derive(CandidType, Deserialize)]
pub struct RegisterDappCanistersRequest { pub canister_ids: Vec<Principal> }
#[derive(CandidType, Deserialize)]
pub struct RegisterDappCanistersRet {}
#[derive(CandidType, Deserialize)]
pub struct SetDappControllersRequest {
  pub canister_ids: Option<RegisterDappCanistersRequest>,
  pub controller_principal_ids: Vec<Principal>,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterCallError { pub code: Option<i32>, pub description: String }
#[derive(CandidType, Deserialize)]
pub struct FailedUpdate {
  pub err: Option<CanisterCallError>,
  pub dapp_canister_id: Option<Principal>,
}
#[derive(CandidType, Deserialize)]
pub struct SetDappControllersResponse { pub failed_updates: Vec<FailedUpdate> }

pub struct Service(pub Principal);
impl Service {
  pub async fn canister_status(&self, arg0: CanisterIdRecord) -> Result<(CanisterStatusResult,)> {
    ic_cdk::call(self.0, "canister_status", (arg0,)).await
  }
  pub async fn change_canister(&self, arg0: ChangeCanisterRequest) -> Result<()> {
    ic_cdk::call(self.0, "change_canister", (arg0,)).await
  }
  pub async fn get_build_metadata(&self) -> Result<(String,)> {
    ic_cdk::call(self.0, "get_build_metadata", ()).await
  }
  pub async fn get_sns_canisters_summary(&self, arg0: GetSnsCanistersSummaryRequest) -> Result<(GetSnsCanistersSummaryResponse,)> {
    ic_cdk::call(self.0, "get_sns_canisters_summary", (arg0,)).await
  }
  pub async fn list_sns_canisters(&self, arg0: ListSnsCanistersArg) -> Result<(ListSnsCanistersResponse,)> {
    ic_cdk::call(self.0, "list_sns_canisters", (arg0,)).await
  }
  pub async fn manage_dapp_canister_settings(&self, arg0: ManageDappCanisterSettingsRequest) -> Result<(ManageDappCanisterSettingsResponse,)> {
    ic_cdk::call(self.0, "manage_dapp_canister_settings", (arg0,)).await
  }
  pub async fn register_dapp_canister(&self, arg0: RegisterDappCanisterRequest) -> Result<(RegisterDappCanisterRet,)> {
    ic_cdk::call(self.0, "register_dapp_canister", (arg0,)).await
  }
  pub async fn register_dapp_canisters(&self, arg0: RegisterDappCanistersRequest) -> Result<(RegisterDappCanistersRet,)> {
    ic_cdk::call(self.0, "register_dapp_canisters", (arg0,)).await
  }
  pub async fn set_dapp_controllers(&self, arg0: SetDappControllersRequest) -> Result<(SetDappControllersResponse,)> {
    ic_cdk::call(self.0, "set_dapp_controllers", (arg0,)).await
  }
}

