// Build the dashboard for Chrome 57 and verify the output really runs there.
//   node build.mjs           production build into dist/
//   node build.mjs --watch   rebuild on change
import * as esbuild from "esbuild";
import * as acorn from "acorn";
import { cpSync, mkdirSync, readFileSync } from "node:fs";

const TARGET = "chrome57"; // Samsung Galaxy Tab 3 Lite (Android 4.2)
const watch = process.argv.includes("--watch");

mkdirSync("dist", { recursive: true });
cpSync("static", "dist", { recursive: true });

const options = {
  entryPoints: ["src/main.tsx"],
  outfile: "dist/app.js",
  bundle: true,
  minify: !watch,
  sourcemap: watch,
  target: TARGET,
  format: "iife",
  jsx: "automatic",
  jsxImportSource: "preact",
  legalComments: "none",
  logLevel: "info",
};

if (watch) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
} else {
  await esbuild.build(options);
  verify("dist/app.js");
}

/**
 * esbuild rewrites syntax for the target, but it cannot polyfill missing
 * runtime APIs. Parse as ES2017 (≈ Chrome 57: async/await yes, object
 * spread / optional chaining no) and scan for APIs newer than Chrome 57.
 */
function verify(file) {
  const code = readFileSync(file, "utf8");
  try {
    acorn.parse(code, { ecmaVersion: 2017, sourceType: "script" });
  } catch (e) {
    console.error(`✗ ${file} uses syntax newer than ES2017: ${e.message}`);
    process.exit(1);
  }
  const tooNew = {
    queueMicrotask: 71, globalThis: 71, "Object.fromEntries": 73, ".flatMap(": 69,
    ".flat(": 69, ".finally(": 63, structuredClone: 98, ".replaceAll(": 85,
    ".matchAll(": 73, "ResizeObserver": 64, "AbortController": 66,
  };
  const hits = Object.keys(tooNew).filter((api) => code.includes(api));
  if (hits.length) {
    console.error("✗ APIs newer than Chrome 57: " + hits.map((h) => `${h} (Chrome ${tooNew[h]})`).join(", "));
    process.exit(1);
  }
  console.log(`✓ ${file}: ${(code.length / 1024).toFixed(1)} KB, ES2017 syntax, no post-Chrome-57 APIs`);
}
