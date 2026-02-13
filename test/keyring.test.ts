import { assertEquals } from "@std/assert"
import { fromFileUrl } from "@std/path"

const keyringUrl = new URL("../src/keyring/index.ts", import.meta.url)
const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))

async function runWithKeyring(code: string): Promise<string> {
  const command = new Deno.Command("deno", {
    args: [
      "eval",
      `--config=${denoJsonPath}`,
      code,
    ],
    stdout: "piped",
    stderr: "piped",
  })

  const { stdout, stderr } = await command.output()
  const output = new TextDecoder().decode(stdout).trim()
  const errorOutput = new TextDecoder().decode(stderr)

  if (errorOutput && !errorOutput.startsWith("Check file:")) {
    console.error("Subprocess stderr:", errorOutput)
  }

  return output
}

Deno.test("keyring - getPassword returns null when not set", async () => {
  const code = `
    import { getPassword, _setBackend } from "${keyringUrl}";
    _setBackend({
      store: new Map(),
      get(account) { return Promise.resolve(this.store.get(account) ?? null) },
      set(account, password) { this.store.set(account, password); return Promise.resolve() },
      delete(account) { this.store.delete(account); return Promise.resolve() },
    });
    const result = await getPassword("missing");
    console.log(result === null ? "null" : result);
  `
  const output = await runWithKeyring(code)
  assertEquals(output, "null")
})

Deno.test("keyring - setPassword and getPassword round-trip", async () => {
  const code = `
    import { getPassword, setPassword, _setBackend } from "${keyringUrl}";
    _setBackend({
      store: new Map(),
      get(account) { return Promise.resolve(this.store.get(account) ?? null) },
      set(account, password) { this.store.set(account, password); return Promise.resolve() },
      delete(account) { this.store.delete(account); return Promise.resolve() },
    });
    await setPassword("my-account", "secret123");
    const result = await getPassword("my-account");
    console.log(result);
  `
  const output = await runWithKeyring(code)
  assertEquals(output, "secret123")
})

Deno.test("keyring - deletePassword removes stored password", async () => {
  const code = `
    import { getPassword, setPassword, deletePassword, _setBackend } from "${keyringUrl}";
    _setBackend({
      store: new Map(),
      get(account) { return Promise.resolve(this.store.get(account) ?? null) },
      set(account, password) { this.store.set(account, password); return Promise.resolve() },
      delete(account) { this.store.delete(account); return Promise.resolve() },
    });
    await setPassword("my-account", "secret123");
    await deletePassword("my-account");
    const result = await getPassword("my-account");
    console.log(result === null ? "null" : result);
  `
  const output = await runWithKeyring(code)
  assertEquals(output, "null")
})

Deno.test("keyring - setPassword overwrites existing value", async () => {
  const code = `
    import { getPassword, setPassword, _setBackend } from "${keyringUrl}";
    _setBackend({
      store: new Map(),
      get(account) { return Promise.resolve(this.store.get(account) ?? null) },
      set(account, password) { this.store.set(account, password); return Promise.resolve() },
      delete(account) { this.store.delete(account); return Promise.resolve() },
    });
    await setPassword("my-account", "first");
    await setPassword("my-account", "second");
    const result = await getPassword("my-account");
    console.log(result);
  `
  const output = await runWithKeyring(code)
  assertEquals(output, "second")
})

Deno.test("keyring - multiple accounts are independent", async () => {
  const code = `
    import { getPassword, setPassword, _setBackend } from "${keyringUrl}";
    _setBackend({
      store: new Map(),
      get(account) { return Promise.resolve(this.store.get(account) ?? null) },
      set(account, password) { this.store.set(account, password); return Promise.resolve() },
      delete(account) { this.store.delete(account); return Promise.resolve() },
    });
    await setPassword("account-a", "password-a");
    await setPassword("account-b", "password-b");
    const a = await getPassword("account-a");
    const b = await getPassword("account-b");
    console.log(JSON.stringify({ a, b }));
  `
  const output = await runWithKeyring(code)
  const result = JSON.parse(output)
  assertEquals(result.a, "password-a")
  assertEquals(result.b, "password-b")
})

Deno.test("keyring - deletePassword on missing account is a no-op", async () => {
  const code = `
    import { deletePassword, _setBackend } from "${keyringUrl}";
    _setBackend({
      store: new Map(),
      get(account) { return Promise.resolve(this.store.get(account) ?? null) },
      set(account, password) { this.store.set(account, password); return Promise.resolve() },
      delete(account) { this.store.delete(account); return Promise.resolve() },
    });
    await deletePassword("nonexistent");
    console.log("ok");
  `
  const output = await runWithKeyring(code)
  assertEquals(output, "ok")
})
