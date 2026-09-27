import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { access, chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));

async function executable(path, body) {
  await writeFile(path, body, "utf8");
  await chmod(path, 0o755);
}

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "copypaste-dev-launcher-test-"));
  const ui = join(root, "crates", "copypaste-ui");
  const bin = join(root, "mock-bin");
  const record = join(root, "record.log");
  const mockTemp = join(root, "mock-temp");
  await Promise.all([
    mkdir(join(ui, "scripts"), { recursive: true }),
    mkdir(join(ui, "public"), { recursive: true }),
    mkdir(join(root, "target", "debug"), { recursive: true }),
    mkdir(bin, { recursive: true }),
    mkdir(mockTemp, { recursive: true }),
  ]);
  await Promise.all([
    writeFile(join(ui, "scripts", "dev-web-daemon.sh"), await readFile(join(scriptsDir, "dev-web-daemon.sh")), "utf8"),
    writeFile(join(ui, "scripts", "dev-native.sh"), await readFile(join(scriptsDir, "dev-native.sh")), "utf8"),
    writeFile(join(ui, "scripts", "web-bridge-runtime.sh"), await readFile(join(scriptsDir, "web-bridge-runtime.sh")), "utf8"),
  ]);
  const common = `#!/usr/bin/env sh
set -eu
printf '%s data=%s socket=%s key=%s args=%s\\n' "$(basename \"$0\")" "\${COPYPASTE_DATA_DIR:-}" "\${COPYPASTE_SOCKET:-}" "\${COPYPASTE_EPHEMERAL_KEY:-}" "$*" >> "$MOCK_RECORD"
`;
  await Promise.all([
    executable(join(bin, "cargo"), `${common}
case " $* " in *" run "*) printf 'VITE_COPYPASTE_WEB_BRIDGE_URL=http://127.0.0.1:9876\\nVITE_COPYPASTE_WEB_BRIDGE_TOKEN=test-token\\n' > "$COPYPASTE_WEB_BRIDGE_ENV_FILE";; esac
`),
    executable(join(bin, "copypaste"), `${common}
exit 0
`),
    executable(join(bin, "copypaste-daemon"), `${common}
exit 0
`),
    executable(join(bin, "copypaste-web-bridge"), `${common}
printf 'VITE_COPYPASTE_WEB_BRIDGE_URL=http://127.0.0.1:9876\\nVITE_COPYPASTE_WEB_BRIDGE_TOKEN=test-token\\n' > "$COPYPASTE_WEB_BRIDGE_ENV_FILE"
`),
    executable(join(bin, "npm"), `${common}
exit 0
`),
    executable(join(bin, "curl"), "#!/usr/bin/env sh\nexit 1\n"),
    executable(join(bin, "mktemp"), `#!/usr/bin/env sh
set -eu
printf 'mktemp args=%s\\n' "$*" >> "$MOCK_RECORD"
case "$*" in
  *cpd.XXXXXX) target="$MOCK_TEMP/cpd"; mkdir -p "$target"; printf '%s\\n' "$target" ;;
  *copypaste-web-bridge.XXXXXX) target="$MOCK_TEMP/bridge-env"; : > "$target"; printf '%s\\n' "$target" ;;
  *.copypaste-web-bridge.XXXXXX) target="$MOCK_TEMP/runtime"; : > "$target"; printf '%s\\n' "$target" ;;
  *) target="$MOCK_TEMP/other"; : > "$target"; printf '%s\\n' "$target" ;;
esac
`),
  ]);
  for (const name of ["copypaste", "copypaste-daemon", "copypaste-web-bridge"]) {
    await executable(join(root, "target", "debug", name), `#!/usr/bin/env sh
exec "${join(bin, name)}" "$@"
`);
  }
  return { root, ui, bin, record, mockTemp };
}

async function runLauncher(name) {
  const testFixture = await fixture();
  try {
    const result = spawnSync("sh", [join(testFixture.ui, "scripts", name)], {
      cwd: testFixture.root,
      encoding: "utf8",
      env: {
        ...process.env,
        COPYPASTE_DATA_DIR: "/production-data-must-not-be-used",
        COPYPASTE_SOCKET: "/production.sock",
        MOCK_RECORD: testFixture.record,
        MOCK_TEMP: testFixture.mockTemp,
        PATH: `${testFixture.bin}:${process.env.PATH}`,
      },
    });
    assert.equal(result.status, 0, result.stderr);
    const record = await readFile(testFixture.record, "utf8");
    await assert.rejects(access(join(testFixture.mockTemp, "cpd")));
    return record;
  } finally {
    await rm(testFixture.root, { recursive: true, force: true });
  }
}

function assertIsolatedDaemon(record) {
  assert.match(record, /mktemp args=-d \/tmp\/cpd\.XXXXXX/);
  assert.match(record, /copypaste-daemon data=.*\/mock-temp\/cpd socket=.*\/mock-temp\/cpd\/daemon\.sock key=1 args=--foreground --data-dir .*\/mock-temp\/cpd/);
  assert.match(record, /copypaste data=.*\/mock-temp\/cpd socket=.*\/mock-temp\/cpd\/daemon\.sock key=1 args=status/);
  assert.doesNotMatch(record, /production-data-must-not-be-used|production\.sock/);
}

test("web launcher rebuilds an isolated fake-clipboard daemon", async () => {
  const record = await runLauncher("dev-web-daemon.sh");

  assertIsolatedDaemon(record);
  assert.match(record, /cargo .*copypaste-daemon\/dev-ephemeral-key,copypaste-daemon\/dev-fake-clipboard/);
  assert.match(record, /npm .*args=run dev:web/);
});

test("native launcher rebuilds an isolated ephemeral-key daemon", async () => {
  const record = await runLauncher("dev-native.sh");

  assertIsolatedDaemon(record);
  assert.match(record, /cargo .*copypaste-daemon\/dev-ephemeral-key/);
  assert.doesNotMatch(record, /cargo .*dev-fake-clipboard/);
  assert.match(record, /npm .*args=run tauri -- dev/);
});
