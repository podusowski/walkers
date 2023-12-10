set -e

cargo ndk --target arm64-v8a -o app/src/main/jniLibs/ build --profile release
./gradlew build

adb wait-for-device
adb install -d app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n local.walkers.debug/local.walkers.MainActivity
adb logcat -v color -s walkers RustStdoutStderr
