# CYCLES-TRANSFER-STATION

https://cycles-transfer-station.com


### ARCHITECTURE
An overview of the CTS Architecture is located in the document in this repo named ARCHITECTURE.md.


### Build
This build aims to be reproducible on Fedora Linux. On a reproducible build, the build output files in the `build` folder have the same hash as the canister modules running on the CTS, therefore the CTS canisters' code is verifiable.

For a reproducible-build, the single requirement is the `podman` package. Run the following command:

> `bash scripts/podman_build.sh`


### Build and Test without podman

Requirements: `rust`, [`just`](https://github.com/casey/just), [`ic-wasm`](https://crates.io/crates/ic-wasm), [`candid-extractor`](https://crates.io/crates/candid-extractor).

> `just build <dev or release>`

> `just test`
