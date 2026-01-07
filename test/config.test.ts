import { assertEquals } from "@std/assert"
import { getOption } from "../src/config.ts"

// Note: These tests use the cliValue parameter (highest precedence)
// to avoid interference from config files that may exist in the repo

Deno.test("getOption - download_images returns boolean for truthy strings", () => {
  const truthyValues = [
    "true",
    "TRUE",
    "True",
    "yes",
    "YES",
    "y",
    "Y",
    "on",
    "ON",
    "1",
    "t",
    "T",
  ]

  for (const value of truthyValues) {
    const result = getOption("download_images", value)
    assertEquals(result, true, `Expected "${value}" to coerce to true`)
  }
})

Deno.test("getOption - download_images returns boolean for falsy strings", () => {
  const falsyValues = [
    "false",
    "FALSE",
    "False",
    "no",
    "NO",
    "n",
    "N",
    "off",
    "OFF",
    "0",
    "f",
    "F",
  ]

  for (const value of falsyValues) {
    const result = getOption("download_images", value)
    assertEquals(result, false, `Expected "${value}" to coerce to false`)
  }
})

Deno.test("getOption - download_images returns undefined for unrecognized strings", () => {
  const result = getOption("download_images", "maybe")
  assertEquals(result, undefined)
})

Deno.test("getOption - environment variables take precedence over config file", async () => {
  // Create a temp directory with a config file
  const tempDir = await Deno.makeTempDir()
  const configValue = "from-config-file"
  const envValue = "from-env-var"

  try {
    // Write a .linear.toml with a workspace value
    await Deno.writeTextFile(
      `${tempDir}/.linear.toml`,
      `workspace = "${configValue}"\n`,
    )

    // Get absolute paths to the config module and deno.json
    const configPath = new URL("../src/config.ts", import.meta.url).pathname
    const denoJsonPath = new URL("../deno.json", import.meta.url).pathname

    // Run a subprocess that imports config and prints the workspace value
    // The subprocess runs from the temp directory so it loads our test config
    const command = new Deno.Command("deno", {
      args: [
        "eval",
        `--config=${denoJsonPath}`,
        `import { getOption } from "file://${configPath}"; console.log(getOption("workspace") ?? "undefined");`,
      ],
      cwd: tempDir,
      env: {
        LINEAR_WORKSPACE: envValue,
      },
      stdout: "piped",
      stderr: "piped",
    })

    const { stdout, stderr } = await command.output()
    const output = new TextDecoder().decode(stdout).trim()
    const errorOutput = new TextDecoder().decode(stderr)

    if (errorOutput) {
      console.error("Subprocess stderr:", errorOutput)
    }

    // The env var should win over the config file
    assertEquals(
      output,
      envValue,
      "Environment variable should take precedence over config file",
    )
  } finally {
    // Clean up temp directory
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("getOption - config file is used when no env var is set", async () => {
  // Create a temp directory with a config file
  const tempDir = await Deno.makeTempDir()
  const configValue = "from-config-file"

  try {
    // Write a .linear.toml with a workspace value
    await Deno.writeTextFile(
      `${tempDir}/.linear.toml`,
      `workspace = "${configValue}"\n`,
    )

    // Get absolute paths to the config module and deno.json
    const configPath = new URL("../src/config.ts", import.meta.url).pathname
    const denoJsonPath = new URL("../deno.json", import.meta.url).pathname

    // Run a subprocess without LINEAR_WORKSPACE env var
    const command = new Deno.Command("deno", {
      args: [
        "eval",
        `--config=${denoJsonPath}`,
        `import { getOption } from "file://${configPath}"; console.log(getOption("workspace") ?? "undefined");`,
      ],
      cwd: tempDir,
      env: {}, // No LINEAR_WORKSPACE set
      stdout: "piped",
      stderr: "piped",
    })

    const { stdout, stderr } = await command.output()
    const output = new TextDecoder().decode(stdout).trim()
    const errorOutput = new TextDecoder().decode(stderr)

    if (errorOutput) {
      console.error("Subprocess stderr:", errorOutput)
    }

    // The config file value should be used as fallback
    assertEquals(
      output,
      configValue,
      "Config file should be used when no env var is set",
    )
  } finally {
    // Clean up temp directory
    await Deno.remove(tempDir, { recursive: true })
  }
})
