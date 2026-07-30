const __importMetaUrl = require('url').pathToFileURL(__filename).href;
"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// src/index.ts
var index_exports = {};
__export(index_exports, {
  SENSITIVE_DATA_REPLACER: () => SENSITIVE_DATA_REPLACER,
  default: () => getLogger
});
module.exports = __toCommonJS(index_exports);

// src/utils.ts
var import_safe_regex2 = __toESM(require("safe-regex2"), 1);
var SENSITIVE_DATA_REPLACER = "**MASKED**";
var skipError = (aFunction) => {
  try {
    return aFunction();
  } catch {
    return void 0;
  }
};
var parseMaskingPatterns = (maskingRegexString) => {
  if (typeof maskingRegexString !== "string") {
    return void 0;
  }
  const regexStrings = maskingRegexString?.split(/,\s*/).filter((regexStr) => regexStr.trim() !== "");
  return regexStrings?.map((regexStr) => {
    const regexParts = regexStr.match(/^\/(.*?)\/([gimsuy]*)$/);
    if (!regexParts && (0, import_safe_regex2.default)(regexStr)) {
      return skipError(() => new RegExp(regexStr));
    }
    if (regexParts?.[1] && (0, import_safe_regex2.default)(regexParts[1])) {
      return skipError(() => regexParts[2] ? new RegExp(regexParts[1], regexParts[2]) : new RegExp(regexParts[1]));
    }
    return void 0;
  }).filter((regex2) => regex2 !== void 0);
};
var mask = (text, maskingPatterns) => {
  if (!maskingPatterns || typeof text !== "string") {
    return text;
  }
  const endsWithNewline = text.endsWith("\n");
  let maskedText = text;
  maskingPatterns.forEach((maskingRegex) => {
    maskedText = maskedText.replace(maskingRegex, (fullMatch, ...capturedGroupsAndMore) => {
      const capturedGroups = capturedGroupsAndMore.slice(0, capturedGroupsAndMore.length - 2);
      if (capturedGroups.length === 0) {
        return SENSITIVE_DATA_REPLACER;
      }
      let matchedMaskedText = fullMatch;
      capturedGroups.forEach((group) => {
        matchedMaskedText = matchedMaskedText.replace(group, SENSITIVE_DATA_REPLACER);
      });
      return matchedMaskedText;
    });
  });
  if (endsWithNewline && !maskedText.endsWith("\n")) {
    maskedText += "\n";
  }
  return maskedText;
};

// src/index.ts
var import_node_fs = __toESM(require("node:fs"), 1);
var import_node_util = __toESM(require("node:util"), 1);
var import_loglevel = __toESM(require("loglevel"), 1);

// ../../node_modules/.pnpm/chalk@5.6.2/node_modules/chalk/source/vendor/ansi-styles/index.js
var ANSI_BACKGROUND_OFFSET = 10;
var wrapAnsi16 = (offset = 0) => (code) => `\x1B[${code + offset}m`;
var wrapAnsi256 = (offset = 0) => (code) => `\x1B[${38 + offset};5;${code}m`;
var wrapAnsi16m = (offset = 0) => (red, green, blue) => `\x1B[${38 + offset};2;${red};${green};${blue}m`;
var styles = {
  modifier: {
    reset: [0, 0],
    // 21 isn't widely supported and 22 does the same thing
    bold: [1, 22],
    dim: [2, 22],
    italic: [3, 23],
    underline: [4, 24],
    overline: [53, 55],
    inverse: [7, 27],
    hidden: [8, 28],
    strikethrough: [9, 29]
  },
  color: {
    black: [30, 39],
    red: [31, 39],
    green: [32, 39],
    yellow: [33, 39],
    blue: [34, 39],
    magenta: [35, 39],
    cyan: [36, 39],
    white: [37, 39],
    // Bright color
    blackBright: [90, 39],
    gray: [90, 39],
    // Alias of `blackBright`
    grey: [90, 39],
    // Alias of `blackBright`
    redBright: [91, 39],
    greenBright: [92, 39],
    yellowBright: [93, 39],
    blueBright: [94, 39],
    magentaBright: [95, 39],
    cyanBright: [96, 39],
    whiteBright: [97, 39]
  },
  bgColor: {
    bgBlack: [40, 49],
    bgRed: [41, 49],
    bgGreen: [42, 49],
    bgYellow: [43, 49],
    bgBlue: [44, 49],
    bgMagenta: [45, 49],
    bgCyan: [46, 49],
    bgWhite: [47, 49],
    // Bright color
    bgBlackBright: [100, 49],
    bgGray: [100, 49],
    // Alias of `bgBlackBright`
    bgGrey: [100, 49],
    // Alias of `bgBlackBright`
    bgRedBright: [101, 49],
    bgGreenBright: [102, 49],
    bgYellowBright: [103, 49],
    bgBlueBright: [104, 49],
    bgMagentaBright: [105, 49],
    bgCyanBright: [106, 49],
    bgWhiteBright: [107, 49]
  }
};
var modifierNames = Object.keys(styles.modifier);
var foregroundColorNames = Object.keys(styles.color);
var backgroundColorNames = Object.keys(styles.bgColor);
var colorNames = [...foregroundColorNames, ...backgroundColorNames];
function assembleStyles() {
  const codes = /* @__PURE__ */ new Map();
  for (const [groupName, group] of Object.entries(styles)) {
    for (const [styleName, style] of Object.entries(group)) {
      styles[styleName] = {
        open: `\x1B[${style[0]}m`,
        close: `\x1B[${style[1]}m`
      };
      group[styleName] = styles[styleName];
      codes.set(style[0], style[1]);
    }
    Object.defineProperty(styles, groupName, {
      value: group,
      enumerable: false
    });
  }
  Object.defineProperty(styles, "codes", {
    value: codes,
    enumerable: false
  });
  styles.color.close = "\x1B[39m";
  styles.bgColor.close = "\x1B[49m";
  styles.color.ansi = wrapAnsi16();
  styles.color.ansi256 = wrapAnsi256();
  styles.color.ansi16m = wrapAnsi16m();
  styles.bgColor.ansi = wrapAnsi16(ANSI_BACKGROUND_OFFSET);
  styles.bgColor.ansi256 = wrapAnsi256(ANSI_BACKGROUND_OFFSET);
  styles.bgColor.ansi16m = wrapAnsi16m(ANSI_BACKGROUND_OFFSET);
  Object.defineProperties(styles, {
    rgbToAnsi256: {
      value(red, green, blue) {
        if (red === green && green === blue) {
          if (red < 8) {
            return 16;
          }
          if (red > 248) {
            return 231;
          }
          return Math.round((red - 8) / 247 * 24) + 232;
        }
        return 16 + 36 * Math.round(red / 255 * 5) + 6 * Math.round(green / 255 * 5) + Math.round(blue / 255 * 5);
      },
      enumerable: false
    },
    hexToRgb: {
      value(hex) {
        const matches2 = /[a-f\d]{6}|[a-f\d]{3}/i.exec(hex.toString(16));
        if (!matches2) {
          return [0, 0, 0];
        }
        let [colorString] = matches2;
        if (colorString.length === 3) {
          colorString = [...colorString].map((character) => character + character).join("");
        }
        const integer = Number.parseInt(colorString, 16);
        return [
          /* eslint-disable no-bitwise */
          integer >> 16 & 255,
          integer >> 8 & 255,
          integer & 255
          /* eslint-enable no-bitwise */
        ];
      },
      enumerable: false
    },
    hexToAnsi256: {
      value: (hex) => styles.rgbToAnsi256(...styles.hexToRgb(hex)),
      enumerable: false
    },
    ansi256ToAnsi: {
      value(code) {
        if (code < 8) {
          return 30 + code;
        }
        if (code < 16) {
          return 90 + (code - 8);
        }
        let red;
        let green;
        let blue;
        if (code >= 232) {
          red = ((code - 232) * 10 + 8) / 255;
          green = red;
          blue = red;
        } else {
          code -= 16;
          const remainder = code % 36;
          red = Math.floor(code / 36) / 5;
          green = Math.floor(remainder / 6) / 5;
          blue = remainder % 6 / 5;
        }
        const value = Math.max(red, green, blue) * 2;
        if (value === 0) {
          return 30;
        }
        let result = 30 + (Math.round(blue) << 2 | Math.round(green) << 1 | Math.round(red));
        if (value === 2) {
          result += 60;
        }
        return result;
      },
      enumerable: false
    },
    rgbToAnsi: {
      value: (red, green, blue) => styles.ansi256ToAnsi(styles.rgbToAnsi256(red, green, blue)),
      enumerable: false
    },
    hexToAnsi: {
      value: (hex) => styles.ansi256ToAnsi(styles.hexToAnsi256(hex)),
      enumerable: false
    }
  });
  return styles;
}
var ansiStyles = assembleStyles();
var ansi_styles_default = ansiStyles;

// ../../node_modules/.pnpm/chalk@5.6.2/node_modules/chalk/source/vendor/supports-color/index.js
var import_node_process = __toESM(require("node:process"), 1);
var import_node_os = __toESM(require("node:os"), 1);
var import_node_tty = __toESM(require("node:tty"), 1);
function hasFlag(flag, argv = globalThis.Deno ? globalThis.Deno.args : import_node_process.default.argv) {
  const prefix2 = flag.startsWith("-") ? "" : flag.length === 1 ? "-" : "--";
  const position = argv.indexOf(prefix2 + flag);
  const terminatorPosition = argv.indexOf("--");
  return position !== -1 && (terminatorPosition === -1 || position < terminatorPosition);
}
var { env } = import_node_process.default;
var flagForceColor;
if (hasFlag("no-color") || hasFlag("no-colors") || hasFlag("color=false") || hasFlag("color=never")) {
  flagForceColor = 0;
} else if (hasFlag("color") || hasFlag("colors") || hasFlag("color=true") || hasFlag("color=always")) {
  flagForceColor = 1;
}
function envForceColor() {
  if ("FORCE_COLOR" in env) {
    if (env.FORCE_COLOR === "true") {
      return 1;
    }
    if (env.FORCE_COLOR === "false") {
      return 0;
    }
    return env.FORCE_COLOR.length === 0 ? 1 : Math.min(Number.parseInt(env.FORCE_COLOR, 10), 3);
  }
}
function translateLevel(level) {
  if (level === 0) {
    return false;
  }
  return {
    level,
    hasBasic: true,
    has256: level >= 2,
    has16m: level >= 3
  };
}
function _supportsColor(haveStream, { streamIsTTY, sniffFlags = true } = {}) {
  const noFlagForceColor = envForceColor();
  if (noFlagForceColor !== void 0) {
    flagForceColor = noFlagForceColor;
  }
  const forceColor = sniffFlags ? flagForceColor : noFlagForceColor;
  if (forceColor === 0) {
    return 0;
  }
  if (sniffFlags) {
    if (hasFlag("color=16m") || hasFlag("color=full") || hasFlag("color=truecolor")) {
      return 3;
    }
    if (hasFlag("color=256")) {
      return 2;
    }
  }
  if ("TF_BUILD" in env && "AGENT_NAME" in env) {
    return 1;
  }
  if (haveStream && !streamIsTTY && forceColor === void 0) {
    return 0;
  }
  const min = forceColor || 0;
  if (env.TERM === "dumb") {
    return min;
  }
  if (import_node_process.default.platform === "win32") {
    const osRelease = import_node_os.default.release().split(".");
    if (Number(osRelease[0]) >= 10 && Number(osRelease[2]) >= 10586) {
      return Number(osRelease[2]) >= 14931 ? 3 : 2;
    }
    return 1;
  }
  if ("CI" in env) {
    if (["GITHUB_ACTIONS", "GITEA_ACTIONS", "CIRCLECI"].some((key) => key in env)) {
      return 3;
    }
    if (["TRAVIS", "APPVEYOR", "GITLAB_CI", "BUILDKITE", "DRONE"].some((sign) => sign in env) || env.CI_NAME === "codeship") {
      return 1;
    }
    return min;
  }
  if ("TEAMCITY_VERSION" in env) {
    return /^(9\.(0*[1-9]\d*)\.|\d{2,}\.)/.test(env.TEAMCITY_VERSION) ? 1 : 0;
  }
  if (env.COLORTERM === "truecolor") {
    return 3;
  }
  if (env.TERM === "xterm-kitty") {
    return 3;
  }
  if (env.TERM === "xterm-ghostty") {
    return 3;
  }
  if (env.TERM === "wezterm") {
    return 3;
  }
  if ("TERM_PROGRAM" in env) {
    const version = Number.parseInt((env.TERM_PROGRAM_VERSION || "").split(".")[0], 10);
    switch (env.TERM_PROGRAM) {
      case "iTerm.app": {
        return version >= 3 ? 3 : 2;
      }
      case "Apple_Terminal": {
        return 2;
      }
    }
  }
  if (/-256(color)?$/i.test(env.TERM)) {
    return 2;
  }
  if (/^screen|^xterm|^vt100|^vt220|^rxvt|color|ansi|cygwin|linux/i.test(env.TERM)) {
    return 1;
  }
  if ("COLORTERM" in env) {
    return 1;
  }
  return min;
}
function createSupportsColor(stream, options = {}) {
  const level = _supportsColor(stream, {
    streamIsTTY: stream && stream.isTTY,
    ...options
  });
  return translateLevel(level);
}
var supportsColor = {
  stdout: createSupportsColor({ isTTY: import_node_tty.default.isatty(1) }),
  stderr: createSupportsColor({ isTTY: import_node_tty.default.isatty(2) })
};
var supports_color_default = supportsColor;

// ../../node_modules/.pnpm/chalk@5.6.2/node_modules/chalk/source/utilities.js
function stringReplaceAll(string, substring, replacer) {
  let index = string.indexOf(substring);
  if (index === -1) {
    return string;
  }
  const substringLength = substring.length;
  let endIndex = 0;
  let returnValue = "";
  do {
    returnValue += string.slice(endIndex, index) + substring + replacer;
    endIndex = index + substringLength;
    index = string.indexOf(substring, endIndex);
  } while (index !== -1);
  returnValue += string.slice(endIndex);
  return returnValue;
}
function stringEncaseCRLFWithFirstIndex(string, prefix2, postfix, index) {
  let endIndex = 0;
  let returnValue = "";
  do {
    const gotCR = string[index - 1] === "\r";
    returnValue += string.slice(endIndex, gotCR ? index - 1 : index) + prefix2 + (gotCR ? "\r\n" : "\n") + postfix;
    endIndex = index + 1;
    index = string.indexOf("\n", endIndex);
  } while (index !== -1);
  returnValue += string.slice(endIndex);
  return returnValue;
}

// ../../node_modules/.pnpm/chalk@5.6.2/node_modules/chalk/source/index.js
var { stdout: stdoutColor, stderr: stderrColor } = supports_color_default;
var GENERATOR = /* @__PURE__ */ Symbol("GENERATOR");
var STYLER = /* @__PURE__ */ Symbol("STYLER");
var IS_EMPTY = /* @__PURE__ */ Symbol("IS_EMPTY");
var levelMapping = [
  "ansi",
  "ansi",
  "ansi256",
  "ansi16m"
];
var styles2 = /* @__PURE__ */ Object.create(null);
var applyOptions = (object, options = {}) => {
  if (options.level && !(Number.isInteger(options.level) && options.level >= 0 && options.level <= 3)) {
    throw new Error("The `level` option should be an integer from 0 to 3");
  }
  const colorLevel = stdoutColor ? stdoutColor.level : 0;
  object.level = options.level === void 0 ? colorLevel : options.level;
};
var chalkFactory = (options) => {
  const chalk2 = (...strings) => strings.join(" ");
  applyOptions(chalk2, options);
  Object.setPrototypeOf(chalk2, createChalk.prototype);
  return chalk2;
};
function createChalk(options) {
  return chalkFactory(options);
}
Object.setPrototypeOf(createChalk.prototype, Function.prototype);
for (const [styleName, style] of Object.entries(ansi_styles_default)) {
  styles2[styleName] = {
    get() {
      const builder = createBuilder(this, createStyler(style.open, style.close, this[STYLER]), this[IS_EMPTY]);
      Object.defineProperty(this, styleName, { value: builder });
      return builder;
    }
  };
}
styles2.visible = {
  get() {
    const builder = createBuilder(this, this[STYLER], true);
    Object.defineProperty(this, "visible", { value: builder });
    return builder;
  }
};
var getModelAnsi = (model, level, type, ...arguments_) => {
  if (model === "rgb") {
    if (level === "ansi16m") {
      return ansi_styles_default[type].ansi16m(...arguments_);
    }
    if (level === "ansi256") {
      return ansi_styles_default[type].ansi256(ansi_styles_default.rgbToAnsi256(...arguments_));
    }
    return ansi_styles_default[type].ansi(ansi_styles_default.rgbToAnsi(...arguments_));
  }
  if (model === "hex") {
    return getModelAnsi("rgb", level, type, ...ansi_styles_default.hexToRgb(...arguments_));
  }
  return ansi_styles_default[type][model](...arguments_);
};
var usedModels = ["rgb", "hex", "ansi256"];
for (const model of usedModels) {
  styles2[model] = {
    get() {
      const { level } = this;
      return function(...arguments_) {
        const styler = createStyler(getModelAnsi(model, levelMapping[level], "color", ...arguments_), ansi_styles_default.color.close, this[STYLER]);
        return createBuilder(this, styler, this[IS_EMPTY]);
      };
    }
  };
  const bgModel = "bg" + model[0].toUpperCase() + model.slice(1);
  styles2[bgModel] = {
    get() {
      const { level } = this;
      return function(...arguments_) {
        const styler = createStyler(getModelAnsi(model, levelMapping[level], "bgColor", ...arguments_), ansi_styles_default.bgColor.close, this[STYLER]);
        return createBuilder(this, styler, this[IS_EMPTY]);
      };
    }
  };
}
var proto = Object.defineProperties(() => {
}, {
  ...styles2,
  level: {
    enumerable: true,
    get() {
      return this[GENERATOR].level;
    },
    set(level) {
      this[GENERATOR].level = level;
    }
  }
});
var createStyler = (open, close, parent) => {
  let openAll;
  let closeAll;
  if (parent === void 0) {
    openAll = open;
    closeAll = close;
  } else {
    openAll = parent.openAll + open;
    closeAll = close + parent.closeAll;
  }
  return {
    open,
    close,
    openAll,
    closeAll,
    parent
  };
};
var createBuilder = (self, _styler, _isEmpty) => {
  const builder = (...arguments_) => applyStyle(builder, arguments_.length === 1 ? "" + arguments_[0] : arguments_.join(" "));
  Object.setPrototypeOf(builder, proto);
  builder[GENERATOR] = self;
  builder[STYLER] = _styler;
  builder[IS_EMPTY] = _isEmpty;
  return builder;
};
var applyStyle = (self, string) => {
  if (self.level <= 0 || !string) {
    return self[IS_EMPTY] ? "" : string;
  }
  let styler = self[STYLER];
  if (styler === void 0) {
    return string;
  }
  const { openAll, closeAll } = styler;
  if (string.includes("\x1B")) {
    while (styler !== void 0) {
      string = stringReplaceAll(string, styler.close, styler.open);
      styler = styler.parent;
    }
  }
  const lfIndex = string.indexOf("\n");
  if (lfIndex !== -1) {
    string = stringEncaseCRLFWithFirstIndex(string, closeAll, openAll, lfIndex);
  }
  return openAll + string + closeAll;
};
Object.defineProperties(createChalk.prototype, styles2);
var chalk = createChalk();
var chalkStderr = createChalk({ level: stderrColor ? stderrColor.level : 0 });
var source_default = chalk;

// src/index.ts
var import_loglevel_plugin_prefix = __toESM(require("loglevel-plugin-prefix"), 1);

// ../../node_modules/.pnpm/ansi-regex@6.2.2/node_modules/ansi-regex/index.js
function ansiRegex({ onlyFirst = false } = {}) {
  const ST = "(?:\\u0007|\\u001B\\u005C|\\u009C)";
  const osc = `(?:\\u001B\\][\\s\\S]*?${ST})`;
  const csi = "[\\u001B\\u009B][[\\]()#;?]*(?:\\d{1,4}(?:[;:]\\d{0,4})*)?[\\dA-PR-TZcf-nq-uy=><~]";
  const pattern = `${osc}|${csi}`;
  return new RegExp(pattern, onlyFirst ? void 0 : "g");
}

// ../../node_modules/.pnpm/strip-ansi@7.1.2/node_modules/strip-ansi/index.js
var regex = ansiRegex();
function stripAnsi(string) {
  if (typeof string !== "string") {
    throw new TypeError(`Expected a \`string\`, got \`${typeof string}\``);
  }
  return string.replace(regex, "");
}

// src/index.ts
import_loglevel_plugin_prefix.default.reg(import_loglevel.default);
var DEFAULT_LEVEL = process.env.WDIO_DEBUG ? "trace" : "info";
var COLORS = {
  error: "red",
  warn: "yellow",
  info: "cyanBright",
  debug: "green",
  trace: "cyan",
  progress: "magenta"
};
var matches = {
  COMMAND: "COMMAND",
  BIDICOMMAND: "BIDI COMMAND",
  DATA: "DATA",
  RESULT: "RESULT",
  BIDIRESULT: "BIDI RESULT"
};
var SERIALIZERS = [{
  /**
   * display error stack
   */
  matches: (err) => err instanceof Error,
  serialize: (err) => err.stack
}, {
  /**
   * color commands blue
   */
  matches: (log2) => log2 === matches.COMMAND || log2 === matches.BIDICOMMAND,
  serialize: (log2) => source_default.magenta(log2)
}, {
  /**
   * color data yellow
   */
  matches: (log2) => log2 === matches.DATA,
  serialize: (log2) => source_default.yellow(log2)
}, {
  /**
   * color result cyan
   */
  matches: (log2) => log2 === matches.RESULT || log2 === matches.BIDIRESULT,
  serialize: (log2) => source_default.cyan(log2)
}];
var loggers = import_loglevel.default.getLoggers();
var logLevelsConfig = {};
var maskingPatternsConfig = {};
var logCache = /* @__PURE__ */ new Set();
var logFile;
var originalFactory = import_loglevel.default.methodFactory;
var wdioLoggerMethodFactory = (wdioLogger) => function(methodName, logLevel, loggerName) {
  const rawMethod = originalFactory(methodName, logLevel, loggerName);
  return (...args) => {
    if (!logFile && process.env.WDIO_LOG_PATH) {
      logFile = import_node_fs.default.createWriteStream(process.env.WDIO_LOG_PATH);
    }
    const match = Object.values(matches).filter((x) => args[0].endsWith(`: ${x}`))[0];
    if (match) {
      const prefixStr = args.shift().slice(0, -match.length - 1);
      args.unshift(prefixStr, match);
    }
    args = args.map((arg) => {
      for (const s of SERIALIZERS) {
        if (s.matches(arg)) {
          return s.serialize(arg);
        }
      }
      return arg;
    });
    const unmaskedLogText = stripAnsi(`${import_node_util.default.format.apply(this, args)}
`);
    const maskedLogText = mask(unmaskedLogText, wdioLogger.maskingPatterns);
    if (logFile && logFile.writable) {
      if (logCache.size) {
        logCache.forEach((log2) => {
          if (logFile) {
            logFile.write(log2);
          }
        });
        logCache.clear();
      }
      if (!logsContainInitPackageError(unmaskedLogText)) {
        return logFile.write(maskedLogText);
      }
      logFile.write(maskedLogText);
    }
    logCache.add(maskedLogText);
    if (maskedLogText === unmaskedLogText) {
      rawMethod(...args);
    } else {
      rawMethod(maskedLogText.replace(/\n$/, ""));
    }
  };
};
var progress = function(data) {
  if (process.stdout.isTTY && this.getLevel() <= import_loglevel.default.levels.INFO) {
    const level = "progress";
    const timestampFormatter = source_default.gray((/* @__PURE__ */ new Date()).toISOString());
    const levelFormatter = source_default[COLORS[level]](level.toUpperCase());
    const nameFormatter = source_default.whiteBright(this.name);
    const _data = data.length > 0 ? `${timestampFormatter} ${levelFormatter} ${nameFormatter}: ${data}` : "\r\x1B[K\x1B[?25h";
    process.stdout.write("\x1B[?25l");
    process.stdout.write(`${_data}\r`);
  }
};
function getLogger(name) {
  if (loggers[name]) {
    return loggers[name];
  }
  let logLevel = process.env.WDIO_LOG_LEVEL || DEFAULT_LEVEL;
  const logLevelName = getLogLevelName(name);
  if (logLevelsConfig[logLevelName]) {
    logLevel = logLevelsConfig[logLevelName];
  }
  loggers[name] = import_loglevel.default.getLogger(name);
  const logger = loggers[name];
  logger.setLevel(logLevel);
  logger.maskingPatterns = maskingPatternsConfig[name] ?? parseMaskingPatterns(process.env.WDIO_LOG_MASKING_PATTERNS);
  logger.progress = progress;
  logger.methodFactory = wdioLoggerMethodFactory(logger);
  import_loglevel_plugin_prefix.default.apply(logger, {
    template: "%t %l %n:",
    timestampFormatter: (date) => source_default.gray(date.toISOString()),
    levelFormatter: (level) => source_default[COLORS[level]](level.toUpperCase()),
    nameFormatter: (name2) => source_default.whiteBright(name2)
  });
  return logger;
}
getLogger.waitForBuffer = async () => new Promise((resolve) => {
  if (logFile && Array.isArray(logFile.writableBuffer) && logFile.writableBuffer.length !== 0) {
    return setTimeout(async () => {
      await getLogger.waitForBuffer();
      resolve();
    }, 20);
  }
  resolve();
});
getLogger.setLevel = (name, level) => loggers[name].setLevel(level);
getLogger.clearLogger = () => {
  if (logFile) {
    logFile.end();
  }
  logFile = null;
};
getLogger.setLogLevelsConfig = (logLevels = {}, wdioLogLevel = DEFAULT_LEVEL) => {
  if (process.env.WDIO_LOG_LEVEL === void 0) {
    process.env.WDIO_LOG_LEVEL = wdioLogLevel;
  }
  logLevelsConfig = {};
  Object.entries(logLevels).forEach(([logName, logLevel]) => {
    const logLevelName = getLogLevelName(logName);
    logLevelsConfig[logLevelName] = logLevel;
  });
  Object.keys(loggers).forEach((logName) => {
    const logLevelName = getLogLevelName(logName);
    const logLevel = typeof logLevelsConfig[logLevelName] !== "undefined" ? logLevelsConfig[logLevelName] : process.env.WDIO_LOG_LEVEL;
    loggers[logName].setLevel(logLevel);
  });
};
getLogger.setMaskingPatterns = (pattern) => {
  if (typeof pattern === "string") {
    if (process.env.WDIO_LOG_MASKING_PATTERNS === void 0) {
      process.env.WDIO_LOG_MASKING_PATTERNS = pattern;
    }
  } else if (typeof pattern === "object") {
    maskingPatternsConfig = Object.entries(pattern).reduce((acc, [logName, maskingPatternsString]) => {
      acc[logName] = parseMaskingPatterns(maskingPatternsString);
      return acc;
    }, maskingPatternsConfig);
  } else {
    throw new Error(`Invalid pattern property, expected \`string\` or \`Record<string, string>\` but received \`${typeof pattern}\``);
  }
  Object.keys(loggers).forEach((logName) => {
    const maskingPatterns = maskingPatternsConfig[logName] ?? parseMaskingPatterns(process.env.WDIO_LOG_MASKING_PATTERNS);
    loggers[logName].maskingPatterns = maskingPatterns;
  });
};
var getLogLevelName = (logName) => logName.split(":").shift();
function logsContainInitPackageError(logText) {
  return ERROR_LOG_VALIDATOR.every((pattern) => logText.includes(pattern));
}
var ERROR_LOG_VALIDATOR = [
  "Couldn't find plugin",
  "neither as wdio scoped package",
  "nor as community package",
  "Please make sure you have it installed"
];
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  SENSITIVE_DATA_REPLACER
});
