import { assertEquals } from "@std/assert"
import { fromFileUrl } from "@std/path"
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
    const configUrl = new URL("../src/config.ts", import.meta.url)
    const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))

    // Run a subprocess that imports config and prints the workspace value
    // The subprocess runs from the temp directory so it loads our test config
    const command = new Deno.Command("deno", {
      args: [
        "eval",
        `--config=${denoJsonPath}`,
        `import { getOption } from "${configUrl}"; console.log(getOption("workspace") ?? "undefined");`,
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
    const configUrl = new URL("../src/config.ts", import.meta.url)
    const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))

    // Run a subprocess without LINEAR_WORKSPACE env var
    // Include essential env vars for subprocess to work correctly
    const command = new Deno.Command("deno", {
      args: [
        "eval",
        `--config=${denoJsonPath}`,
        `import { getOption } from "${configUrl}"; console.log(getOption("workspace") ?? "undefined");`,
      ],
      cwd: tempDir,
      env: {
        PATH: Deno.env.get("PATH") ?? "",
        ...(Deno.build.os === "windows"
          ? { SystemRoot: Deno.env.get("SystemRoot") ?? "" }
          : {}),
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

Deno.test("getOption - home folder config is used as fallback", async () => {
  // Create a temp directory structure simulating a home folder
  const tempHome = await Deno.makeTempDir()
  const homeConfigValue = "from-home-config"

  try {
    // Create config file in platform-appropriate location
    const isWindows = Deno.build.os === "windows"
    if (isWindows) {
      // Windows: %APPDATA%\linear\linear.toml
      await Deno.mkdir(`${tempHome}/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/linear/linear.toml`,
        `workspace = "${homeConfigValue}"\n`,
      )
    } else {
      // Unix: ~/.config/linear/linear.toml
      await Deno.mkdir(`${tempHome}/.config/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/.config/linear/linear.toml`,
        `workspace = "${homeConfigValue}"\n`,
      )
    }

    // Create a separate temp directory to run from (no project config)
    const workDir = await Deno.makeTempDir()

    try {
      const configUrl = new URL("../src/config.ts", import.meta.url)
      const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))

      // Run subprocess with appropriate env var for platform
      // Note: Must NOT include XDG_CONFIG_HOME so HOME/.config is used
      const env: Record<string, string> = isWindows
        ? { APPDATA: tempHome }
        : { HOME: tempHome, PATH: Deno.env.get("PATH") ?? "" }
      const command = new Deno.Command("deno", {
        args: [
          "eval",
          `--config=${denoJsonPath}`,
          `import { getOption } from "${configUrl}"; console.log(getOption("workspace") ?? "undefined");`,
        ],
        cwd: workDir,
        env,
        clearEnv: true,
        stdout: "piped",
        stderr: "piped",
      })

      const { stdout, stderr } = await command.output()
      const output = new TextDecoder().decode(stdout).trim()
      const errorOutput = new TextDecoder().decode(stderr)

      if (errorOutput) {
        console.error("Subprocess stderr:", errorOutput)
      }

      assertEquals(
        output,
        homeConfigValue,
        "Home folder config should be used when no project config exists",
      )
    } finally {
      await Deno.remove(workDir, { recursive: true })
    }
  } finally {
    await Deno.remove(tempHome, { recursive: true })
  }
})

Deno.test("getOption - project config takes precedence over home config", async () => {
  // Create temp directories for home and project
  const tempHome = await Deno.makeTempDir()
  const projectDir = await Deno.makeTempDir()
  const homeConfigValue = "from-home-config"
  const projectConfigValue = "from-project-config"

  try {
    // Create home config in platform-appropriate location
    const isWindows = Deno.build.os === "windows"
    if (isWindows) {
      await Deno.mkdir(`${tempHome}/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/linear/linear.toml`,
        `workspace = "${homeConfigValue}"\n`,
      )
    } else {
      // Unix: ~/.config/linear/linear.toml
      await Deno.mkdir(`${tempHome}/.config/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/.config/linear/linear.toml`,
        `workspace = "${homeConfigValue}"\n`,
      )
    }

    // Create project .linear.toml
    await Deno.writeTextFile(
      `${projectDir}/.linear.toml`,
      `workspace = "${projectConfigValue}"\n`,
    )

    const configUrl = new URL("../src/config.ts", import.meta.url)
    const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))

    const env: Record<string, string> = isWindows
      ? { APPDATA: tempHome, SystemRoot: Deno.env.get("SystemRoot") ?? "" }
      : { HOME: tempHome, PATH: Deno.env.get("PATH") ?? "" }
    const command = new Deno.Command("deno", {
      args: [
        "eval",
        `--config=${denoJsonPath}`,
        `import { getOption } from "${configUrl}"; console.log(getOption("workspace") ?? "undefined");`,
      ],
      cwd: projectDir,
      env,
      stdout: "piped",
      stderr: "piped",
    })

    const { stdout, stderr } = await command.output()
    const output = new TextDecoder().decode(stdout).trim()
    const errorOutput = new TextDecoder().decode(stderr)

    if (errorOutput) {
      console.error("Subprocess stderr:", errorOutput)
    }

    assertEquals(
      output,
      projectConfigValue,
      "Project config should take precedence over home config",
    )
  } finally {
    await Deno.remove(tempHome, { recursive: true })
    await Deno.remove(projectDir, { recursive: true })
  }
})

Deno.test({
  name: "getOption - XDG_CONFIG_HOME takes precedence over HOME/.config",
  ignore: Deno.build.os === "windows", // XDG is Unix-specific
  fn: async () => {
    // Create temp directories for XDG and regular home
    const tempHome = await Deno.makeTempDir()
    const xdgConfigDir = await Deno.makeTempDir()
    const homeConfigValue = "from-home-config"
    const xdgConfigValue = "from-xdg-config"

    try {
      // Create ~/.config/linear/linear.toml (should NOT be used)
      await Deno.mkdir(`${tempHome}/.config/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/.config/linear/linear.toml`,
        `workspace = "${homeConfigValue}"\n`,
      )

      // Create $XDG_CONFIG_HOME/linear/linear.toml (should be used)
      await Deno.mkdir(`${xdgConfigDir}/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${xdgConfigDir}/linear/linear.toml`,
        `workspace = "${xdgConfigValue}"\n`,
      )

      // Create a work directory (no project config)
      const workDir = await Deno.makeTempDir()

      try {
        const configUrl = new URL("../src/config.ts", import.meta.url)
        const denoJsonPath = fromFileUrl(
          new URL("../deno.json", import.meta.url),
        )

        const command = new Deno.Command("deno", {
          args: [
            "eval",
            `--config=${denoJsonPath}`,
            `import { getOption } from "${configUrl}"; console.log(getOption("workspace") ?? "undefined");`,
          ],
          cwd: workDir,
          env: {
            HOME: tempHome,
            XDG_CONFIG_HOME: xdgConfigDir,
            PATH: Deno.env.get("PATH") ?? "",
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

        assertEquals(
          output,
          xdgConfigValue,
          "XDG_CONFIG_HOME should take precedence over HOME/.config",
        )
      } finally {
        await Deno.remove(workDir, { recursive: true })
      }
    } finally {
      await Deno.remove(tempHome, { recursive: true })
      await Deno.remove(xdgConfigDir, { recursive: true })
    }
  },
})

Deno.test({
  name: "getOption - APPDATA config is used on Windows",
  ignore: Deno.build.os !== "windows", // Windows-specific test
  fn: async () => {
    // Create temp directory simulating APPDATA
    const tempAppData = await Deno.makeTempDir()
    const configValue = "from-appdata-config"

    try {
      // Create %APPDATA%\linear\linear.toml
      await Deno.mkdir(`${tempAppData}/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempAppData}/linear/linear.toml`,
        `workspace = "${configValue}"\n`,
      )

      // Create a work directory (no project config)
      const workDir = await Deno.makeTempDir()

      try {
        const configUrl = new URL("../src/config.ts", import.meta.url)
        const denoJsonPath = fromFileUrl(
          new URL("../deno.json", import.meta.url),
        )

        const command = new Deno.Command("deno", {
          args: [
            "eval",
            `--config=${denoJsonPath}`,
            `import { getOption } from "${configUrl}"; console.log(getOption("workspace") ?? "undefined");`,
          ],
          cwd: workDir,
          env: {
            APPDATA: tempAppData,
            SystemRoot: Deno.env.get("SystemRoot") ?? "",
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

        assertEquals(
          output,
          configValue,
          "APPDATA config should be used on Windows",
        )
      } finally {
        await Deno.remove(workDir, { recursive: true })
      }
    } finally {
      await Deno.remove(tempAppData, { recursive: true })
    }
  },
})

Deno.test("getOption - global and project configs are merged", async () => {
  // Create temp directories for home and project
  const tempHome = await Deno.makeTempDir()
  const projectDir = await Deno.makeTempDir()
  const globalIssueSort = "priority"
  const projectWorkspace = "my-workspace"

  try {
    // Create home config with issue_sort
    const isWindows = Deno.build.os === "windows"
    if (isWindows) {
      await Deno.mkdir(`${tempHome}/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/linear/linear.toml`,
        `issue_sort = "${globalIssueSort}"\n`,
      )
    } else {
      await Deno.mkdir(`${tempHome}/.config/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/.config/linear/linear.toml`,
        `issue_sort = "${globalIssueSort}"\n`,
      )
    }

    // Create project config with workspace (different key)
    await Deno.writeTextFile(
      `${projectDir}/.linear.toml`,
      `workspace = "${projectWorkspace}"\n`,
    )

    const configUrl = new URL("../src/config.ts", import.meta.url)
    const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))

    // Note: clearEnv ensures XDG_CONFIG_HOME doesn't interfere with HOME/.config
    const env: Record<string, string> = isWindows
      ? { APPDATA: tempHome, SystemRoot: Deno.env.get("SystemRoot") ?? "" }
      : { HOME: tempHome, PATH: Deno.env.get("PATH") ?? "" }

    // Test that both values are accessible
    const command = new Deno.Command("deno", {
      args: [
        "eval",
        `--config=${denoJsonPath}`,
        `import { getOption } from "${configUrl}"; console.log(JSON.stringify({ issue_sort: getOption("issue_sort"), workspace: getOption("workspace") }));`,
      ],
      cwd: projectDir,
      env,
      clearEnv: true,
      stdout: "piped",
      stderr: "piped",
    })

    const { stdout, stderr } = await command.output()
    const output = new TextDecoder().decode(stdout).trim()
    const errorOutput = new TextDecoder().decode(stderr)

    if (errorOutput) {
      console.error("Subprocess stderr:", errorOutput)
    }

    const result = JSON.parse(output)
    assertEquals(
      result.issue_sort,
      globalIssueSort,
      "Global config value (issue_sort) should be accessible",
    )
    assertEquals(
      result.workspace,
      projectWorkspace,
      "Project config value (workspace) should be accessible",
    )
  } finally {
    await Deno.remove(tempHome, { recursive: true })
    await Deno.remove(projectDir, { recursive: true })
  }
})

Deno.test("getOption - env var takes precedence over home config", async () => {
  // Create temp home directory
  const tempHome = await Deno.makeTempDir()
  const homeConfigValue = "from-home-config"
  const envValue = "from-env-var"

  try {
    // Create home config in platform-appropriate location
    const isWindows = Deno.build.os === "windows"
    if (isWindows) {
      await Deno.mkdir(`${tempHome}/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/linear/linear.toml`,
        `workspace = "${homeConfigValue}"\n`,
      )
    } else {
      // Unix: ~/.config/linear/linear.toml
      await Deno.mkdir(`${tempHome}/.config/linear`, { recursive: true })
      await Deno.writeTextFile(
        `${tempHome}/.config/linear/linear.toml`,
        `workspace = "${homeConfigValue}"\n`,
      )
    }

    // Create a work directory (no project config)
    const workDir = await Deno.makeTempDir()

    try {
      const configUrl = new URL("../src/config.ts", import.meta.url)
      const denoJsonPath = fromFileUrl(
        new URL("../deno.json", import.meta.url),
      )

      const env: Record<string, string> = isWindows
        ? {
          APPDATA: tempHome,
          LINEAR_WORKSPACE: envValue,
          SystemRoot: Deno.env.get("SystemRoot") ?? "",
        }
        : {
          HOME: tempHome,
          LINEAR_WORKSPACE: envValue,
          PATH: Deno.env.get("PATH") ?? "",
        }
      const command = new Deno.Command("deno", {
        args: [
          "eval",
          `--config=${denoJsonPath}`,
          `import { getOption } from "${configUrl}"; console.log(getOption("workspace") ?? "undefined");`,
        ],
        cwd: workDir,
        env,
        stdout: "piped",
        stderr: "piped",
      })

      const { stdout, stderr } = await command.output()
      const output = new TextDecoder().decode(stdout).trim()
      const errorOutput = new TextDecoder().decode(stderr)

      if (errorOutput) {
        console.error("Subprocess stderr:", errorOutput)
      }

      assertEquals(
        output,
        envValue,
        "Environment variable should take precedence over home config",
      )
    } finally {
      await Deno.remove(workDir, { recursive: true })
    }
  } finally {
    await Deno.remove(tempHome, { recursive: true })
  }
})
