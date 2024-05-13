SCRIPTS_DIR=$(readlink -f $(dirname ${0}))
cd $SCRIPTS_DIR/..

BUILD_DIR=build
rm -rf $BUILD_DIR

GIT_COMMIT_ID=$(git rev-parse HEAD)
echo "git_commit_id: $GIT_COMMIT_ID"

podman build --no-cache -t cts --build-arg git_commit_id=$GIT_COMMIT_ID .

container_id=$(podman create cts)
podman cp $container_id:/cts/$BUILD_DIR $BUILD_DIR
podman rm --volumes $container_id

for file in `ls $BUILD_DIR`;
do
    sha256sum $BUILD_DIR/$file
done
