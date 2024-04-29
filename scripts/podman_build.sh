SCRIPTS_DIR=$(readlink -f $(dirname ${0}))
cd $SCRIPTS_DIR/..

BUILD_DIR=build
rm -rf $BUILD_DIR

podman build --no-cache -t cts .

container_id=$(podman create cts)
podman cp $container_id:/cts/$BUILD_DIR $BUILD_DIR
podman rm --volumes $container_id

for file in `ls build`; 
do
    sha256sum build/$file
done