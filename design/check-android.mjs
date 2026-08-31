import { readFileSync } from 'node:fs';
import { converter, parse } from 'culori';
import { AA_TEXT, NON_TEXT, over, ratio } from './lib/tokens.mjs';

const rgb = converter('rgb');
const ROOT = new URL('../', import.meta.url);
const android = JSON.parse(readFileSync(new URL('./tokens/android.json', import.meta.url), 'utf8')).android;

const read = (file) => readFileSync(new URL(file, ROOT), 'utf8');

function resources(file, element) {
  const values = Object.fromEntries(
    [...read(file).matchAll(new RegExp(`<${element} name="([^"]+)">([^<]+)</${element}>`, 'g'))]
      .map(([, name, value]) => [name, value]),
  );
  if (!Object.keys(values).length) throw new Error(`${file} has no ${element} resources`);
  return values;
}

const resourceName = (name) => `copypaste_${name.replaceAll('-', '_')}`;

function sourceNames(group) {
  return Object.keys(android[group]).filter((name) => !name.startsWith('$')).map(resourceName).sort();
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function assertSourceMapping(group, values, file) {
  const expected = sourceNames(group);
  const actual = Object.keys(values).sort();
  assert(actual.join(',') === expected.join(','),
    `${file} resources differ from design/tokens/android.json: expected [${expected}], got [${actual}]`);
}

function androidColor(value) {
  assert(/^#[0-9A-F]{8}$/.test(value), `invalid Android ARGB colour: ${value}`);
  const parsed = parse(`#${value.slice(3)}${value.slice(1, 3)}`);
  assert(parsed, `cannot parse Android ARGB colour: ${value}`);
  return rgb(parsed);
}

function composite(color, background) {
  return color.alpha === undefined || color.alpha === 1 ? color : over(color, background);
}

function assertContrast(foreground, background, floor, label) {
  const measured = ratio(foreground, background);
  assert(measured + 1e-9 >= floor,
    `${label} is ${measured.toFixed(2)}:1, needs ${floor}:1`);
}

function checkColours(file) {
  const values = resources(file, 'color');
  assertSourceMapping('color', values, file);
  const colors = Object.fromEntries(Object.entries(values).map(([name, value]) => [name, androidColor(value)]));
  const canvas = colors.copypaste_canvas;
  const surfaces = ['copypaste_canvas', 'copypaste_panel', 'copypaste_surface', 'copypaste_surface_raised', 'copypaste_selected']
    .map((name) => [name, composite(colors[name], canvas)]);

  for (const foreground of ['copypaste_text', 'copypaste_text_muted']) {
    for (const [name, surface] of surfaces) {
      assertContrast(colors[foreground], surface, AA_TEXT, `${foreground} on ${name} in ${file}`);
    }
  }
  assertContrast(colors.copypaste_on_accent, colors.copypaste_accent, AA_TEXT,
    `copypaste_on_accent on copypaste_accent in ${file}`);
  for (const status of ['copypaste_success', 'copypaste_warning', 'copypaste_error']) {
    assertContrast(colors[status], canvas, NON_TEXT, `${status} on copypaste_canvas in ${file}`);
  }

  assert(values.copypaste_qr_surface === '#FFFFFFFF',
    `${file} QR surface must remain true white for true-black QR modules`);
}

const light = 'crates/copypaste-ui/src-tauri/gen/android/app/src/main/res/values/colors.xml';
const dark = 'crates/copypaste-ui/src-tauri/gen/android/app/src/main/res/values-night/colors.xml';
const dimens = 'crates/copypaste-ui/src-tauri/gen/android/app/src/main/res/values/dimens.xml';
const dimensions = resources(dimens, 'dimen');

assertSourceMapping('dimension', dimensions, dimens);
assert(android.color['qr-surface'].$value === '#ffffff', 'QR source token must stay true white');
assert(android.dimension['touch-target'].$value === '48px', 'Android touch target source must stay 48px');
assert(dimensions.copypaste_touch_target === '48dp', 'Android touch target must emit 48dp');
for (const [name, value] of Object.entries(dimensions)) {
  assert(/^\d+(?:\.\d+)?(?:dp|sp)$/.test(value), `${name} is not an Android dp/sp dimension: ${value}`);
}

checkColours(light);
checkColours(dark);
console.log('android: source mappings, generated dimensions, contrast, and QR exception pass');
