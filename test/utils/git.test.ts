import { assertEquals, assertRejects } from "@std/assert"
import { getCurrentBranch, getRepoDir } from "../../src/utils/git.ts"
import { CliError } from "../../src/utils/errors.ts"

Deno.test("getCurrentBranch - handles errors when not in a git repository", async () => {
  // Create a temporary directory that's not a git repo
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()

  try {
    Deno.chdir(tempDir)
    await assertRejects(
      async () => await getCurrentBranch(),
      CliError,
      "Failed to get current branch",
    )
  } finally {
    Deno.chdir(originalCwd)
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("getRepoDir - handles errors when not in a git repository", async () => {
  // Create a temporary directory that's not a git repo
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()

  try {
    Deno.chdir(tempDir)
    await assertRejects(
      async () => await getRepoDir(),
      CliError,
      "Failed to get repository directory",
    )
  } finally {
    Deno.chdir(originalCwd)
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("getCurrentBranch - returns null for detached HEAD", async () => {
  // Create a temporary git repository
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()

  try {
    Deno.chdir(tempDir)

    // Initialize git repo
    await new Deno.Command("git", { args: ["init"] }).output()
    await new Deno.Command("git", {
      args: ["config", "user.email", "test@example.com"],
    }).output()
    await new Deno.Command("git", {
      args: ["config", "user.name", "Test User"],
    }).output()

    // Create a commit
    await Deno.writeTextFile("test.txt", "test")
    await new Deno.Command("git", { args: ["add", "test.txt"] }).output()
    await new Deno.Command("git", {
      args: ["commit", "-m", "initial commit"],
    }).output()

    // Get the commit hash
    const { stdout } = await new Deno.Command("git", {
      args: ["rev-parse", "HEAD"],
    }).output()
    const commitHash = new TextDecoder().decode(stdout).trim()

    // Checkout the commit to create detached HEAD
    await new Deno.Command("git", {
      args: ["checkout", commitHash],
    }).output()

    // getCurrentBranch should return null for detached HEAD
    const branch = await getCurrentBranch()
    assertEquals(branch, null)
  } finally {
    Deno.chdir(originalCwd)
    await Deno.remove(tempDir, { recursive: true })
  }
})
