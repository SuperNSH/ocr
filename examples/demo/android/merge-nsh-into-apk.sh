#!/usr/bin/env bash
# Merges an NSH-packaged Android APK into the Gradle shell APK:
# NSH's classes.dex rides along as classes2.dex, its native library and the
# app bundle (assets/) are zipped in, then the result is aligned and signed.
# Usage: merge-nsh-into-apk.sh <nsh.apk> <shell.apk> <out.apk>
set -euo pipefail

nsh_apk="$1"
shell_apk="$2"
out_apk="$3"

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$sdk" ]; then
    echo "ANDROID_HOME not set" >&2
    exit 1
fi
build_tools="$(ls "$sdk/build-tools" | sort -V | tail -1)"
bt="$sdk/build-tools/$build_tools"
echo "build-tools: $build_tools"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The NSH pieces: code, native library, app bundle. Resources and manifest
# stay the shell's own — Gradle merged ML Kit's already.
mkdir "$work/nsh"
unzip -q "$nsh_apk" -d "$work/nsh"
test -f "$work/nsh/classes.dex"
test -d "$work/nsh/lib"
test -d "$work/nsh/assets"

cp "$shell_apk" "$work/merged.apk"
# The shell's signature dies with the merge; drop it before re-signing.
zip -q -X -d "$work/merged.apk" 'META-INF/*.SF' 'META-INF/*.RSA' 'META-INF/*.EC' 'META-INF/*.MF' || true

cd "$work/nsh"
mv classes.dex classes2.dex
# Stored (-0): the system maps .so in place; extractNativeLibs is false.
zip -q -X "$work/merged.apk" classes2.dex
zip -q -X -0 "$work/merged.apk" lib
zip -q -X -r "$work/merged.apk" assets

"$bt/zipalign" -f -P 16 4 "$work/merged.apk" "$work/aligned.apk"

keystore="$HOME/.android/debug.keystore"
"$bt/apksigner" sign --ks "$keystore" --ks-pass pass:android --key-pass pass:android "$work/aligned.apk"
"$bt/apksigner" verify "$work/aligned.apk"

cp "$work/aligned.apk" "$out_apk"
echo "wrote $out_apk"
