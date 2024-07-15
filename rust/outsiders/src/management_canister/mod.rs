// interface-spec-version: 0.25.0 (2024-06-14)

// This is an experimental feature to generate Rust binding from Candid.
// You may want to manually adjust some of the types.
#![allow(dead_code, unused_imports)]
use candid::{self, CandidType, Deserialize, Principal};
use ic_cdk::api::call::CallResult as Result;

#[derive(CandidType, Deserialize)]
pub enum BitcoinNetwork {
  #[serde(rename="mainnet")]
  Mainnet,
  #[serde(rename="testnet")]
  Testnet,
}
pub type BitcoinAddress = String;
#[derive(CandidType, Deserialize)]
pub struct BitcoinGetBalanceArgs {
  pub network: BitcoinNetwork,
  pub address: BitcoinAddress,
  pub min_confirmations: Option<u32>,
}
pub type Satoshi = u64;
pub type BitcoinGetBalanceResult = Satoshi;
#[derive(CandidType, Deserialize)]
pub struct BitcoinGetBalanceQueryArgs {
  pub network: BitcoinNetwork,
  pub address: BitcoinAddress,
  pub min_confirmations: Option<u32>,
}
pub type BitcoinGetBalanceQueryResult = Satoshi;
#[derive(CandidType, Deserialize)]
pub struct BitcoinGetCurrentFeePercentilesArgs { pub network: BitcoinNetwork }
pub type MillisatoshiPerByte = u64;
pub type BitcoinGetCurrentFeePercentilesResult = Vec<MillisatoshiPerByte>;
#[derive(CandidType, Deserialize)]
pub enum BitcoinGetUtxosArgsFilterInner {
  #[serde(rename="page")]
  Page(serde_bytes::ByteBuf),
  #[serde(rename="min_confirmations")]
  MinConfirmations(u32),
}
#[derive(CandidType, Deserialize)]
pub struct BitcoinGetUtxosArgs {
  pub network: BitcoinNetwork,
  pub filter: Option<BitcoinGetUtxosArgsFilterInner>,
  pub address: BitcoinAddress,
}
pub type BlockHash = serde_bytes::ByteBuf;
#[derive(CandidType, Deserialize)]
pub struct Outpoint { pub txid: serde_bytes::ByteBuf, pub vout: u32 }
#[derive(CandidType, Deserialize)]
pub struct Utxo { pub height: u32, pub value: Satoshi, pub outpoint: Outpoint }
#[derive(CandidType, Deserialize)]
pub struct BitcoinGetUtxosResult {
  pub next_page: Option<serde_bytes::ByteBuf>,
  pub tip_height: u32,
  pub tip_block_hash: BlockHash,
  pub utxos: Vec<Utxo>,
}
#[derive(CandidType, Deserialize)]
pub enum BitcoinGetUtxosQueryArgsFilterInner {
  #[serde(rename="page")]
  Page(serde_bytes::ByteBuf),
  #[serde(rename="min_confirmations")]
  MinConfirmations(u32),
}
#[derive(CandidType, Deserialize)]
pub struct BitcoinGetUtxosQueryArgs {
  pub network: BitcoinNetwork,
  pub filter: Option<BitcoinGetUtxosQueryArgsFilterInner>,
  pub address: BitcoinAddress,
}
#[derive(CandidType, Deserialize)]
pub struct BitcoinGetUtxosQueryResult {
  pub next_page: Option<serde_bytes::ByteBuf>,
  pub tip_height: u32,
  pub tip_block_hash: BlockHash,
  pub utxos: Vec<Utxo>,
}
#[derive(CandidType, Deserialize)]
pub struct BitcoinSendTransactionArgs {
  pub transaction: serde_bytes::ByteBuf,
  pub network: BitcoinNetwork,
}
pub type CanisterId = Principal;
#[derive(CandidType, Deserialize)]
pub struct CanisterInfoArgs {
  pub canister_id: CanisterId,
  pub num_requested_changes: Option<u64>,
}
#[derive(CandidType, Deserialize)]
pub enum ChangeOrigin {
  #[serde(rename="from_user")]
  FromUser{ user_id: Principal },
  #[serde(rename="from_canister")]
  FromCanister{ canister_version: Option<u64>, canister_id: Principal },
}
#[derive(CandidType, Deserialize)]
pub enum ChangeDetailsCodeDeploymentMode {
  #[serde(rename="reinstall")]
  Reinstall,
  #[serde(rename="upgrade")]
  Upgrade,
  #[serde(rename="install")]
  Install,
}
#[derive(CandidType, Deserialize)]
pub enum ChangeDetails {
  #[serde(rename="creation")]
  Creation{ controllers: Vec<Principal> },
  #[serde(rename="code_deployment")]
  CodeDeployment{
    mode: ChangeDetailsCodeDeploymentMode,
    module_hash: serde_bytes::ByteBuf,
  },
  #[serde(rename="controllers_change")]
  ControllersChange{ controllers: Vec<Principal> },
  #[serde(rename="code_uninstall")]
  CodeUninstall,
}
#[derive(CandidType, Deserialize)]
pub struct Change {
  pub timestamp_nanos: u64,
  pub canister_version: u64,
  pub origin: ChangeOrigin,
  pub details: ChangeDetails,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterInfoResult {
  pub controllers: Vec<Principal>,
  pub module_hash: Option<serde_bytes::ByteBuf>,
  pub recent_changes: Vec<Change>,
  pub total_num_changes: u64,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterStatusArgs { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub enum CanisterStatusResultStatus {
  #[serde(rename="stopped")]
  Stopped,
  #[serde(rename="stopping")]
  Stopping,
  #[serde(rename="running")]
  Running,
}
#[derive(CandidType, Deserialize)]
pub enum LogVisibility {
  #[serde(rename="controllers")]
  Controllers,
  #[serde(rename="public")]
  Public,
}
#[derive(CandidType, Deserialize)]
pub struct DefiniteCanisterSettings {
  pub freezing_threshold: candid::Nat,
  pub controllers: Vec<Principal>,
  pub reserved_cycles_limit: candid::Nat,
  pub log_visibility: LogVisibility,
  pub wasm_memory_limit: candid::Nat,
  pub memory_allocation: candid::Nat,
  pub compute_allocation: candid::Nat,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterStatusResultQueryStats {
  pub response_payload_bytes_total: candid::Nat,
  pub num_instructions_total: candid::Nat,
  pub num_calls_total: candid::Nat,
  pub request_payload_bytes_total: candid::Nat,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterStatusResult {
  pub status: CanisterStatusResultStatus,
  pub memory_size: candid::Nat,
  pub cycles: u128,
  pub settings: DefiniteCanisterSettings,
  pub query_stats: CanisterStatusResultQueryStats,
  pub idle_cycles_burned_per_day: candid::Nat,
  pub module_hash: Option<serde_bytes::ByteBuf>,
  pub reserved_cycles: candid::Nat,
}
#[derive(CandidType, Deserialize)]
pub struct ClearChunkStoreArgs { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub struct CanisterSettings {
  pub freezing_threshold: Option<candid::Nat>,
  pub controllers: Option<Vec<Principal>>,
  pub reserved_cycles_limit: Option<candid::Nat>,
  pub log_visibility: Option<LogVisibility>,
  pub wasm_memory_limit: Option<candid::Nat>,
  pub memory_allocation: Option<candid::Nat>,
  pub compute_allocation: Option<candid::Nat>,
}
#[derive(CandidType, Deserialize)]
pub struct CreateCanisterArgs {
  pub settings: Option<CanisterSettings>,
  pub sender_canister_version: Option<u64>,
}
#[derive(CandidType, Deserialize)]
pub struct CreateCanisterResult { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub struct DeleteCanisterArgs { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub struct DepositCyclesArgs { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub enum EcdsaCurve { #[serde(rename="secp256k1")] Secp256K1 }
#[derive(CandidType, Deserialize)]
pub struct EcdsaPublicKeyArgsKeyId { pub name: String, pub curve: EcdsaCurve }
#[derive(CandidType, Deserialize)]
pub struct EcdsaPublicKeyArgs {
  pub key_id: EcdsaPublicKeyArgsKeyId,
  pub canister_id: Option<CanisterId>,
  pub derivation_path: Vec<serde_bytes::ByteBuf>,
}
#[derive(CandidType, Deserialize)]
pub struct EcdsaPublicKeyResult {
  pub public_key: serde_bytes::ByteBuf,
  pub chain_code: serde_bytes::ByteBuf,
}
#[derive(CandidType, Deserialize)]
pub struct FetchCanisterLogsArgs { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub struct CanisterLogRecord {
  pub idx: u64,
  pub timestamp_nanos: u64,
  pub content: serde_bytes::ByteBuf,
}
#[derive(CandidType, Deserialize)]
pub struct FetchCanisterLogsResult {
  pub canister_log_records: Vec<CanisterLogRecord>,
}
#[derive(CandidType, Deserialize)]
pub enum HttpRequestArgsMethod {
  #[serde(rename="get")]
  Get,
  #[serde(rename="head")]
  Head,
  #[serde(rename="post")]
  Post,
}
#[derive(CandidType, Deserialize)]
pub struct HttpHeader { pub value: String, pub name: String }
#[derive(CandidType, Deserialize)]
pub struct HttpRequestResult {
  pub status: candid::Nat,
  pub body: serde_bytes::ByteBuf,
  pub headers: Vec<HttpHeader>,
}
#[derive(CandidType, Deserialize)]
pub struct HttpRequestArgsTransformInnerFunctionArg {
  pub context: serde_bytes::ByteBuf,
  pub response: HttpRequestResult,
}
candid::define_function!(pub HttpRequestArgsTransformInnerFunction : (
    HttpRequestArgsTransformInnerFunctionArg,
  ) -> (HttpRequestResult) query);
#[derive(CandidType, Deserialize)]
pub struct HttpRequestArgsTransformInner {
  pub function: HttpRequestArgsTransformInnerFunction,
  pub context: serde_bytes::ByteBuf,
}
#[derive(CandidType, Deserialize)]
pub struct HttpRequestArgs {
  pub url: String,
  pub method: HttpRequestArgsMethod,
  pub max_response_bytes: Option<u64>,
  pub body: Option<serde_bytes::ByteBuf>,
  pub transform: Option<HttpRequestArgsTransformInner>,
  pub headers: Vec<HttpHeader>,
}
#[derive(CandidType, Deserialize)]
pub enum CanisterInstallModeUpgradeInnerWasmMemoryPersistenceInner {
  #[serde(rename="keep")]
  Keep,
  #[serde(rename="replace")]
  Replace,
}
#[derive(CandidType, Deserialize)]
pub struct CanisterInstallModeUpgradeInner {
  pub wasm_memory_persistence: Option<
    CanisterInstallModeUpgradeInnerWasmMemoryPersistenceInner
  >,
  pub skip_pre_upgrade: Option<bool>,
}
#[derive(CandidType, Deserialize)]
pub enum CanisterInstallMode {
  #[serde(rename="reinstall")]
  Reinstall,
  #[serde(rename="upgrade")]
  Upgrade(Option<CanisterInstallModeUpgradeInner>),
  #[serde(rename="install")]
  Install,
}
#[derive(CandidType, Deserialize)]
pub struct ChunkHash { pub hash: serde_bytes::ByteBuf }
#[derive(CandidType, Deserialize)]
pub struct InstallChunkedCodeArgs {
  pub arg: serde_bytes::ByteBuf,
  pub wasm_module_hash: serde_bytes::ByteBuf,
  pub mode: CanisterInstallMode,
  pub chunk_hashes_list: Vec<ChunkHash>,
  pub target_canister: CanisterId,
  pub store_canister: Option<CanisterId>,
  pub sender_canister_version: Option<u64>,
}
pub type WasmModule = serde_bytes::ByteBuf;
#[derive(CandidType, Deserialize)]
pub struct InstallCodeArgs {
  pub arg: serde_bytes::ByteBuf,
  pub wasm_module: WasmModule,
  pub mode: CanisterInstallMode,
  pub canister_id: CanisterId,
  pub sender_canister_version: Option<u64>,
}
#[derive(CandidType, Deserialize)]
pub struct NodeMetricsHistoryArgs {
  pub start_at_timestamp_nanos: u64,
  pub subnet_id: Principal,
}
#[derive(CandidType, Deserialize)]
pub struct NodeMetrics {
  pub num_block_failures_total: u64,
  pub node_id: Principal,
  pub num_blocks_proposed_total: u64,
}
#[derive(CandidType, Deserialize)]
pub struct NodeMetricsHistoryResultItem {
  pub timestamp_nanos: u64,
  pub node_metrics: Vec<NodeMetrics>,
}
pub type NodeMetricsHistoryResult = Vec<NodeMetricsHistoryResultItem>;
#[derive(CandidType, Deserialize)]
pub struct ProvisionalCreateCanisterWithCyclesArgs {
  pub settings: Option<CanisterSettings>,
  pub specified_id: Option<CanisterId>,
  pub amount: Option<candid::Nat>,
  pub sender_canister_version: Option<u64>,
}
#[derive(CandidType, Deserialize)]
pub struct ProvisionalCreateCanisterWithCyclesResult {
  pub canister_id: CanisterId,
}
#[derive(CandidType, Deserialize)]
pub struct ProvisionalTopUpCanisterArgs {
  pub canister_id: CanisterId,
  pub amount: candid::Nat,
}
pub type RawRandResult = serde_bytes::ByteBuf;
#[derive(CandidType, Deserialize)]
pub struct SignWithEcdsaArgsKeyId { pub name: String, pub curve: EcdsaCurve }
#[derive(CandidType, Deserialize)]
pub struct SignWithEcdsaArgs {
  pub key_id: SignWithEcdsaArgsKeyId,
  pub derivation_path: Vec<serde_bytes::ByteBuf>,
  pub message_hash: serde_bytes::ByteBuf,
}
#[derive(CandidType, Deserialize)]
pub struct SignWithEcdsaResult { pub signature: serde_bytes::ByteBuf }
#[derive(CandidType, Deserialize)]
pub struct StartCanisterArgs { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub struct StopCanisterArgs { pub canister_id: CanisterId }
#[derive(CandidType, Deserialize)]
pub struct StoredChunksArgs { pub canister_id: CanisterId }
pub type StoredChunksResult = Vec<ChunkHash>;
#[derive(CandidType, Deserialize)]
pub struct UninstallCodeArgs {
  pub canister_id: CanisterId,
  pub sender_canister_version: Option<u64>,
}
#[derive(CandidType, Deserialize)]
pub struct UpdateSettingsArgs {
  pub canister_id: Principal,
  pub settings: CanisterSettings,
  pub sender_canister_version: Option<u64>,
}
#[derive(CandidType, Deserialize)]
pub struct UploadChunkArgs {
  pub chunk: serde_bytes::ByteBuf,
  pub canister_id: Principal,
}
pub type UploadChunkResult = ChunkHash;

pub struct Service(pub Principal);
impl Service {
  pub async fn bitcoin_get_balance(&self, arg0: BitcoinGetBalanceArgs) -> Result<(BitcoinGetBalanceResult,)> {
    ic_cdk::call(self.0, "bitcoin_get_balance", (arg0,)).await
  }
  pub async fn bitcoin_get_balance_query(&self, arg0: BitcoinGetBalanceQueryArgs) -> Result<(BitcoinGetBalanceQueryResult,)> {
    ic_cdk::call(self.0, "bitcoin_get_balance_query", (arg0,)).await
  }
  pub async fn bitcoin_get_current_fee_percentiles(&self, arg0: BitcoinGetCurrentFeePercentilesArgs) -> Result<(BitcoinGetCurrentFeePercentilesResult,)> {
    ic_cdk::call(self.0, "bitcoin_get_current_fee_percentiles", (arg0,)).await
  }
  pub async fn bitcoin_get_utxos(&self, arg0: BitcoinGetUtxosArgs) -> Result<(BitcoinGetUtxosResult,)> {
    ic_cdk::call(self.0, "bitcoin_get_utxos", (arg0,)).await
  }
  pub async fn bitcoin_get_utxos_query(&self, arg0: BitcoinGetUtxosQueryArgs) -> Result<(BitcoinGetUtxosQueryResult,)> {
    ic_cdk::call(self.0, "bitcoin_get_utxos_query", (arg0,)).await
  }
  pub async fn bitcoin_send_transaction(&self, arg0: BitcoinSendTransactionArgs) -> Result<()> {
    ic_cdk::call(self.0, "bitcoin_send_transaction", (arg0,)).await
  }
  pub async fn canister_info(&self, arg0: CanisterInfoArgs) -> Result<(CanisterInfoResult,)> {
    ic_cdk::call(self.0, "canister_info", (arg0,)).await
  }
  pub async fn canister_status(&self, arg0: CanisterStatusArgs) -> Result<(CanisterStatusResult,)> {
    ic_cdk::call(self.0, "canister_status", (arg0,)).await
  }
  pub async fn clear_chunk_store(&self, arg0: ClearChunkStoreArgs) -> Result<()> {
    ic_cdk::call(self.0, "clear_chunk_store", (arg0,)).await
  }
  pub async fn create_canister(&self, arg0: CreateCanisterArgs) -> Result<(CreateCanisterResult,)> {
    ic_cdk::call(self.0, "create_canister", (arg0,)).await
  }
  pub async fn delete_canister(&self, arg0: DeleteCanisterArgs) -> Result<()> {
    ic_cdk::call(self.0, "delete_canister", (arg0,)).await
  }
  pub async fn deposit_cycles(&self, arg0: DepositCyclesArgs) -> Result<()> {
    ic_cdk::call(self.0, "deposit_cycles", (arg0,)).await
  }
  pub async fn ecdsa_public_key(&self, arg0: EcdsaPublicKeyArgs) -> Result<(EcdsaPublicKeyResult,)> {
    ic_cdk::call(self.0, "ecdsa_public_key", (arg0,)).await
  }
  pub async fn fetch_canister_logs(&self, arg0: FetchCanisterLogsArgs) -> Result<(FetchCanisterLogsResult,)> {
    ic_cdk::call(self.0, "fetch_canister_logs", (arg0,)).await
  }
  pub async fn http_request(&self, arg0: HttpRequestArgs) -> Result<(HttpRequestResult,)> {
    ic_cdk::call(self.0, "http_request", (arg0,)).await
  }
  pub async fn install_chunked_code(&self, arg0: InstallChunkedCodeArgs) -> Result<()> {
    ic_cdk::call(self.0, "install_chunked_code", (arg0,)).await
  }
  pub async fn install_code(&self, arg0: InstallCodeArgs) -> Result<()> {
    ic_cdk::call(self.0, "install_code", (arg0,)).await
  }
  pub async fn node_metrics_history(&self, arg0: NodeMetricsHistoryArgs) -> Result<(NodeMetricsHistoryResult,)> {
    ic_cdk::call(self.0, "node_metrics_history", (arg0,)).await
  }
  pub async fn provisional_create_canister_with_cycles(&self, arg0: ProvisionalCreateCanisterWithCyclesArgs) -> Result<(ProvisionalCreateCanisterWithCyclesResult,)> {
    ic_cdk::call(self.0, "provisional_create_canister_with_cycles", (arg0,)).await
  }
  pub async fn provisional_top_up_canister(&self, arg0: ProvisionalTopUpCanisterArgs) -> Result<()> {
    ic_cdk::call(self.0, "provisional_top_up_canister", (arg0,)).await
  }
  pub async fn raw_rand(&self) -> Result<(RawRandResult,)> {
    ic_cdk::call(self.0, "raw_rand", ()).await
  }
  pub async fn sign_with_ecdsa(&self, arg0: SignWithEcdsaArgs) -> Result<(SignWithEcdsaResult,)> {
    ic_cdk::call(self.0, "sign_with_ecdsa", (arg0,)).await
  }
  pub async fn start_canister(&self, arg0: StartCanisterArgs) -> Result<()> {
    ic_cdk::call(self.0, "start_canister", (arg0,)).await
  }
  pub async fn stop_canister(&self, arg0: StopCanisterArgs) -> Result<()> {
    ic_cdk::call(self.0, "stop_canister", (arg0,)).await
  }
  pub async fn stored_chunks(&self, arg0: StoredChunksArgs) -> Result<(StoredChunksResult,)> {
    ic_cdk::call(self.0, "stored_chunks", (arg0,)).await
  }
  pub async fn uninstall_code(&self, arg0: UninstallCodeArgs) -> Result<()> {
    ic_cdk::call(self.0, "uninstall_code", (arg0,)).await
  }
  pub async fn update_settings(&self, arg0: UpdateSettingsArgs) -> Result<()> {
    ic_cdk::call(self.0, "update_settings", (arg0,)).await
  }
  pub async fn upload_chunk(&self, arg0: UploadChunkArgs) -> Result<(UploadChunkResult,)> {
    ic_cdk::call(self.0, "upload_chunk", (arg0,)).await
  }
}

