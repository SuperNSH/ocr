#!/bin/sh
# The ocrs models the demo bundles as assets: no CI runner or browser can fetch
# them at runtime (the only public copies are on S3, without CORS headers).
# The S3 URLs are not guaranteed stable long-term (ocrs' own script says so);
# the sha256 checks below pin the bytes. The files are gitignored.
set -eu

here="$(cd "$(dirname -- "$0")" && pwd)"
mkdir -p "$here/assets"

fetch() {
    file="$1" url="$2" want="$3"
    curl -fsSL -o "$here/assets/$file" "$url"
    if command -v sha256sum > /dev/null 2>&1; then
        got="$(sha256sum "$here/assets/$file" | cut -d' ' -f1)"
    else
        got="$(shasum -a 256 "$here/assets/$file" | cut -d' ' -f1)"
    fi
    test "$got" = "$want" || { echo "$file: sha256 $got, wanted $want" >&2; exit 1; }
}

fetch text-detection.onnx \
    https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx \
    a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe
fetch text-recognition.onnx \
    https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx \
    86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5
