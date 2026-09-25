import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"

const frozenRoot = new URL("./c008-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname

Deno.test("C008 main cases preserve 16 frozen Deno inputs and pin nine added probes", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    frozenNames.push(entry.name)
  }
  frozenNames.sort()
  const corpusNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("c008-")) {
      corpusNames.push(entry.name)
    }
  }
  corpusNames.sort()
  assertEquals(
    corpusNames,
    [
      ...frozenNames,
      "c008-closed-stdout.json",
      "c008-collation.json",
      "c008-date-only.json",
      "c008-pipe-no-color-env.json",
      "c008-raw-extra-field.json",
      "c008-raw-key-workspace-conflict.json",
      "c008-raw-null-name.json",
      "c008-web-browser-missing-opener.json",
      "c008-web-empty-sourced-workspace.json",
    ].sort(),
  )

  for (const name of frozenNames) {
    const original = JSON.parse(await Deno.readTextFile(join(frozenRoot, name)))
    const promoted = JSON.parse(await Deno.readTextFile(join(corpusRoot, name)))
    promoted.deviation = original.deviation
    assertEquals(promoted, original, name)
  }
  for (
    const [name, expected] of [
      [
        "c008-closed-stdout.json",
        "d4600814332c3e451883b289249d6839a04e862a537439484cc34ea0f9de9abb",
      ],
      [
        "c008-collation.json",
        "ebfaa29ec063ec2e49c4bb2c1a23cf42fd746e75383c7d7e3b62748099f1a6db",
      ],
      [
        "c008-date-only.json",
        "eb5ce9b1ecc22c2d99e4e05dd9b7302a4cd3b49c862f4a156334b01812af4cc2",
      ],
      [
        "c008-pipe-no-color-env.json",
        "1997e8f957c76a5620a0047cfa6c55dccecb142d59cd854aedd099e99763a4e0",
      ],
      [
        "c008-raw-extra-field.json",
        "330edb0bfd70324f3b5b08ddad80e784c522b491669307326332b2cea2a9173e",
      ],
      [
        "c008-raw-key-workspace-conflict.json",
        "4b03a6af53dc4b75f6fd116d7682c9b66bd314fc698edb15399b73ba939f3c7e",
      ],
      [
        "c008-raw-null-name.json",
        "1d0ba19a2ccbeb568a414bf9569d4e4c33e1b72c032e07d28b2e97877c39e3b4",
      ],
      [
        "c008-web-browser-missing-opener.json",
        "b0a77f28cc61435ea028d0fb890167e00050aff07ff1592c8f1ed78e8702a350",
      ],
      [
        "c008-web-empty-sourced-workspace.json",
        "4c0df2b5a64e9b49dc82ef971cae17e91f70c3df61ffd7821eef28fd8ec2c03f",
      ],
    ]
  ) {
    assertEquals(
      await sha256Hex(await Deno.readFile(join(corpusRoot, name))),
      expected,
      name,
    )
  }
})
