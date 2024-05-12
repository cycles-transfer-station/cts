default:
  just --list


target_wasm32_path := justfile_directory() + "/rust/target/wasm32-unknown-unknown"
rust_canisters_path := justfile_directory() / "rust/canisters" 

cts_filename := "cts.wasm"
cts_did_path := rust_canisters_path / "cts/cts.did"

bank_filename := "bank.wasm"
bank_did_path := rust_canisters_path / "bank/bank.did"

cm_main_filename := "cm_main.wasm"
cm_main_did_path := rust_canisters_path / "market/cm_main/cm_main.did"

cm_tc_filename := "cm_tc.wasm"
cm_tc_did_path := rust_canisters_path / "market/cm_tc/cm_tc.did"

cm_positions_storage_filename := "cm_positions_storage.wasm"
cm_positions_storage_did_path := rust_canisters_path / "market/cm_positions_storage/cm_positions_storage.did"

cm_trades_storage_filename := "cm_trades_storage.wasm"
cm_trades_storage_did_path := rust_canisters_path / "market/cm_trades_storage/cm_trades_storage.did"


cargo-build-wasms profile:
    cd {{justfile_directory()}}/rust && cargo build --locked --target wasm32-unknown-unknown --profile {{profile}}
    
create-candid-files profile: (cargo-build-wasms profile)
    #!/usr/bin/env sh
    for i in \
        "{{cts_filename}}","{{cts_did_path}}" \
        "{{bank_filename}}","{{bank_did_path}}" \
        "{{cm_main_filename}}","{{cm_main_did_path}}" \
        "{{cm_tc_filename}}","{{cm_tc_did_path}}" \
        "{{cm_positions_storage_filename}}","{{cm_positions_storage_did_path}}" \
        "{{cm_trades_storage_filename}}","{{cm_trades_storage_did_path}}"; \
    do
        IFS=","
        set -- $i
        FILE_PATH={{target_wasm32_path}}/{{ if profile == "dev" { "debug" } else { profile } }}/$1
        DID_PATH=$([ "{{profile}}" = "release" ] && echo $2 || echo $(echo "$2" | sed -e 's/.did/_test.did/g') )
        candid-extractor $FILE_PATH > $DID_PATH
        ic-wasm $FILE_PATH -o $FILE_PATH metadata candid:service -f $DID_PATH -v public
    done
    
build profile="release": (cargo-build-wasms profile) (create-candid-files profile)
    #!/usr/bin/env sh
    rm -rf build
    if [ {{profile}} = release ]; then
        mkdir build
        for filename in \
            "{{cts_filename}}" \
            "{{bank_filename}}" \
            "{{cm_main_filename}}" \
            "{{cm_tc_filename}}" \
            "{{cm_positions_storage_filename}}" \
            "{{cm_trades_storage_filename}}"; \
        do
            cp {{target_wasm32_path}}/release/$filename build/
        done
    fi

test-unit:
    cd {{justfile_directory()}}/rust && cargo test
    
test-pic: (build "dev")
    cd {{justfile_directory()}}/rust/pic_tests/tests && cargo test | grep -v "Non-increasing batch time at height"
    
test: && test-unit test-pic
    @echo "test"

show-build-hashes:
    #!/usr/bin/env sh
    mkdir -p build
    for file in `ls build`; 
    do
        sha256sum build/$file
    done    
    