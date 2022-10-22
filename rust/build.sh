
ls

rm -rf target/

ls

cargo build --target wasm32-unknown-unknown --release


ic-wasm target/wasm32-unknown-unknown/release/cts.wasm -o target/wasm32-unknown-unknown/release/cts-o.wasm shrink

ic-wasm target/wasm32-unknown-unknown/release/cbs_map.wasm -o target/wasm32-unknown-unknown/release/cbs_map-o.wasm shrink

ic-wasm target/wasm32-unknown-unknown/release/cycles_bank.wasm -o target/wasm32-unknown-unknown/release/cycles_bank-o.wasm shrink

ic-wasm target/wasm32-unknown-unknown/release/cycles_transferrer.wasm -o target/wasm32-unknown-unknown/release/cycles_transferrer-o.wasm shrink

ic-wasm target/wasm32-unknown-unknown/release/cycles_market.wasm -o target/wasm32-unknown-unknown/release/cycles_market-o.wasm shrink

ic-wasm target/wasm32-unknown-unknown/release/cm_caller.wasm -o target/wasm32-unknown-unknown/release/cm_caller-o.wasm shrink

