#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
  echo "usage: $0 <library> <platform> <architecture> <output.cpmodule> <repository-root>" >&2
  exit 2
fi

library=$1
platform=$2
architecture=$3
output=$4
repository_root=$5
module_dir=$(cd "$(dirname "$0")/.." && pwd)
runtime_dir="$module_dir/native/$platform/$architecture"
model_dir="$module_dir/assets/models"

[[ -f "$library" ]] || { echo "module library is missing" >&2; exit 1; }
[[ -d "$runtime_dir" ]] || { echo "native runtime is missing: $runtime_dir" >&2; exit 1; }
for model in detector.onnx eslav.onnx latin.onnx cjk.onnx arabic.onnx devanagari.onnx korean.onnx thai.onnx greek.onnx tamil.onnx telugu.onnx eslav.keys.txt latin.keys.txt cjk.keys.txt arabic.keys.txt devanagari.keys.txt korean.keys.txt thai.keys.txt greek.keys.txt tamil.keys.txt telugu.keys.txt; do
  [[ -f "$model_dir/$model" ]] || { echo "model is missing: $model" >&2; exit 1; }
done

python3 "$repository_root/scripts/modules/package.py" \
  --module-dir "$module_dir" \
  --library "$library" \
  --platform "$platform" \
  --architecture "$architecture" \
  --output "$output"
