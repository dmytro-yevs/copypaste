import assert from "node:assert/strict";

import {
  checkWiring,
  compareCollection,
  compareExecution,
  expected,
  guardedVerifyCommand,
  requiredScripts,
  verifierCommand,
  xvfbCommand,
} from "./verify-dmy-45-tests.mjs";

const workflow = {
  jobs: {
    browser: {
      steps: [
        { "working-directory": "e2e", run: "npm run test:dmy-45:browser-repeat" },
        { "working-directory": "e2e", run: "npm test" },
      ],
    },
  },
};

const packageJson = {
  scripts: {
    ...requiredScripts,
    "test:dmy-45:browser-repeat": Array(3).fill("npm run test:dmy-45:browser").join(" && "),
  },
};

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function assertWiringPasses(w = workflow, p = packageJson) {
  assert.deepEqual(checkWiring({ workflow: w, packageJson: p }), []);
}

function assertWiringFails(label, mutate) {
  const w = clone(workflow);
  const p = clone(packageJson);
  mutate(w, p);
  assert.notDeepEqual(checkWiring({ workflow: w, packageJson: p }), [], label);
}

assertWiringPasses();
assertWiringFails("one repeat is rejected", (_w, p) => {
  p.scripts["test:dmy-45:browser-repeat"] = "npm run test:dmy-45:browser";
});
assertWiringFails("wrong order is rejected", (w) => {
  w.jobs.browser.steps.reverse();
});
assertWiringFails("missing focused step is rejected", (w) => {
  w.jobs.browser.steps = w.jobs.browser.steps.filter(
    (step) => step.run !== "npm run test:dmy-45:browser-repeat",
  );
});
assertWiringFails("missing full suite is rejected", (w) => {
  w.jobs.browser.steps = w.jobs.browser.steps.filter((step) => step.run !== "npm test");
});

for (const [label, run] of [
  ["focused", "npm run test:dmy-45:browser-repeat"],
  ["full suite", "npm test"],
]) {
  assertWiringFails(`continue-on-error on the ${label} step is rejected`, (w) => {
    w.jobs.browser.steps.find((step) => step.run === run)["continue-on-error"] = true;
  });
  assertWiringFails(`if on the ${label} step is rejected`, (w) => {
    w.jobs.browser.steps.find((step) => step.run === run).if = false;
  });
}

assertWiringFails("a collecting-but-not-running focused script is rejected", (_w, p) => {
  p.scripts["test:dmy-45:browser"] = `${xvfbCommand} npm run test:dmy-45:verify`;
});
assertWiringFails("a zero-match suffix on the full suite is rejected", (_w, p) => {
  p.scripts.test = `${guardedVerifyCommand} && ${xvfbCommand} vitest run -t nothing`;
});
assertWiringFails("dropping the pinned Xvfb screen is rejected", (_w, p) => {
  p.scripts["test:dmy-45:browser"] = "xvfb-run npm run test:dmy-45 --";
});
assertWiringFails("dropping the self-test from the verify chain is rejected", (_w, p) => {
  p.scripts["test:dmy-45:verify"] = verifierCommand;
});
assertWiringFails("dropping the self-test from the focused script is rejected", (_w, p) => {
  p.scripts["test:dmy-45"] = `${verifierCommand} --run`;
});
assertWiringFails("dropping the self-test from the full suite is rejected", (_w, p) => {
  p.scripts.test = `${verifierCommand} && ${xvfbCommand} vitest run`;
});
assertWiringFails("dropping the self-test script is rejected", (_w, p) => {
  delete p.scripts["test:dmy-45:self-test"];
});
assertWiringFails("dropping the verifier from the full suite is rejected", (_w, p) => {
  p.scripts.test = `${xvfbCommand} vitest run`;
});

assert.equal(compareCollection(expected).ok, true);
assert.equal(compareCollection([]).ok, false, "zero collection is rejected");
assert.equal(
  compareCollection([...expected, { file: "tests/other.e2e.test.ts", name: "extra" }]).ok,
  false,
  "extra collection is rejected",
);
assert.equal(
  compareCollection([
    { ...expected[0], name: "the bulk bar > bulk delete skips confirmation" },
    expected[1],
    expected[2],
  ]).ok,
  false,
  "name drift is rejected",
);

assert.equal(compareExecution({ numPassedTests: expected.length, numFailedTests: 0 }), true);
assert.equal(
  compareExecution({ numPassedTests: 0, numFailedTests: 0 }),
  false,
  "a statically collected but never executed run is rejected",
);
assert.equal(
  compareExecution({ numPassedTests: expected.length - 1, numFailedTests: 0 }),
  false,
  "a run that skips one focused test is rejected",
);
assert.equal(
  compareExecution({ numPassedTests: expected.length, numFailedTests: 1 }),
  false,
  "a run with a failure is rejected",
);

console.log("DMY-45 verifier self-test passed");
