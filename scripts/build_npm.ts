#!/usr/bin/env -S deno run -A
import { build, emptyDir } from "jsr:@deno/dnt@0.41.3"
import denoConfig from "../deno.json" with { type: "json" }

await emptyDir("./npm")

await build({
  entryPoints: ["./src/main.ts"],
  outDir: "./npm",
  shims: {
    deno: true,
  },
  scriptModule: false, // ESM only (needed for top-level await)
  typeCheck: false,
  test: false,
  package: {
    name: "@schpet/linear-cli",
    version: denoConfig.version,
    description: "CLI tool for linear.app",
    license: "MIT",
    repository: {
      type: "git",
      url: "git+https://github.com/schpet/linear-cli.git",
    },
    bugs: {
      url: "https://github.com/schpet/linear-cli/issues",
    },
    bin: {
      linear: "./esm/src/main.js",
    },
    keywords: ["linear", "cli", "productivity"],
  },
  async postBuild() {
    // Copy the README
    Deno.copyFileSync("README.md", "npm/README.md")

    // Create .npmrc to configure JSR registry for @jsr scope
    await Deno.writeTextFile(
      "npm/.npmrc",
      "@jsr:registry=https://npm.jsr.io\n",
    )

    // Add shebang to the binary entry point
    const mainPath = "npm/esm/src/main.js"
    const mainContent = await Deno.readTextFile(mainPath)
    await Deno.writeTextFile(mainPath, `#!/usr/bin/env node\n${mainContent}`)

    // Add JSR dependencies to package.json using npm mirror (@jsr/ scope)
    // This works with npm, pnpm, yarn, and bun
    const pkgJsonPath = "npm/package.json"
    const pkgJson = JSON.parse(await Deno.readTextFile(pkgJsonPath))
    pkgJson.dependencies = {
      ...pkgJson.dependencies,
      "@cliffy/ansi": "npm:@jsr/cliffy__ansi@^1.0.0-rc.8",
      "@cliffy/command": "npm:@jsr/cliffy__command@^1.0.0-rc.8",
      "@cliffy/prompt": "npm:@jsr/cliffy__prompt@^1.0.0-rc.8",
      "@littletof/charmd": "npm:@jsr/littletof__charmd@^0.1.2",
      "@opensrc/deno-open": "npm:@jsr/opensrc__deno-open@^1.0.0",
      "@std/assert": "npm:@jsr/std__assert@^1.0.0",
      "@std/cli": "npm:@jsr/std__cli@^1.0.0",
      "@std/dotenv": "npm:@jsr/std__dotenv@^0.225.0",
      "@std/encoding": "npm:@jsr/std__encoding@^1.0.0",
      "@std/fmt": "npm:@jsr/std__fmt@^1.0.0",
      "@std/fs": "npm:@jsr/std__fs@^1.0.0",
      "@std/path": "npm:@jsr/std__path@^1.0.0",
      "@std/toml": "npm:@jsr/std__toml@^1.0.0",
      "@graphql-typed-document-node/core": "^3.2.0",
      "graphql": "^16.8.1",
      "graphql-request": "^7.2.0",
      "open": "^10.0.0",
      "sanitize-filename": "^1.6.3",
      "unified": "^11.0.5",
      "remark-parse": "^11.0.0",
      "remark-stringify": "^11.0.0",
      "unist-util-visit": "^5.0.0",
      "valibot": "^1.2.0",
    }
    await Deno.writeTextFile(pkgJsonPath, JSON.stringify(pkgJson, null, 2))
  },
})
