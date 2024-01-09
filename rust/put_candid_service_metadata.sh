ic-wasm "target/wasm32-unknown-unknown/debug/cts.wasm" -o "target/wasm32-unknown-unknown/debug/cts.wasm" metadata candid:service -f canisters/cts/cts.did -v public

ic-wasm "target/wasm32-unknown-unknown/release/cts.wasm" -o "target/wasm32-unknown-unknown/release/cts.wasm" metadata candid:service -f canisters/cts/cts.did -v public
