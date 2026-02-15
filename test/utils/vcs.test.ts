import { assertEquals, assertRejects } from "@std/assert"
import { getCurrentIssueFromVcs, startVcsWork } from "../../src/utils/vcs.ts"
import { CliError } from "../../src/utils/errors.ts"

Deno.test("getCurrentIssueFromVcs - handles git errors gracefully", async () => {
  // Create a temporary directory that's not a git repo
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()
  const originalVcs = Deno.env.get("LINEAR_VCS")

  try {
    // Explicitly set VCS to git for this test
    Deno.env.set("LINEAR_VCS", "git")
    Deno.chdir(tempDir)
    await assertRejects(
      async () => await getCurrentIssueFromVcs(),
      CliError,
      "Failed to get current branch",
    )
  } finally {
    Deno.chdir(originalCwd)
    if (originalVcs !== undefined) {
      Deno.env.set("LINEAR_VCS", originalVcs)
    } else {
      Deno.env.delete("LINEAR_VCS")
    }
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("getCurrentIssueFromVcs - extracts issue ID from git branch", async () => {
  // Create a temporary git repository
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()
  const originalVcs = Deno.env.get("LINEAR_VCS")

  try {
    // Explicitly set VCS to git for this test
    Deno.env.set("LINEAR_VCS", "git")
    Deno.chdir(tempDir)

    // Initialize git repo
    await new Deno.Command("git", { args: ["init"] }).output()
    await new Deno.Command("git", {
      args: ["config", "user.email", "test@example.com"],
    }).output()
    await new Deno.Command("git", {
      args: ["config", "user.name", "Test User"],
    }).output()

    // Create initial commit
    await Deno.writeTextFile("test.txt", "test")
    await new Deno.Command("git", { args: ["add", "test.txt"] }).output()
    await new Deno.Command("git", {
      args: ["commit", "-m", "initial commit"],
    }).output()

    // Create a branch with an issue ID
    await new Deno.Command("git", {
      args: ["checkout", "-b", "feature/ABC-123-test-feature"],
    }).output()

    const issueId = await getCurrentIssueFromVcs()
    assertEquals(issueId, "ABC-123")
  } finally {
    Deno.chdir(originalCwd)
    if (originalVcs !== undefined) {
      Deno.env.set("LINEAR_VCS", originalVcs)
    } else {
      Deno.env.delete("LINEAR_VCS")
    }
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("getCurrentIssueFromVcs - returns null for branch without issue ID", async () => {
  // Create a temporary git repository
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()
  const originalVcs = Deno.env.get("LINEAR_VCS")

  try {
    // Explicitly set VCS to git for this test
    Deno.env.set("LINEAR_VCS", "git")
    Deno.chdir(tempDir)

    // Initialize git repo
    await new Deno.Command("git", { args: ["init"] }).output()
    await new Deno.Command("git", {
      args: ["config", "user.email", "test@example.com"],
    }).output()
    await new Deno.Command("git", {
      args: ["config", "user.name", "Test User"],
    }).output()

    // Create initial commit
    await Deno.writeTextFile("test.txt", "test")
    await new Deno.Command("git", { args: ["add", "test.txt"] }).output()
    await new Deno.Command("git", {
      args: ["commit", "-m", "initial commit"],
    }).output()

    // Create a branch without an issue ID
    await new Deno.Command("git", {
      args: ["checkout", "-b", "main"],
    }).output()

    const issueId = await getCurrentIssueFromVcs()
    assertEquals(issueId, null)
  } finally {
    Deno.chdir(originalCwd)
    if (originalVcs !== undefined) {
      Deno.env.set("LINEAR_VCS", originalVcs)
    } else {
      Deno.env.delete("LINEAR_VCS")
    }
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("startVcsWork - propagates git checkout errors when not in a git repo", async () => {
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()
  const originalVcs = Deno.env.get("LINEAR_VCS")

  try {
    Deno.env.set("LINEAR_VCS", "git")
    Deno.chdir(tempDir)

    await assertRejects(
      async () => await startVcsWork("ABC-123", "feature/ABC-123-test"),
      CliError,
      "Failed to create branch",
    )
  } finally {
    Deno.chdir(originalCwd)
    if (originalVcs !== undefined) {
      Deno.env.set("LINEAR_VCS", originalVcs)
    } else {
      Deno.env.delete("LINEAR_VCS")
    }
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("startVcsWork - propagates git checkout errors when source ref doesn't exist", async () => {
  const tempDir = await Deno.makeTempDir()
  const originalCwd = Deno.cwd()
  const originalVcs = Deno.env.get("LINEAR_VCS")

  try {
    Deno.env.set("LINEAR_VCS", "git")
    Deno.chdir(tempDir)

    // Initialize git repo
    await new Deno.Command("git", { args: ["init"] }).output()
    await new Deno.Command("git", {
      args: ["config", "user.email", "test@example.com"],
    }).output()
    await new Deno.Command("git", {
      args: ["config", "user.name", "Test User"],
    }).output()

    // Create initial commit
    await Deno.writeTextFile("test.txt", "test")
    await new Deno.Command("git", { args: ["add", "test.txt"] }).output()
    await new Deno.Command("git", {
      args: ["commit", "-m", "initial commit"],
    }).output()

    // Try to create a branch from a non-existent ref
    await assertRejects(
      async () =>
        await startVcsWork("ABC-123", "feature/ABC-123-test", "nonexistent"),
      CliError,
      "Failed to create branch",
    )
  } finally {
    Deno.chdir(originalCwd)
    if (originalVcs !== undefined) {
      Deno.env.set("LINEAR_VCS", originalVcs)
    } else {
      Deno.env.delete("LINEAR_VCS")
    }
    await Deno.remove(tempDir, { recursive: true })
  }
})
