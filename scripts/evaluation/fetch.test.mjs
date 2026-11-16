import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { root, tarFiles, transcripts } from "./fetch.mjs";

// One tar entry: a 512-byte header and the body padded to whole blocks.
function entry(name, body, type = "0") {
  const header = Buffer.alloc(512);
  header.write(name, 0);
  header.write(body.length.toString(8).padStart(11, "0"), 124);
  header.write(type, 156);
  const padded = Buffer.alloc(Math.ceil(body.length / 512) * 512);
  body.copy(padded);
  return Buffer.concat([header, padded]);
}

test("tarFiles lists whole files in order and skips folders", () => {
  const archive = Buffer.concat([
    entry("dev/", Buffer.alloc(0), "5"),
    entry("dev/a.wav", Buffer.from("first")),
    entry("dev/b.wav", Buffer.alloc(700, 7)),
    Buffer.alloc(1024),
  ]);
  const files = tarFiles(archive);
  assert.deepEqual(
    files.map(([name, bytes]) => [name, bytes.length]),
    [
      ["dev/a.wav", 5],
      ["dev/b.wav", 700],
    ],
  );
  assert.equal(files[0][1].toString(), "first");
});

test("tarFiles leaves out a file the archive was cut off in", () => {
  const whole = Buffer.concat([entry("a.wav", Buffer.from("first")), entry("b.wav", Buffer.alloc(700, 7))]);
  const cut = whole.subarray(0, whole.length - 600);
  assert.deepEqual(
    tarFiles(cut).map(([name]) => name),
    ["a.wav"],
  );
  assert.deepEqual(tarFiles(whole.subarray(0, 300)), []);
});

test("transcripts reads what was said by file name", () => {
  const said = transcripts("1\ta.wav\tFirst one.\tfirst one\n2\tb.wav\t Second \tsecond\n\nbroken line\n");
  assert.equal(said.get("a.wav"), "First one.");
  assert.equal(said.get("b.wav"), "Second");
  assert.equal(said.size, 2);
});

test("every listed evaluation set says where to fetch from and which languages", () => {
  const sets = JSON.parse(readFileSync(join(root, "evaluation-sets.json"), "utf8"));
  for (const [name, set] of Object.entries(sets)) {
    if (set.joins) {
      assert.ok(sets[set.joins] && !sets[set.joins].joins, name);
      assert.ok(set.mixes.every((mix) => mix.length > 1 && mix.every((l) => sets[set.joins].languages[l])), name);
      continue;
    }
    if (set.rows) {
      assert.match(set.rows, /^https:\/\//, name);
      assert.ok(set.clips > 0 && set.pageRows > 0 && set.spoken && set.textField && set.audioField, name);
      continue;
    }
    assert.match(set.base, /^https:\/\//, name);
    assert.ok(set.transcripts.includes("{name}") && set.audio.includes("{name}"), name);
    assert.ok(set.clips > 0 && set.audioBytes > 0, name);
    assert.ok(Object.keys(set.languages).length > 0, name);
  }
});
