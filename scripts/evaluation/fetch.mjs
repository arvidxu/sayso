// Fetches only the start of each archive, enough for the recordings asked for.
//   node scripts/evaluation/fetch.mjs [set] [language ...]
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { constants, gunzipSync } from "node:zlib";

export const root = resolve(import.meta.dirname, "../..");

const blockSize = 512;

// [name, bytes] in archive order; a file cut off by the end of `bytes` is left out.
export function tarFiles(bytes) {
  const files = [];
  let at = 0;
  while (at + blockSize <= bytes.length) {
    const header = bytes.subarray(at, at + blockSize);
    const text = (from, to) => header.subarray(from, to).toString("utf8").replace(/\0.*$/s, "");
    const name = text(0, 100);
    if (name === "") break;
    const size = Number.parseInt(text(124, 136).trim() || "0", 8);
    const body = at + blockSize;
    if (body + size > bytes.length) break;
    const isFile = ["", "0"].includes(text(156, 157));
    if (isFile) files.push([name, bytes.subarray(body, body + size)]);
    at = body + Math.ceil(size / blockSize) * blockSize;
  }
  return files;
}

export function transcripts(table) {
  const said = new Map();
  for (const line of table.split("\n")) {
    const [, file, text] = line.split("\t");
    if (file && text) said.set(file, text.trim());
  }
  return said;
}

// The server now and then answers a request with a passing error, or none at all, so a failed
// one is tried up to `tries` times in all, waiting `pauseMs` longer after each failure.
const tries = 5;
const pauseMs = 3000;

async function get(url, bytes) {
  const headers = bytes ? { Range: `bytes=0-${bytes - 1}` } : {};
  for (let attempt = 1; ; attempt += 1) {
    const response = await fetch(url, { headers }).catch((error) => error);
    if (response.ok) return Buffer.from(await response.arrayBuffer());
    const answered = response instanceof Response;
    const isPassing = !answered || response.status === 429 || response.status >= 500;
    if (!isPassing || attempt === tries) {
      throw answered ? new Error(`${url} answered ${response.status}`) : response;
    }
    await new Promise((later) => setTimeout(later, pauseMs * attempt));
  }
}

async function fetchLanguage(set, language, name, into) {
  const at = (path) => `${set.base}/${path.replace("{name}", name)}`;
  const said = transcripts((await get(at(set.transcripts))).toString("utf8"));
  const packed = await get(at(set.audio), set.audioBytes);
  // The archive is cut short on purpose, so the end of the stream is missing.
  const archive = gunzipSync(packed, { finishFlush: constants.Z_SYNC_FLUSH });
  const lines = [];
  mkdirSync(join(into, language), { recursive: true });
  for (const [path, bytes] of tarFiles(archive)) {
    const file = path.split("/").pop();
    const text = said.get(file);
    if (!text || lines.length >= set.clips) continue;
    writeFileSync(join(into, language, file), bytes);
    lines.push([language, `${language}/${file}`, text.replace(/\s+/g, " ")].join("\t"));
  }
  return lines;
}

// A set served row by row: takes the rows that match `set.only` and are long enough, up to
// `set.clips` of them, and labels them all with the languages in `set.spoken`.
async function fetchRows(set, into) {
  const lines = [];
  const folder = set.spoken.replaceAll("+", "-");
  mkdirSync(join(into, folder), { recursive: true });
  for (let offset = 0; lines.length < set.clips; offset += set.pageRows) {
    const url = `${set.rows}&offset=${offset}&length=${set.pageRows}`;
    const page = JSON.parse((await get(url)).toString("utf8"));
    if (!page.rows?.length) break;
    for (const { row } of page.rows) {
      const wanted = Object.entries(set.only ?? {}).every(([field, value]) => row[field] === value);
      const text = row[set.textField]?.trim();
      const source = row[set.audioField]?.[0]?.src;
      if (!wanted || !text || !source || (row.duration ?? 0) < set.minSecs) continue;
      if (lines.length >= set.clips) break;
      const file = `${folder}/${lines.length}.wav`;
      writeFileSync(join(into, file), await get(source));
      lines.push([set.spoken, file, text.replace(/\s+/g, " ")].join("\t"));
    }
  }
  return lines;
}

if (import.meta.main) {
  const sets = JSON.parse(readFileSync(join(root, "evaluation-sets.json"), "utf8"));
  const [setName = Object.keys(sets)[0], ...wanted] = process.argv.slice(2);
  const set = sets[setName];
  if (set?.joins) throw new Error(`'${setName}' is made from '${set.joins}' by scripts/evaluation/mix.mjs`);
  if (!set) throw new Error(`evaluation-sets.json has no set '${setName}'`);
  const into = join(root, "target/evaluation", setName);
  const isWanted = (language) => wanted.length === 0 || wanted.includes(language);
  const fetched = [];
  const manifest = [];
  if (set.rows && isWanted(set.spoken)) {
    manifest.push(...(await fetchRows(set, into)));
    fetched.push(set.spoken);
    console.log(`${set.spoken}: ${manifest.length} recordings`);
  }
  for (const [language, name] of Object.entries(set.languages ?? {})) {
    if (!isWanted(language)) continue;
    const lines = await fetchLanguage(set, language, name, into);
    fetched.push(language);
    console.log(`${language}: ${lines.length} recordings`);
    manifest.push(...lines);
  }
  // The lines of languages not fetched this time stay as they were.
  const path = join(into, "manifest.tsv");
  const kept = existsSync(path)
    ? readFileSync(path, "utf8")
        .split("\n")
        .filter((line) => line && !fetched.includes(line.split("\t")[0]))
    : [];
  writeFileSync(path, `${[...kept, ...manifest].join("\n")}\n`);
  console.log(path);
}
