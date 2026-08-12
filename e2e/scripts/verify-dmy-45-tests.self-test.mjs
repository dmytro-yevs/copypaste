import assert from "node:assert/strict";

import {
  checkWiring,
  compareCollection,
  expected,
  verifierCommand,
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
    test: `${verifierCommand} && xvfb-run -a -s "-screen 0 1280x900x24" vitest run`,
    "test:dmy-45:verify": verifierCommand,
    "test:dmy-45": `${verifierCommand} --run`,
    "test:dmy-45:browser": 'xvfb-run -a -s "-screen 0 1280x900x24" npm run test:dmy-45 --',
    "test:dmy-45:browser-repeat":
      "npm run test:dmy-45:browser && npm run test:dmy-45:browser && npm run test:dmy-45:browser",
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

console.log("DMY-45 verifier self-test passed");
