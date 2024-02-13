
default:
  just --list

wasms_release_path := justfile_directory() + "/rust/target/wasm32-unknown-unknown/release"
bank_wasm_path := wasms_release_path + "/bank.wasm"
bank_did_path := justfile_directory() + "/rust/canisters/bank/bank.did"


put-candid-metadata:
    candid-extractor {{bank_wasm_path}} > {{bank_did_path}}
    ic-wasm {{bank_wasm_path}} -o {{bank_wasm_path}} metadata candid:service -f {{bank_did_path}} -v public
    
cargo-build-wasms:
    cd {{justfile_directory()}}/rust && cargo build --target wasm32-unknown-unknown --release
    
build: && cargo-build-wasms put-candid-metadata
    @echo "build"
