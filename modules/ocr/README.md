# CopyPaste OCR module

This is an optional first-party OCR module. It receives a host-staged image
file, runs wholly locally, and returns plain text to the shared CopyPaste UI.
The base application has no OCR model or ONNX Runtime dependency.

## Package preparation

The catalog declares macOS 14+, Windows 10+, and Android 7+ for this runtime.
Linux compatibility is proved by the signed native packages; the marketplace does not
claim a distribution version because its current system-version field cannot represent
the runtime's glibc ABI floor.
CopyPaste 1.0.6+ displays these requirements and keeps incompatible modules
visible with installation disabled. The **Build and publish OCR module**
workflow prepares all seven shipped packages, signs them, performs native
qualification, and publishes the module release and signed marketplace.

Run `python3 scripts/fetch-models.py` from this directory. It downloads only
the PaddlePaddle model files declared at immutable revisions in
`assets/model-sources.json` and verifies every SHA-256 value. No executable
uses that network path after packaging.

Download ONNX Runtime **1.28** CPU archives from the official Microsoft
release, verify its published checksum, extract the library, then place it in
the module staging tree, for example:

```sh
python3 scripts/fetch-runtime.py \
  --library /tmp/onnxruntime/lib/libonnxruntime.so \
  --sha256 <published-sha256> \
  --platform linux --architecture x86_64
```

The package tool places `native/<platform>/<architecture>` beside the module
library as `bin/`. On macOS the copied runtime must retain an `@loader_path`
install name; on Linux/Android it must use `$ORIGIN`; on Windows the host loads
the dependency from the module DLL directory.

Build the native library from this directory with:

```sh
PATH=/Users/dmytro/.cargo/bin:$PATH cargo build --release
```

Then package one target with:

```sh
scripts/package-ocr.sh target/release/libcopypaste_module_ocr.dylib macos aarch64 /tmp/ocr.cpmodule /absolute/path/to/CopyPaste
```

The release signer invoked by the generic package builder signs the generated
manifest. Do not package unsigned artifacts.

## Native lifecycle requirement

ONNX Runtime owns a process-global environment. This module disables its
telemetry and does not register custom log or thread callbacks, while its model
sessions are owned by and dropped with the module instance. Its manifest uses
`"unload_policy": "process"`, so CopyPaste pins the native OCR code until the
application exits and never unloads it while ORT globals exist.

## Routing and coverage

The module shares PP-OCRv5 detection, then evaluates each detected line against
the packaged recognizers. It selects a line by a
combination of model confidence and Unicode script fit. It does not select the
largest raw confidence across unrelated models. This supports mixed-script
documents where scripts are on separate detected lines.

Included recognition routes are East Slavic (including Ukrainian and English),
Latin, Chinese/Japanese, Arabic, Devanagari, Korean, Thai, Greek, Tamil, and
Telugu. The router is a heuristic, not a calibrated image-language detector:
decoded Unicode and model confidence can be wrong, especially on one line that
mixes scripts.

On macOS arm64, the prepared local package recognized generated English-only
and separate Ukrainian/English lines. A one-line `CopyPaste: Привіт, Україно!
OCR 123` image retained `Україно` but read `Привіт` as `Привит` and converted
some Latin lookalikes to Cyrillic. Do not claim same-line mixed-script quality
from this baseline. The upstream PP-OCRv5 model table documents route coverage;
each route still needs representative quality evidence.
Before a release, run real-image evidence on every packaged target. At minimum
cover Ukrainian plus English, a mixed-script image, the remaining route scripts,
small text, rotation, and the package's memory and latency budget. The current
module does not include an orientation classifier, so rotated-image evidence is
required before claiming rotation support. Do not treat
host builds or model metadata as physical macOS, Android, or Windows evidence.
