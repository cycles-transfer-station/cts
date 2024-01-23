ic-wasm "target/wasm32-unknown-unknown/debug/bank.wasm" -o "target/wasm32-unknown-unknown/debug/bank.wasm" metadata candid:service -f canisters/bank/bank.did -v public

ic-wasm "target/wasm32-unknown-unknown/release/bank.wasm" -o "target/wasm32-unknown-unknown/release/bank.wasm" metadata candid:service -f canisters/bank/bank.did -v public
