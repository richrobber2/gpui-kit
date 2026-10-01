#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")"

native_library="target/aarch64-linux-android/debug/libgpui_kit_demo.so"
test -f "$native_library"
test -f android/android.jar
test -f android/r8.jar
mkdir -p android/package/lib/arm64-v8a android/classes dist
mkdir -p android/src/dev/gpui/mobile
# GPUI calls these helpers during activity lifecycle transitions. Keep them
# matched to the mobile platform revision, including their Java dependencies.
for helper in GpuiPlatformView GpuiCamera GpuiVideoPlayer; do
    cp "../../../gpui-mobile/example/android/gradle/app/src/main/java/dev/gpui/mobile/$helper.java" \
        android/src/dev/gpui/mobile/
done
rm -rf android/classes
mkdir -p android/classes
mapfile -t java_sources < <(find android/src -name '*.java' -type f)
javac --release 8 -Xlint:-options -classpath android/android.jar \
    -d android/classes "${java_sources[@]}"
mapfile -t java_classes < <(find android/classes -name '*.class' -type f)
java -cp android/r8.jar com.android.tools.r8.D8 --min-api 29 \
    --lib android/android.jar --output android/package \
    "${java_classes[@]}"
cp "$native_library" android/package/lib/arm64-v8a/
/data/data/com.termux/files/usr/bin/llvm-strip --strip-unneeded \
    android/package/lib/arm64-v8a/libgpui_kit_demo.so
patchelf --remove-rpath android/package/lib/arm64-v8a/libgpui_kit_demo.so

# Include the C++ runtime only if the Rust library links it dynamically.
if /data/data/com.termux/files/usr/bin/llvm-readelf -d "$native_library" \
    | grep -q 'NEEDED.*libc++_shared.so'; then
    cp /data/data/com.termux/files/usr/lib/libc++_shared.so \
        android/package/lib/arm64-v8a/
fi

# Match RustDL's resource packaging against the Android framework on this device.
framework_res="${FRAMEWORK_RES:-/system/framework/framework-res.apk}"
test -f "$framework_res"
mkdir -p android/compiled
rm -f android/compiled/*.flat
aapt2 compile --dir android/res -o android/compiled
aapt2 link -I "$framework_res" --manifest android/AndroidManifest.xml \
    --min-sdk-version 29 --target-sdk-version 34 \
    -o dist/gpui-kit-demo-unsigned.apk android/compiled/*.flat
(
    cd android/package
    zip -q -r ../../dist/gpui-kit-demo-unsigned.apk lib classes.dex
)
zipalign -f 4 dist/gpui-kit-demo-unsigned.apk dist/gpui-kit-demo-aligned.apk

if [[ ! -f android/debug.keystore ]]; then
    keytool -genkeypair -keystore android/debug.keystore \
        -storepass android -keypass android -alias androiddebugkey \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=GPUI Kit Demo,O=Android,C=US"
fi
apksigner sign --ks android/debug.keystore --ks-pass pass:android \
    --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true \
    --key-pass pass:android --out dist/gpui-kit-demo.apk \
    dist/gpui-kit-demo-aligned.apk
apksigner verify --verbose dist/gpui-kit-demo.apk
aapt dump badging dist/gpui-kit-demo.apk
