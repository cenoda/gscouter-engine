// Runs the module as Gscouter's workers will: node wasm/run.mjs <module>
// <config> <warmup> <seed> <first> <count> prints gse_run_slice's JSON;
// with "check" in place of the numbers, gse_check's; node wasm/run.mjs
// <module> --catalog prints gse_catalog's; with "parse", gse_parse's (on
// stdout either way, exiting with 1 when the config does not parse).
import { readFileSync } from "node:fs";

const [mod, cfgPath, ...rest] = process.argv.slice(2);
const { instance } = await WebAssembly.instantiate(readFileSync(mod), {});
const x = instance.exports;
if (cfgPath === "--catalog") {
  const r = x.gse_catalog();
  const head = new DataView(x.memory.buffer, r, 8);
  const text = new TextDecoder().decode(new Uint8Array(x.memory.buffer, r + 8, head.getUint32(4, true)));
  x.gse_free_result(r);
  process.stdout.write(text);
  process.exit(0);
}
if (cfgPath === "--parse-many") {
  // each file's gse_parse, as wasm/parity.sh prints the native ones
  for (const f of rest) {
    const cfg = new TextEncoder().encode(readFileSync(f, "utf8"));
    const p = x.gse_alloc(cfg.length);
    new Uint8Array(x.memory.buffer, p, cfg.length).set(cfg);
    const r = x.gse_parse(p, cfg.length);
    const head = new DataView(x.memory.buffer, r, 8);
    const text = new TextDecoder().decode(new Uint8Array(x.memory.buffer, r + 8, head.getUint32(4, true)));
    process.stdout.write(`== ${f} ${head.getUint32(0, true)}\n${text}`);
    x.gse_free_result(r);
    x.gse_free(p, cfg.length);
  }
  process.exit(0);
}
const cfg = new TextEncoder().encode(readFileSync(cfgPath, "utf8"));
const p = x.gse_alloc(cfg.length);
new Uint8Array(x.memory.buffer, p, cfg.length).set(cfg);
const t0 = performance.now();
const r = rest[0] === "check"
  ? x.gse_check(p, cfg.length)
  : rest[0] === "parse"
  ? x.gse_parse(p, cfg.length)
  : x.gse_run_slice(p, cfg.length, Number(rest[0]), BigInt(rest[1]), Number(rest[2]), Number(rest[3]));
const ms = performance.now() - t0;
const head = new DataView(x.memory.buffer, r, 8);
const status = head.getUint32(0, true), len = head.getUint32(4, true);
const text = new TextDecoder().decode(new Uint8Array(x.memory.buffer, r + 8, len));
x.gse_free_result(r);
x.gse_free(p, cfg.length);
if (rest[0] === "parse") {
  process.stdout.write(text);
  process.exit(status);
}
if (status !== 0) {
  console.error(text);
  process.exit(1);
}
console.log(text);
console.error(`${ms.toFixed(0)} ms`);
