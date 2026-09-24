import { assert, assertEquals } from "@std/assert"
import { createPrivateKey, createPublicKey, X509Certificate } from "node:crypto"
import { sha256Hex } from "./bytes.ts"

const certs = new URL("./certs/", import.meta.url)
const digests = {
  ca: "46ce07ecda8f2252b5730fa3ee2bda9a0bee357fec0b15af9dcc06d64b03fcf1",
  leaf: "8040ccc0e43544a9e080052c7ee41358c8d500ecc5d54bb31281091d1716df30",
  key: "1175fa4b34466b18b0487996eeb92afd756ad1918e4bc0e533607ce52ca8680c",
}

Deno.test("fixed-host test certificates have pinned bytes, exact SANs, valid chain and matching key", async () => {
  const caBytes = await Deno.readFile(new URL("test-ca.pem", certs))
  const leafBytes = await Deno.readFile(new URL("leaf.pem", certs))
  const keyBytes = await Deno.readFile(new URL("leaf.key", certs))
  assertEquals(await sha256Hex(caBytes), digests.ca)
  assertEquals(await sha256Hex(leafBytes), digests.leaf)
  assertEquals(await sha256Hex(keyBytes), digests.key)
  const decoder = new TextDecoder("utf-8", { fatal: true })
  const ca = new X509Certificate(decoder.decode(caBytes))
  const leaf = new X509Certificate(decoder.decode(leafBytes))
  assertEquals(
    leaf.subjectAltName,
    "DNS:uploads.linear.app, DNS:public.linear.app",
  )
  assert(leaf.checkIssued(ca))
  assert(leaf.verify(ca.publicKey))
  const now = Date.now()
  assert(Date.parse(leaf.validFrom) <= now)
  assert(Date.parse(leaf.validTo) > now + 365 * 24 * 60 * 60 * 1000)
  const key = createPrivateKey(decoder.decode(keyBytes))
  const publicFromKey = createPublicKey(key).export({
    type: "spki",
    format: "der",
  })
  const publicFromLeaf = leaf.publicKey.export({ type: "spki", format: "der" })
  assertEquals(publicFromKey, publicFromLeaf)
})
