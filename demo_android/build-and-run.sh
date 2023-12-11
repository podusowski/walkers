set -e

trap popd EXIT
pushd demo_android/

cargo ndk --target arm64-v8a -o app/src/main/jniLibs/ build --profile release
./gradlew installDebug
adb shell am start -n local.walkers/.MainActivity
