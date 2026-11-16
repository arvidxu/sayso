// Joins recordings in different languages, with silence between; languages read like `zh+en`.
//   node scripts/evaluation/mix.mjs [set]
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");

const floatFormat = 3;

function readFloatWav(bytes, name = "the file") {
  let at = 12;
  let format;
  let data;
  while (at + 8 <= bytes.length) {
    const tag = bytes.toString("latin1", at, at + 4);
    const size = bytes.readUInt32LE(at + 4);
    const body = at + 8;
    if (tag === "fmt ") {
      format = {
        tag: bytes.readUInt16LE(body),
        channels: bytes.readUInt16LE(body + 2),
        rate: bytes.readUInt32LE(body + 4),
        bits: bytes.readUInt16LE(body + 14),
      };
    } else if (tag === "data") {
      data = bytes.subarray(body, Math.min(body + size, bytes.length));
    }
    at = body + size + (size % 2);
  }
  if (!format || !data || format.tag !== floatFormat || format.bits !== 32) {
    throw new Error(`${name} is not a WAV file of 32-bit floating-point samples`);
  }
  // A file cut off inside a frame keeps only its whole frames.
  const frame = format.channels * 4;
  return { rate: format.rate, channels: format.channels, data: data.subarray(0, data.length - (data.length % frame)) };
}

function writeFloatWav(rate, channels, data) {
  const header = Buffer.alloc(44);
  const frame = channels * 4;
  header.write("RIFF", 0, "latin1");
  header.writeUInt32LE(36 + data.length, 4);
  header.write("WAVEfmt ", 8, "latin1");
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(floatFormat, 20);
  header.writeUInt16LE(channels, 22);
  header.writeUInt32LE(rate, 24);
  header.writeUInt32LE(rate * frame, 28);
  header.writeUInt16LE(frame, 32);
  header.writeUInt16LE(32, 34);
  header.write("data", 36, "latin1");
  header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]);
}

// Sounds must share rate and channels.
function join_(sounds, gapMs) {
  const [{ rate, channels }] = sounds;
  if (sounds.some((sound) => sound.rate !== rate || sound.channels !== channels)) {
    throw new Error("the recordings to join differ in sample rate or channels");
  }
  const gap = Buffer.alloc(Math.round((rate * gapMs) / 1000) * channels * 4);
  const parts = sounds.flatMap((sound, index) => (index === 0 ? [sound.data] : [gap, sound.data]));
  return { rate, channels, data: Buffer.concat(parts) };
}

function seconds(sound) {
  return sound.data.length / (sound.rate * sound.channels * 4);
}

// The mixes of one set: for every list of languages, the 1st shortest recording of each
// joined, then the 2nd shortest, and so on, while the result stays within `maxSecs`.
function plan(set, byLanguage) {
  const mixes = [];
  for (const languages of set.mixes) {
    for (let rank = 0; rank < set.perMix; rank += 1) {
      const picked = languages.map((language) => byLanguage.get(language)?.[rank]);
      if (picked.some((clip) => !clip)) break;
      const sound = join_(
        picked.map((clip) => clip.sound),
        set.gapMs,
      );
      if (seconds(sound) > set.maxSecs) break;
      mixes.push({ languages, rank, sound, said: picked.map((clip) => clip.said).join(" ") });
    }
  }
  return mixes;
}

if (import.meta.main) {
  const sets = JSON.parse(readFileSync(join(root, "evaluation-sets.json"), "utf8"));
  const names = process.argv[2] ? [process.argv[2]] : Object.keys(sets).filter((name) => sets[name].joins);
  for (const name of names) {
    const set = sets[name];
    const from = join(root, "target/evaluation", set.joins);
    const byLanguage = new Map();
    for (const line of readFileSync(join(from, "manifest.tsv"), "utf8").split("\n")) {
      const [language, file, said] = line.split("\t");
      if (!said) continue;
      const sound = readFloatWav(readFileSync(join(from, file)), file);
      byLanguage.set(language, [...(byLanguage.get(language) ?? []), { sound, said }]);
    }
    for (const clips of byLanguage.values()) clips.sort((a, b) => seconds(a.sound) - seconds(b.sound));
    const into = join(root, "target/evaluation", name);
    mkdirSync(into, { recursive: true });
    const lines = [];
    for (const mix of plan(set, byLanguage)) {
      const label = mix.languages.join("+");
      const file = `${mix.languages.join("-")}/${mix.rank}.wav`;
      mkdirSync(dirname(join(into, file)), { recursive: true });
      writeFileSync(join(into, file), writeFloatWav(mix.sound.rate, mix.sound.channels, mix.sound.data));
      lines.push([label, file, mix.said].join("\t"));
    }
    writeFileSync(join(into, "manifest.tsv"), `${lines.join("\n")}\n`);
    console.log(`${name}: ${lines.length} recordings, ${join(into, "manifest.tsv")}`);
  }
}
