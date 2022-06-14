
cargo build --target wasm32-unknown-unknown --release

ic-cdk-optimizer target/wasm32-unknown-unknown/release/cts.wasm -o target/wasm32-unknown-unknown/release/cts-o.wasm

ic-cdk-optimizer target/wasm32-unknown-unknown/release/cycles_market.wasm -o target/wasm32-unknown-unknown/release/cycles_market-o.wasm

ic-cdk-optimizer target/wasm32-unknown-unknown/release/cycles_transferrer.wasm -o target/wasm32-unknown-unknown/release/cycles_transferrer-o.wasm

ic-cdk-optimizer target/wasm32-unknown-unknown/release/users_map_canister.wasm -o target/wasm32-unknown-unknown/release/users_map_canister-o.wasm

ic-cdk-optimizer target/wasm32-unknown-unknown/release/user_canister.wasm -o target/wasm32-unknown-unknown/release/user_canister-o.wasm

