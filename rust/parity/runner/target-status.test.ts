// Protocol tests for the runner side of parity-status/2. A test-only fake
// helper (plain TCP client below) emits valid, missing, malformed, duplicate,
// late and mismatched frames; the production helper has no such switches
// and no case JSON can select these behaviours.
import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import {
  describeTargetExit,
  HELPER_EXIT_CODES,
  LINUX_SIGNALS,
  openTargetStatusChannel,
  outerAgrees,
  parseResultFrame,
  STATUS_FRAME_MAX,
  STATUS_PROTOCOL,
  type TargetStatusChannel,
  TargetStatusError,
} from "./target-status.ts"

const encoder = new TextEncoder()
const decoder = new TextDecoder()

function frame(payload: string): Uint8Array {
  const bytes = encoder.encode(payload)
  const out = new Uint8Array(2 + bytes.length)
  out[0] = bytes.length >> 8
  out[1] = bytes.length & 0xff
  out.set(bytes, 2)
  return out
}

async function readFrame(conn: Deno.Conn): Promise<string | null> {
  const header = new Uint8Array(2)
  let filled = 0
  while (filled < 2) {
    const read = await conn.read(header.subarray(filled))
    if (read == null) return null
    filled += read
  }
  const length = (header[0] << 8) | header[1]
  const payload = new Uint8Array(length)
  filled = 0
  while (filled < length) {
    const read = await conn.read(payload.subarray(filled))
    if (read == null) return null
    filled += read
  }
  return decoder.decode(payload)
}

/** Test-only fake helper: connects, sends HELLO, awaits ACK, then runs `after`. */
async function fakeHelper(
  channel: TargetStatusChannel,
  options: {
    hello?: string
    after: (conn: Deno.Conn, ack: string | null) => Promise<void>
    helperPid?: number
  },
): Promise<void> {
  const conn = await Deno.connect({ hostname: "127.0.0.1", port: channel.port })
  try {
    const hello = options.hello ??
      `${STATUS_PROTOCOL} HELLO ${channel.nonce} ${channel.identity} ${
        options.helperPid ?? 2
      }`
    await conn.write(frame(hello))
    const ack = await readFrame(conn).catch(() => null)
    await options.after(conn, ack)
  } finally {
    try {
      conn.close()
    } catch {
      // closed by the helper body
    }
  }
}

function resultFrame(
  channel: TargetStatusChannel,
  tail: string,
  overrides: { nonce?: string; identity?: string; pid?: number } = {},
): string {
  const closure =
    /^(exited (0|[1-9][0-9]*)|signaled [1-9][0-9]* [A-Z0-9]+)$/.test(tail)
      ? " stdout drain 0 0 none"
      : ""
  return `${STATUS_PROTOCOL} RESULT ${overrides.nonce ?? channel.nonce} ${
    overrides.identity ?? channel.identity
  } ${overrides.pid ?? 3} ${tail}${closure}`
}

function open(deadlineMs = 5000): TargetStatusChannel {
  return openTargetStatusChannel({ deadlineMs })
}

Deno.test("a valid HELLO is ACKed before any RESULT and one RESULT followed by EOF is authenticated", async () => {
  const channel = open()
  let ackSeen: string | null = null
  let ackBeforeResult = false
  await fakeHelper(channel, {
    after: async (conn, ack) => {
      ackSeen = ack
      ackBeforeResult = ack != null
      await conn.write(frame(resultFrame(channel, "exited 143")))
      conn.close()
    },
  })
  assertEquals(ackSeen, `${STATUS_PROTOCOL} ACK ${channel.nonce}`)
  assert(ackBeforeResult)
  assertEquals(await channel.hello, { helperPid: 2 })
  assertEquals(await channel.finish(1000), {
    exit: { code: 143 },
    helperPid: 2,
    targetPid: 3,
    stdoutClosure: {
      mode: "drain",
      count: 0,
      bytesRelayed: 0,
      closure: "none",
    },
  })
  assertEquals(channel.helperArgs, [
    String(channel.port),
    channel.nonce,
    channel.identity,
  ])
  assert(/^[0-9a-f]{32}$/.test(channel.nonce))
  assert(/^[0-9a-f]{32}$/.test(channel.identity))
  assert(channel.nonce !== channel.identity)
  channel.close()
})

Deno.test("signal RESULTs map through the pinned Linux table and are distinct from folded codes", async () => {
  for (
    const [tail, exit] of [
      ["signaled 15 SIGTERM", { signal: "SIGTERM", number: 15 }],
      ["signaled 13 SIGPIPE", { signal: "SIGPIPE", number: 13 }],
      ["signaled 2 SIGINT", { signal: "SIGINT", number: 2 }],
      ["signaled 9 SIGKILL", { signal: "SIGKILL", number: 9 }],
      ["exited 0", { code: 0 }],
      ["exited 255", { code: 255 }],
    ] as const
  ) {
    const channel = open()
    await fakeHelper(channel, {
      after: async (conn) => {
        await conn.write(frame(resultFrame(channel, tail)))
      },
    })
    assertEquals((await channel.finish(1000)).exit, exit, tail)
    channel.close()
  }
  assertEquals(LINUX_SIGNALS.get(15), "SIGTERM")
  assertEquals(LINUX_SIGNALS.size, 31)
  assertEquals(describeTargetExit({ code: 143 }), "code 143")
  assertEquals(
    describeTargetExit({ signal: "SIGTERM", number: 15 }),
    "signal SIGTERM",
  )
  assertEquals(describeTargetExit(null), "no authenticated target status")
  assertEquals(outerAgrees({ code: 143 }, { code: 143 }), true)
  assertEquals(
    outerAgrees({ code: 143 }, { signal: "SIGTERM", number: 15 }),
    true,
  )
  assertEquals(
    outerAgrees({ code: 137 }, { signal: "SIGTERM", number: 15 }),
    false,
  )
  assertEquals(outerAgrees({ code: 0 }, { code: 1 }), false)
  assertEquals(
    outerAgrees({ signal: "SIGKILL" }, { signal: "SIGKILL", number: 9 }),
    false,
    "an engine-observed SIGKILL never stands in for a target status",
  )
  assertEquals(HELPER_EXIT_CODES.get(124), "target execve failed")
})

Deno.test("wrong nonce, wrong identity, wrong version, malformed HELLO and a RESULT before HELLO are rejected without an ACK", async () => {
  const other = open()
  const bad: Array<[string, (c: TargetStatusChannel) => string]> = [
    [
      "HELLO nonce mismatch",
      (c) => `${STATUS_PROTOCOL} HELLO ${other.nonce} ${c.identity} 2`,
    ],
    [
      "HELLO identity mismatch",
      (c) => `${STATUS_PROTOCOL} HELLO ${c.nonce} ${other.identity} 2`,
    ],
    [
      "malformed HELLO",
      (c) => `parity-status/1 HELLO ${c.nonce} ${c.identity} 2`,
    ],
    ["malformed HELLO", (c) => `${STATUS_PROTOCOL} HELLO ${c.nonce} 2`],
    [
      "malformed HELLO",
      (c) => `${STATUS_PROTOCOL} HELLO ${c.nonce} ${c.identity} 0`,
    ],
    [
      "malformed HELLO",
      (c) => `${STATUS_PROTOCOL} HELLO ${c.nonce} ${c.identity} 2 `,
    ],
    ["malformed HELLO", (c) => resultFrame(c, "exited 0")],
  ]
  for (const [message, hello] of bad) {
    const channel = open()
    let ack: string | null = "unset"
    await fakeHelper(channel, {
      hello: hello(channel),
      after: async (_conn, seen) => {
        ack = seen
        await Promise.resolve()
      },
    })
    assertEquals(ack, null, `${message}: no ACK`)
    await assertRejects(() => channel.hello, TargetStatusError, message)
    await assertRejects(() => channel.finish(500), TargetStatusError, message)
    channel.close()
  }
  other.close()
})

Deno.test("empty, oversize, truncated and non-ASCII frames are rejected", async () => {
  const cases: Array<[string, Uint8Array]> = [
    ["empty frame", new Uint8Array([0, 0])],
    ["exceeds cap", new Uint8Array([1, 1])],
    [
      "truncated frame",
      new Uint8Array([0, 40, ...encoder.encode("parity-status/2 HELLO")]),
    ],
    [
      "non-ASCII byte",
      frame(`${STATUS_PROTOCOL} HELLO ÿ`),
    ],
  ]
  for (const [message, bytes] of cases) {
    const channel = open()
    const conn = await Deno.connect({
      hostname: "127.0.0.1",
      port: channel.port,
    })
    await conn.write(bytes)
    if (message !== "truncated frame") {
      await assertRejects(() => channel.hello, TargetStatusError, message)
    }
    conn.close()
    await assertRejects(() => channel.finish(500), TargetStatusError, message)
    channel.close()
  }
  assertEquals(STATUS_FRAME_MAX, 256)
})

Deno.test("EOF before RESULT, bytes after RESULT, a second RESULT and a malformed or mismatched RESULT are rejected", async () => {
  const other = open()
  const cases: Array<
    [string, (c: TargetStatusChannel, conn: Deno.Conn) => Promise<void>]
  > = [
    ["EOF before RESULT", async (_c, conn) => {
      conn.close()
      await Promise.resolve()
    }],
    ["bytes after RESULT", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exited 0")))
      await conn.write(new Uint8Array([0]))
    }],
    ["bytes after RESULT", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exited 0")))
      await conn.write(frame(resultFrame(c, "exited 0")))
    }],
    ["RESULT nonce mismatch", async (c, conn) => {
      await conn.write(
        frame(resultFrame(c, "exited 0", { nonce: other.nonce })),
      )
    }],
    ["RESULT identity mismatch", async (c, conn) => {
      await conn.write(
        frame(resultFrame(c, "exited 0", { identity: other.identity })),
      )
    }],
    ["names the helper as the target", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exited 0", { pid: 2 })))
    }],
    ["malformed RESULT", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exited")))
    }],
    ["malformed RESULT", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exited 1 extra")))
    }],
    ["malformed RESULT", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "signaled 15")))
    }],
    ["malformed RESULT", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exited 007")))
    }],
    ["malformed RESULT", async (c, conn) => {
      await conn.write(
        frame(`${STATUS_PROTOCOL} HELLO ${c.nonce} ${c.identity} 2`),
      )
    }],
    ["out of range", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exited 256")))
    }],
    ["not in the pinned table", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "signaled 32 SIGRTMIN")))
    }],
    ["does not match number", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "signaled 15 SIGKILL")))
    }],
    ["does not match number", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "signaled 13 SIGTERM")))
    }],
    ["target execve failed with errno 2 (ENOENT)", async (c, conn) => {
      await conn.write(frame(resultFrame(c, "exec-failed 2")))
    }],
  ]
  for (const [message, body] of cases) {
    const channel = open()
    await fakeHelper(channel, {
      after: async (conn) => {
        await body(channel, conn)
      },
    })
    await channel.hello
    await assertRejects(() => channel.finish(1000), TargetStatusError, message)
    channel.close()
  }
  other.close()
})

Deno.test("only one connection is accepted: a second connection is refused and cannot inject a RESULT", async () => {
  const channel = open()
  await fakeHelper(channel, {
    after: async (conn) => {
      await assertRejects(
        () => Deno.connect({ hostname: "127.0.0.1", port: channel.port }),
        Deno.errors.ConnectionRefused,
      )
      await conn.write(frame(resultFrame(channel, "exited 7")))
    },
  })
  assertEquals((await channel.finish(1000)).exit, { code: 7 })
  channel.close()
})

Deno.test("a RESULT after the channel deadline is late, and a missing RESULT after the outer exit grace is an error", async () => {
  const late = open(300)
  await fakeHelper(late, {
    after: async (conn) => {
      await new Promise((resolve) => setTimeout(resolve, 600))
      await conn.write(frame(resultFrame(late, "exited 0"))).catch(() => {})
    },
  })
  await assertRejects(
    () => late.finish(100),
    TargetStatusError,
    "late or missing result",
  )
  late.close()

  const silent = open()
  const helperDone = fakeHelper(silent, {
    after: async () => {
      await new Promise((resolve) => setTimeout(resolve, 1500))
    },
  })
  await silent.hello
  await assertRejects(
    () => silent.finish(200),
    TargetStatusError,
    "no RESULT within 200 ms",
  )
  silent.close()
  await helperDone

  const never = open(200)
  await assertRejects(
    () => never.result,
    TargetStatusError,
    "late or missing result (200 ms deadline)",
  )
  never.close()
})

Deno.test("a second case cannot consume the first case's record: channels are independent", async () => {
  const first = open()
  const second = open()
  assert(first.port !== second.port)
  assert(first.nonce !== second.nonce)
  await fakeHelper(second, {
    after: async (conn) => {
      await conn.write(frame(resultFrame(first, "exited 0")))
    },
  })
  await assertRejects(
    () => second.finish(500),
    TargetStatusError,
    "RESULT nonce mismatch",
  )
  first.close()
  second.close()
  await assertRejects(
    () => first.finish(100),
    TargetStatusError,
    "listener closed before the helper connected",
  )
})

Deno.test("parseResultFrame is strict on its own", () => {
  const nonce = "a".repeat(32)
  const identity = "b".repeat(32)
  assertEquals(
    parseResultFrame(
      `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 exited 143 stdout drain 0 0 none`,
      nonce,
      identity,
      2,
    ),
    {
      exit: { code: 143 },
      helperPid: 2,
      targetPid: 3,
      stdoutClosure: {
        mode: "drain",
        count: 0,
        bytesRelayed: 0,
        closure: "none",
      },
    },
  )
  for (
    const bad of [
      `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 exited 143\n`,
      `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 EXITED 143`,
      `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 signaled 15 sigterm`,
      `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 signaled 0 SIGTERM`,
      `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 03 exited 1`,
      ` ${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 exited 1`,
      "",
    ]
  ) {
    let threw = false
    try {
      parseResultFrame(bad, nonce, identity, 2)
    } catch (error) {
      threw = error instanceof TargetStatusError
    }
    assert(threw, JSON.stringify(bad))
  }
})

Deno.test("closure RESULT grammar rejects contradictions, old version, and closure on exec failure", () => {
  const nonce = "a".repeat(32)
  const identity = "b".repeat(32)
  const prefix =
    `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 exited 0 stdout `
  assertEquals(
    parseResultFrame(
      `${prefix}close-after-bytes 4 4 after-N`,
      nonce,
      identity,
      2,
    ).stdoutClosure,
    {
      mode: "close-after-bytes",
      count: 4,
      bytesRelayed: 4,
      closure: "after-N",
    },
  )
  assertEquals(
    parseResultFrame(
      `${prefix}close-after-bytes 4 2 threshold-not-reached`,
      nonce,
      identity,
      2,
    ).stdoutClosure.closure,
    "threshold-not-reached",
  )
  for (
    const tail of [
      "drain 1 0 none",
      "drain 0 1 none",
      "closed-at-start 0 0 none",
      "closed-at-start 0 1 before-start",
      "close-after-bytes 0 0 after-N",
      "close-after-bytes 4 3 after-N",
      "close-after-bytes 4 4 threshold-not-reached",
      "close-after-bytes 4 5 after-N",
      "close-after-bytes 67108865 4 after-N",
    ]
  ) {
    assertThrows(
      () => parseResultFrame(`${prefix}${tail}`, nonce, identity, 2),
      TargetStatusError,
    )
  }
  assertThrows(
    () =>
      parseResultFrame(
        `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 3 exec-failed 2 stdout drain 0 0 none`,
        nonce,
        identity,
        2,
      ),
    TargetStatusError,
    "malformed RESULT",
  )
  assertThrows(
    () =>
      parseResultFrame(
        `parity-status/1 RESULT ${nonce} ${identity} 3 exited 0 stdout drain 0 0 none`,
        nonce,
        identity,
        2,
      ),
    TargetStatusError,
    "malformed RESULT",
  )
  const maxFrame =
    `${STATUS_PROTOCOL} RESULT ${nonce} ${identity} 9999999 signaled 16 SIGSTKFLT stdout close-after-bytes 67108864 67108863 threshold-not-reached`
  assert(
    encoder.encode(maxFrame).length <= STATUS_FRAME_MAX,
    "maximum grammar frame exceeds the pinned cap",
  )
})
