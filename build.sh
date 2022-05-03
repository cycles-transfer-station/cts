
cargo build --target wasm32-unknown-unknown --release

ic-cdk-optimizer target/wasm32-unknown-unknown/release/cts_main.wasm -o target/wasm32-unknown-unknown/release/cts_main-o.wasm

ic-cdk-optimizer target/wasm32-unknown-unknown/release/cycles_wallet.wasm -o target/wasm32-unknown-unknown/release/cycles_wallet-o.wasm

