# Semantic inference lifecycle

The application runtime owns history, encrypted vectors, indexing admission,
query caching, and notifications. Search-provider native libraries, tokenizers,
and ONNX sessions are loaded only in a dedicated local inference process.

## Work and results

One host scheduler processes foreground query embeddings before the next bounded
background index fragment. The query queue and query-vector cache each retain at
most 32 entries, and a query already running cannot be queued again. Latest
queries take priority. Foreground History and Quick Paste reads return lexical
results without waiting for model startup or computation. Successful embeddings
publish an Items notification, after which the current query can include semantic
matches. Stale queries do not replace the caller's current query state.

Only new or changed text fragments need indexing. Index coverage and cached
query vectors survive worker exit. Empty or already indexed history does not
perform a synthetic inference to establish readiness. Clipboard capture and
synchronization remain independent of model residency.

## Process ownership

Desktop reuses the packaged daemon executable with `--inference-worker`, before
normal daemon bootstrap. This mode opens no clipboard, history database,
keyring, or application IPC listener. Windows suppresses a console window.

Android binds a non-exported service in `:inference`, transfers a private socket
through Binder, and invokes the same Rust worker protocol through JNI. The main
process initializes no history runtime in the worker. Service teardown explicitly
terminates the dedicated process, because unbinding alone permits Android to
cache process-global allocator memory. No new foreground-service notification is
needed for this bound worker.

Signed package, inventory, model resources, command, and preference validation
still apply before native execution. The inherited channel carries bounded
length-prefixed messages, with one request in flight. Serialization buffers are
zeroized. Database handles and encryption keys never cross this channel. Text
and vectors are not logged or sent to a network service.

## Idle, cancellation, and failure

The idle owner runs independently of UI visibility and indexing polling. It
reaps the worker 60 seconds after the last completed invocation, provided no
invocation is active. It cannot terminate an active model operation for being
idle. Subsequent work starts a new process lazily.

Disabling/removing the module interrupts active IPC, waits for process teardown,
and prevents previously admitted work from starting. Model changes stop the old
process before another model is loaded. Storage publication rechecks the current
module configuration, so disabled or replaced models cannot publish late results.
Shutdown cancels inference before joining the host scheduler.

A failed or crashed worker yields lexical results. The failed query is bounded
in the query cache; a later foreground read may request it again after five
seconds. A single IPC response has a 60-second execution limit, after which that
worker is terminated. There is no automatic retry of a failed invocation.

## Validation

Portable tests cover framing limits, Unicode, cancellation, stale admission,
model replacement, idle exit/reload, immediate lexical results during blocked
inference, duplicate admission, empty-history startup, and cached-vector results
after process exit. Real-model qualification also waits for asynchronous query
publication before checking its existing retrieval assertions.

On macOS, the maintained scenario creates a temporary registry and copies only
signed module/model assets; it never opens or changes installed history or
preferences:

```sh
cargo build -p copypaste-modules --release \
  --bin copypaste-inference-worker --example semantic-memory
./target/release/examples/semantic-memory \
  /absolute/path/to/signed/semantic-package \
  /absolute/path/to/semantic-module-data \
  /absolute/path/to/candidate/copypaste-inference-worker \
  /absolute/path/to/evidence/receipt.json
```

The receipt records parent and worker physical footprint, cross-language
retrieval, actual 60-second idle exit, identical vectors after reload,
disable/enable, and parent shutdown. Cross-compilation does not substitute for
Android or Windows process-lifecycle and memory evidence on those platforms.
