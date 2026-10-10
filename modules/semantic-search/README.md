# Semantic Search module

First-party offline semantic search for macOS, Windows, Linux, and Android. The host
owns language preferences, authenticated model downloads, encrypted embeddings,
background indexing, filtering, cursor paging, and shared History/Quick Paste UI.
Only this separately built module links ONNX Runtime and tokenizers.

## User contract

1. Install the module and choose one or more search languages in Preferences.
2. The form shows the compatible model's download size. Saving downloads only
   that model, checks its pinned SHA-256 and size, and atomically applies settings.
   A failed download preserves the previous configuration.
3. Enable the module. Retained text clips are indexed in the background in
   bounded UTF-8 fragments; new, changed, deleted, restored, and synced clips are
   accounted for. Queries keep working through ordinary fuzzy search while the
   model loads or indexing progresses.
4. Semantic matches automatically augment History and Quick Paste. Lexical
   matches retain priority, pins and filters retain their established behavior,
   and duplicate clips are removed before pagination.
5. Inference runs in a dedicated local process. It exits after 60 seconds
   without completed or active work, releasing model, tokenizer, and native
   allocator memory. Disabling stops admission and terminates that worker;
   removing also clears its encrypted index and downloaded data.

Language selection chooses model coverage; it does not hide clips in other
languages. Changing among language sets covered by the same model reuses assets
and its index. Models, queries, and clip text are never sent to an external inference
service. The network is used only to obtain selected public model assets during
explicit preference setup.

## Models

The signed manifest pins immutable Hugging Face revisions and byte digests:

| Selection | Model | Model and tokenizer download |
| --- | --- | --- |
| English only | all-MiniLM-L6-v2, INT8 | 23,684,031 bytes |
| Other supported languages or combinations | multilingual-e5-small, INT8 | 135,390,915 bytes |

The first version exposes English, Ukrainian, German, French, Spanish, Italian,
Portuguese, Polish, Czech, Russian, Arabic, Chinese, Japanese, Korean, Turkish,
Dutch, Swedish, Finnish, Greek, and Hindi. Upstream coverage does not establish
equal search quality in every language; local evidence currently covers English,
Ukrainian, and Ukrainian queries retrieving English clips.

Each passage window receives the appropriate E5 prefix. Attention-mask-aware
mean pooling produces normalized 384-dimensional vectors. Similarity thresholds
and a relative best-result margin discard weak results. These are retrieval
heuristics rather than guaranteed natural-language understanding.

The index is local derived data in the app's SQLCipher database, excluded from
clipboard sync. Model identity and clip content hashes prevent stale vectors;
restore clears derived rows and rebuilds them. Query vectors use a 32-entry
memory cache, retained independently of inference residency. Foreground reads
return lexical results immediately; queued query embeddings publish an Items
notification and augment the current query on refresh. One host scheduler gives
queries priority between bounded index fragments. Inference uses a single model
process, limited to two intra-operation threads and one inter-operation thread.
Opening empty or fully indexed History does not warm up a model. Model changes
terminate the previous worker before loading another profile.

The host owns the encrypted database and keys. The worker receives only bounded
command inputs over inherited pipes on desktop or a private socket passed by
Android Binder. Android runs a non-exported `:inference` service and terminates
its dedicated process, rather than leaving its allocator in a cached service.
See [Semantic inference lifecycle](../../docs/semantic-inference.md).

## Build and package

The base app needs manifest schema 5, Linux module support, choices fields, and embedding support;
the module's minimum app version is 1.0.11. Existing schema 1/2 modules remain
compatible with the updated host.

```sh
cargo build --manifest-path modules/semantic-search/Cargo.toml --release --locked --lib
python3 modules/ocr/scripts/prepare-runtime.py --platform macos --architecture aarch64 --destination modules/semantic-search/native --cache /tmp/semantic-runtime
python3 scripts/modules/package.py --module-dir modules/semantic-search --library modules/semantic-search/target/release/libcopypaste_module_semantic_search.dylib --platform macos --architecture aarch64 --output /tmp/semantic-search.cpmodule
```

The runtime preparation utility is shared with OCR and pins ONNX Runtime 1.28.
Only its runtime is used; OCR models and OCR code are not dependencies of this
module. Use the same utility for Windows x86_64, Linux x86_64/aarch64, and Android arm/aarch64/x86_64.
Android builds use `scripts/modules/build-android-module.py` with
`--module-dir modules/semantic-search --library-name libcopypaste_module_semantic_search.so`
and the repository's pinned NDK 29.0.13846066 / 16 KiB alignment checks.

Models are never bundled in `.cpmodule`: only the native module, CPU runtime,
signed resource descriptions, and license notices belong in the package.
Packaging requires the existing production signer. Publication and signed
marketplace availability require target-specific native qualification; source
checks or a local artifact do not establish release availability.
Dispatch `provider-module.yml` with `module=semantic-search` and `publish=true`
to build all seven targets, qualify the production-signed packages on macOS,
Windows, Linux x86_64/aarch64, and Android with both pinned models, and publish the signed catalog.

## Validation

```sh
cargo test -p copypaste-core -p copypaste-module-sdk -p copypaste-modules
cargo test --manifest-path modules/semantic-search/Cargo.toml --locked
cargo check --manifest-path modules/semantic-search/Cargo.toml --locked --target x86_64-pc-windows-msvc --lib
cargo check --manifest-path modules/semantic-search/Cargo.toml --locked --target aarch64-linux-android --lib
```

The explicit native test exercises an ephemeral signed package, the real C ABI,
both real model profiles, encrypted corpus indexing, semantic inclusion and
unrelated-result exclusion, switching languages, disabling, and removal:

```sh
COPYPASTE_INFERENCE_WORKER=/absolute/path/to/candidate/copypaste-inference-worker \
COPYPASTE_SEMANTIC_MODELS=/absolute/path/to/staged-models \
COPYPASTE_SEMANTIC_LIBRARY=/absolute/path/to/native-module-library \
COPYPASTE_SEMANTIC_RUNTIME=/absolute/path/to/native-onnx-runtime \
cargo test -p copypaste-modules signed_semantic_package -- --ignored --nocapture
```

`staged-models/<profile-id>` contains exactly the files pinned in
`assets/search-models.json`. Prepare these public assets explicitly for evidence;
the installed host downloads its selection itself. `semantic-evidence` can also
emit a JSON receipt for staged package/data directories. Test keys never become
trusted by the production host. Cross-compilation and platform-parametrized
Flutter tests do not substitute for Android/Windows native execution evidence.

## Third-party assets

The original English model is Apache-2.0 and the E5 model is MIT, as declared in
their upstream cards. Quantized ONNX conversions are pinned to the publisher's
immutable revisions in the signed manifest:

- https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
- https://huggingface.co/intfloat/multilingual-e5-small
- https://huggingface.co/Xenova/all-MiniLM-L6-v2/tree/751bff37182d3f1213fa05d7196b954e230abad9
- https://huggingface.co/Xenova/multilingual-e5-small/tree/761b726dd34fb83930e26aab4e9ac3899aa1fa78

ONNX Runtime's license and notices are in `assets/licenses`. No model weights
are committed to this repository or included in the application build.
