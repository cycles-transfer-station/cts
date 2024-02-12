
wasms_release_path := "rust/target/wasm32-unknown-unknown/release/"

put_candid_metadata:
    candid-extractor $wasms_release_path"bank.wasm" > rust/canisters/bank/bank.did
    ic-wasm $wasms_release_path"bank.wasm" -o $wasms_release_path"bank.wasm" metadata candid:service -f canisters/bank/bank.did -v public
