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

fueler_filename := "fueler.wasm"
fueler_did_path := rust_canisters_path / "fueler/fueler.did"


cargo-build-wasms profile:
    rm -rf build && mkdir build
    cd {{justfile_directory()}}/rust && cargo build --locked --target wasm32-unknown-unknown --profile {{profile}}
    
build profile *git_commit_id: (cargo-build-wasms profile)
    #!/usr/bin/env sh
    set -eu
    
    if [ -d "{{justfile_directory() / ".git"}}" ]; 
    then
        if ! [ "{{git_commit_id}}" = "" ]; 
        then
            echo "Error: .git directory is found and git_commit_id parameter is set. When there is a .git dir, do not pass the git_commit_id parameter, you can git checkout to the commit you want."
            exit 1
        fi
        GIT_COMMIT_ID=$(git rev-parse HEAD)
    else
        if [ "{{git_commit_id}}" = "" ]; 
        then
            echo "Error: .git directory not found and git_commit_id parameter not set. When there is no .git dir, pass the git_commit_id as a parameter in the build task."
            exit 1
        fi
        GIT_COMMIT_ID={{git_commit_id}}
    fi
    echo "git_commit_id: $GIT_COMMIT_ID"
    
    for i in \
        "{{cts_filename}}","{{cts_did_path}}" \
        "{{bank_filename}}","{{bank_did_path}}" \
        "{{cm_main_filename}}","{{cm_main_did_path}}" \
        "{{cm_tc_filename}}","{{cm_tc_did_path}}" \
        "{{cm_positions_storage_filename}}","{{cm_positions_storage_did_path}}" \
        "{{cm_trades_storage_filename}}","{{cm_trades_storage_did_path}}" \
        "{{fueler_filename}}","{{fueler_did_path}}"; \
    do
        IFS=","
        set -- $i
        FILE_PATH={{target_wasm32_path}}/{{ if profile == "dev" { "debug" } else { profile } }}/$1
        DID_PATH=$([ "{{profile}}" = "release" ] && echo $2 || echo $(echo "$2" | sed -e 's/.did/_test.did/g') )
        candid-extractor $FILE_PATH > $DID_PATH
        ic-wasm $FILE_PATH -o $FILE_PATH metadata candid:service -f $DID_PATH -v public
        ic-wasm $FILE_PATH -o $FILE_PATH metadata git_commit_id -d $GIT_COMMIT_ID -v public
        if [ {{profile}} = release ]; then
            cp $FILE_PATH build/
        fi
    done

test-unit:
    cd {{justfile_directory()}}/rust && cargo test
    
test-pic *cargo_test_params: (build "dev")
    cd {{justfile_directory()}}/rust/pic_tests/tests && cargo test {{cargo_test_params}}
    
test: && test-unit test-pic
    @echo "test"

show-build-hashes:
    #!/usr/bin/env sh
    set -eu
    mkdir -p build
    for file in `ls build`; 
    do
        sha256sum build/$file
    done    
    
live-local: (build "dev")
    cd {{justfile_directory()}}/rust/pic_tests/tests && cargo test make_live_go -- --include-ignored --nocapture

test-upgrade: (build "release")
    cd {{justfile_directory()}}/rust/pic_tests/tests && cargo test test_upgrade -- --include-ignored --nocapture
