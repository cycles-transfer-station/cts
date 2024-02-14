podman build --no-cache -t cts_system_container $(readlink -f $(dirname ${0}))
