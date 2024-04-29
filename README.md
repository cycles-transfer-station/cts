# CYCLES-TRANSFER-STATION

### Build
This build aims to be reproducible on Fedora and Ubuntu Linux. On a reproducible build, the build output files in the `build` folder have the same hash as the canister modules running on the CTS, therefore the CTS canisters' code is verifiable.

> `bash scripts/podman_build.sh`



### Local Build and Test
Requirements: `rust`, [`just`](https://github.com/casey/just), [`ic-wasm`](https://crates.io/crates/ic-wasm), [`candid-extractor`](https://crates.io/crates/candid-extractor).

> `just build <dev or release>`

> `just test`