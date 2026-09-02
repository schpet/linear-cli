import { snapshotTest } from "@cliffy/testing"
import { assertEquals, assertRejects } from "@std/assert"
import {
  composePullRequestBody,
  pullRequestCommand,
  readPullRequestTemplate,
} from "../../../src/commands/issue/issue-pull-request.ts"
import { ValidationError } from "../../../src/utils/errors.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// The help output is the contract for the two new flags.
await snapshotTest({
  name: "Issue Pull Request Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await pullRequestCommand.parse()
  },
})

// `gh pr create` rejects `--template` next to `--body`, and consults a template
// only when interactive -- so the template has to end up inside the body we
// already send. These assert the shape of that body.
Deno.test("composePullRequestBody - appends the issue URL after the template", () => {
  assertEquals(
    composePullRequestBody("## Summary\n\n## Testing", "https://linear.app/x"),
    "## Summary\n\n## Testing\n\nhttps://linear.app/x",
  )
})

Deno.test("composePullRequestBody - collapses the template's trailing whitespace", () => {
  // Template files almost always end in a newline; without the trim the URL
  // would drift further down the body with every blank line in the file.
  assertEquals(
    composePullRequestBody("## Summary\n\n\n", "https://linear.app/x"),
    "## Summary\n\nhttps://linear.app/x",
  )
})

Deno.test("composePullRequestBody - an empty template yields the URL alone", () => {
  assertEquals(
    composePullRequestBody("   \n", "https://linear.app/x"),
    "https://linear.app/x",
  )
})

Deno.test("readPullRequestTemplate - reads a regular file verbatim", async () => {
  const dir = await Deno.makeTempDir()
  try {
    const path = `${dir}/tmpl.md`
    await Deno.writeTextFile(path, "## Summary\n")
    assertEquals(await readPullRequestTemplate(path), "## Summary\n")
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

// An explicitly requested template that cannot be used must fail loudly rather
// than quietly falling back to a URL-only body: the user would get a pull
// request silently missing the content they asked for.
Deno.test("readPullRequestTemplate - rejects a missing file", async () => {
  const dir = await Deno.makeTempDir()
  try {
    await assertRejects(
      () => readPullRequestTemplate(`${dir}/absent.md`),
      ValidationError,
      "does not exist",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("readPullRequestTemplate - rejects a directory", async () => {
  const dir = await Deno.makeTempDir()
  try {
    await assertRejects(
      () => readPullRequestTemplate(dir),
      ValidationError,
      "is a directory, not a file",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("readPullRequestTemplate - rejects an empty path", async () => {
  await assertRejects(
    () => readPullRequestTemplate("   "),
    ValidationError,
    "the path is empty",
  )
})

// Deno.readTextFile does not reject binary input; it substitutes U+FFFD and
// keeps NUL bytes, which Deno.Command later rejects with a bare TypeError that
// never names the file.
Deno.test("readPullRequestTemplate - rejects a file containing NUL bytes", async () => {
  const dir = await Deno.makeTempDir()
  try {
    const path = `${dir}/binary.md`
    await Deno.writeFile(path, new Uint8Array([0x23, 0x00, 0x41]))
    await assertRejects(
      () => readPullRequestTemplate(path),
      ValidationError,
      "is not a text file",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
