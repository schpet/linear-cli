import { Command } from "@cliffy/command"
import { isUsingInlineFormat, migrateToKeyring } from "../../credentials.ts"
import * as keyring from "../../keyring/index.ts"
import { CliError, handleError } from "../../utils/errors.ts"

export const migrateCommand = new Command()
  .name("migrate")
  .description("Migrate plaintext credentials to system keyring")
  .action(async () => {
    try {
      if (!isUsingInlineFormat()) {
        console.log("Credentials are already using the system keyring.")
        return
      }

      const keyringOk = await keyring.isAvailable()
      if (!keyringOk) {
        throw new CliError(
          "No system keyring found. Cannot migrate credentials.",
        )
      }

      const migrated = await migrateToKeyring()
      if (migrated.length === 0) {
        console.log("No credentials to migrate.")
      } else {
        console.log(
          `Migrated ${migrated.length} workspace(s) to system keyring:`,
        )
        for (const ws of migrated) {
          console.log(`  ${ws}`)
        }
      }
    } catch (error) {
      handleError(error, "Failed to migrate credentials")
    }
  })
