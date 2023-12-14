set -e

trap popd EXIT
pushd $(dirname $0)

cd rust
cargo ndk --target arm64-v8a -o ../java/app/src/main/jniLibs/ build --profile release

cd ../java
./gradlew installDebug
adb shell am start -n local.walkers/.MainActivity
