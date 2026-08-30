const MAX_SOURCE_MASKS = 32;

const SOURCE_MASKS: Readonly<Record<string, number>> = {
  UNKNOWN: 0x00000000,
  ANY: 0xffffff00,
  KEYBOARD: 0x00000101,
  DPAD: 0x00000201,
  GAMEPAD: 0x00000401,
  TOUCHSCREEN: 0x00001002,
  MOUSE: 0x00002002,
  STYLUS: 0x00004002,
  BLUETOOTH_STYLUS: 0x0000c002,
  TRACKBALL: 0x00010004,
  MOUSE_RELATIVE: 0x00020004,
  TOUCHPAD: 0x00100008,
  TOUCH_NAVIGATION: 0x00200000,
  JOYSTICK: 0x01000010,
  HDMI: 0x02000001,
  SENSOR: 0x04000000,
  ROTARY_ENCODER: 0x00400000,
};

function sourceMask(value: string): number | undefined {
  if (/^0x[0-9a-f]{1,8}$/i.test(value)) {
    return Number.parseInt(value.slice(2), 16);
  }
  const names = value.split(" | ");
  if (names.length === 0) return undefined;
  let mask = 0;
  for (const name of names) {
    if (!Object.hasOwn(SOURCE_MASKS, name)) return undefined;
    const source = SOURCE_MASKS[name];
    if (typeof source !== "number" || !Number.isSafeInteger(source)) return undefined;
    mask = (mask | source) >>> 0;
  }
  return mask;
}

export function parseInputSourceMasks(inputDump: string | undefined): number[] | "unknown" {
  if (inputDump === undefined) return "unknown";
  const sources = inputDump
    .split("\n")
    .map((line) => /^\s*Sources:\s*([^\s].*?)\s*$/.exec(line)?.[1])
    .filter((value): value is string => value !== undefined);
  if (sources.length === 0 || sources.length > MAX_SOURCE_MASKS) return "unknown";
  const masks = sources.map(sourceMask);
  return masks.some((mask) => mask === undefined) ? "unknown" : masks as number[];
}
