import { cli } from "./cli.ts"

if (import.meta.main) {
  await cli.parse(Deno.args)
}
