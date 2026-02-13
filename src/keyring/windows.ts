import type { KeyringBackend } from "./index.ts"
import { SERVICE } from "./index.ts"

const ERROR_NOT_FOUND = 1168
const CRED_TYPE_GENERIC = 1
const CRED_PERSIST_LOCAL_MACHINE = 2
const CREDENTIAL_SIZE = 80

type FfiBuffer = Uint8Array<ArrayBuffer>

function ffiBuffer(size: number): FfiBuffer {
  return new Uint8Array(new ArrayBuffer(size))
}

function encodeWideString(s: string): FfiBuffer {
  const buf = ffiBuffer((s.length + 1) * 2)
  for (let i = 0; i < s.length; i++) {
    const code = s.charCodeAt(i)
    buf[i * 2] = code & 0xff
    buf[i * 2 + 1] = (code >> 8) & 0xff
  }
  return buf
}

function decodeWideString(
  ptr: Deno.PointerObject,
  byteLen: number,
): string {
  const view = new Deno.UnsafePointerView(ptr)
  const buf = new Uint8Array(byteLen)
  view.copyInto(buf)
  const codes: number[] = []
  for (let i = 0; i < byteLen; i += 2) {
    codes.push(buf[i] | (buf[i + 1] << 8))
  }
  return String.fromCharCode(...codes)
}

function ptrToBigInt(ptr: Deno.PointerObject | null): bigint {
  if (ptr == null) return 0n
  return Deno.UnsafePointer.value(ptr)
}

function openAdvapi32() {
  return Deno.dlopen("advapi32.dll", {
    CredReadW: {
      parameters: ["buffer", "u32", "u32", "buffer"],
      result: "i32",
    },
    CredWriteW: { parameters: ["buffer", "u32"], result: "i32" },
    CredDeleteW: { parameters: ["buffer", "u32", "u32"], result: "i32" },
    CredFree: { parameters: ["pointer"], result: "void" },
  })
}

function openKernel32() {
  return Deno.dlopen("kernel32.dll", {
    GetLastError: { parameters: [], result: "u32" },
  })
}

let advapi32: ReturnType<typeof openAdvapi32> | null = null
let kernel32: ReturnType<typeof openKernel32> | null = null

function getAdvapi32() {
  if (advapi32 != null) return advapi32
  advapi32 = openAdvapi32()
  return advapi32
}

function getKernel32() {
  if (kernel32 != null) return kernel32
  kernel32 = openKernel32()
  return kernel32
}

function getLastError(): number {
  return getKernel32().symbols.GetLastError()
}

function credGet(account: string): string | null {
  const target = encodeWideString(`${SERVICE}:${account}`)
  const outBuf = ffiBuffer(8)
  const lib = getAdvapi32()

  const ok = lib.symbols.CredReadW(target, CRED_TYPE_GENERIC, 0, outBuf)
  if (!ok) {
    const err = getLastError()
    if (err === ERROR_NOT_FOUND) return null
    throw new Error(`CredReadW failed (error ${err})`)
  }

  const ptrValue = new DataView(outBuf.buffer).getBigUint64(0, true)
  const credPtr = Deno.UnsafePointer.create(ptrValue)
  if (credPtr == null) {
    throw new Error("CredReadW returned null credential pointer")
  }
  try {
    const view = new Deno.UnsafePointerView(credPtr)
    const blobSize = view.getUint32(32)
    if (blobSize === 0) return null
    const blobPtr = view.getPointer(40)
    if (blobPtr == null) return null
    return decodeWideString(blobPtr, blobSize)
  } finally {
    lib.symbols.CredFree(credPtr)
  }
}

function credSet(account: string, password: string): void {
  const targetBuf = encodeWideString(`${SERVICE}:${account}`)
  const userBuf = encodeWideString(account)
  const blobBuf = encodeWideString(password)
  const blobSize = password.length * 2

  const struct = ffiBuffer(CREDENTIAL_SIZE)
  const dv = new DataView(struct.buffer)

  dv.setUint32(4, CRED_TYPE_GENERIC, true)
  dv.setBigUint64(8, ptrToBigInt(Deno.UnsafePointer.of(targetBuf)), true)
  dv.setUint32(32, blobSize, true)
  dv.setBigUint64(40, ptrToBigInt(Deno.UnsafePointer.of(blobBuf)), true)
  dv.setUint32(48, CRED_PERSIST_LOCAL_MACHINE, true)
  dv.setBigUint64(72, ptrToBigInt(Deno.UnsafePointer.of(userBuf)), true)

  const lib = getAdvapi32()
  const ok = lib.symbols.CredWriteW(struct, 0)
  if (!ok) {
    const err = getLastError()
    throw new Error(`CredWriteW failed (error ${err})`)
  }
}

function credDelete(account: string): void {
  const target = encodeWideString(`${SERVICE}:${account}`)
  const lib = getAdvapi32()

  const ok = lib.symbols.CredDeleteW(target, CRED_TYPE_GENERIC, 0)
  if (!ok) {
    const err = getLastError()
    if (err === ERROR_NOT_FOUND) return
    throw new Error(`CredDeleteW failed (error ${err})`)
  }
}

export const windowsBackend: KeyringBackend = {
  get: (account) => Promise.resolve(credGet(account)),
  set: (account, password) => Promise.resolve(credSet(account, password)),
  delete: (account) => Promise.resolve(credDelete(account)),
}
