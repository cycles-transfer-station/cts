default:
  just --list


target_wasm32_path := justfile_directory() + "/rust/target/wasm32-unknown-unknown"
wasms_release_path := target_wasm32_path / "release"
wasms_debug_path := target_wasm32_path / "debug"
rust_canisters_path := justfile_directory() / "rust/canisters" 

bank_filename := "bank.wasm"
bank_did_path := rust_canisters_path / "bank/bank.did"
bank_test_did_path := rust_canisters_path / "bank/bank_test.did"

cm_tc_filename := "cm_tc.wasm"
cm_tc_did_path := rust_canisters_path / "market/cm_tc/cm_tc.did"
cm_tc_test_did_path := rust_canisters_path / "market/cm_tc/cm_tc_test.did"


cargo-build-wasms profile:
    cd {{justfile_directory()}}/rust && cargo build --locked --target wasm32-unknown-unknown --profile {{profile}}
    
create-candid-files:
    candid-extractor {{wasms_release_path / bank_filename}} > {{bank_did_path}}
    candid-extractor {{wasms_debug_path / bank_filename}} > {{bank_test_did_path}}
    candid-extractor {{wasms_release_path / cm_tc_filename}} > {{cm_tc_did_path}}
    candid-extractor {{wasms_debug_path / cm_tc_filename}} > {{cm_tc_test_did_path}}

put-candid-metadata:
    ic-wasm {{wasms_release_path / bank_filename}} -o {{wasms_release_path / bank_filename}} metadata candid:service -f {{bank_did_path}} -v public
    ic-wasm {{wasms_debug_path / bank_filename}} -o {{wasms_debug_path / bank_filename}} metadata candid:service -f {{bank_test_did_path}} -v public
    ic-wasm {{wasms_release_path / cm_tc_filename}} -o {{wasms_release_path / cm_tc_filename}} metadata candid:service -f {{cm_tc_did_path}} -v public
    ic-wasm {{wasms_debug_path / cm_tc_filename}} -o {{wasms_debug_path / cm_tc_filename}} metadata candid:service -f {{cm_tc_test_did_path}} -v public

build: && (cargo-build-wasms "dev") (cargo-build-wasms "release") create-candid-files put-candid-metadata
    @echo "build"


test-unit:
    cd {{justfile_directory()}}/rust && cargo test
    
test-pic:
    cd {{justfile_directory()}}/rust/pic_tests/tests && cargo test

test: && test-unit test-pic
    @echo "test"
        

