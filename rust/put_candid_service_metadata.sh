# bank
candid-extractor "target/wasm32-unknown-unknown/release/bank.wasm" > canisters/bank/bank.did

ic-wasm "target/wasm32-unknown-unknown/debug/bank.wasm" -o "target/wasm32-unknown-unknown/debug/bank.wasm" metadata candid:service -f canisters/bank/bank.did -v public

ic-wasm "target/wasm32-unknown-unknown/release/bank.wasm" -o "target/wasm32-unknown-unknown/release/bank.wasm" metadata candid:service -f canisters/bank/bank.did -v public


# cm_tc
candid-extractor "target/wasm32-unknown-unknown/release/cm_tc.wasm" > canisters/market/cm_tc/cm_tc.did

ic-wasm "target/wasm32-unknown-unknown/debug/cm_tc.wasm" -o "target/wasm32-unknown-unknown/debug/cm_tc.wasm" metadata candid:service -f canisters/market/cm_tc/cm_tc.did -v public

ic-wasm "target/wasm32-unknown-unknown/release/cm_tc.wasm" -o "target/wasm32-unknown-unknown/release/cm_tc.wasm" metadata candid:service -f canisters/market/cm_tc/cm_tc.did -v public
