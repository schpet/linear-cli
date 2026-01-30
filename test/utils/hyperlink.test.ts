import { assertEquals } from "@std/assert"
import {
  shouldEnableHyperlinks,
  shouldShowSpinner,
} from "../../src/utils/hyperlink.ts"

// Tests for shouldShowSpinner

Deno.test({
  name: "shouldShowSpinner - returns false when NO_COLOR is set",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal
    Deno.stdout.isTerminal = () => true

    // Set NO_COLOR
    const originalNoColor = Deno.env.get("NO_COLOR")
    Deno.env.set("NO_COLOR", "1")

    try {
      assertEquals(shouldShowSpinner(), false)
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal
      if (originalNoColor != null) {
        Deno.env.set("NO_COLOR", originalNoColor)
      } else {
        Deno.env.delete("NO_COLOR")
      }
    }
  },
})

Deno.test({
  name: "shouldShowSpinner - returns false when stdout is not a terminal",
  fn() {
    // Mock stdout.isTerminal to return false
    const originalIsTerminal = Deno.stdout.isTerminal
    Deno.stdout.isTerminal = () => false

    // Ensure NO_COLOR is not set
    const originalNoColor = Deno.env.get("NO_COLOR")
    if (originalNoColor != null) {
      Deno.env.delete("NO_COLOR")
    }

    try {
      assertEquals(shouldShowSpinner(), false)
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal
      if (originalNoColor != null) {
        Deno.env.set("NO_COLOR", originalNoColor)
      }
    }
  },
})

Deno.test({
  name: "shouldShowSpinner - returns true when terminal and NO_COLOR not set",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal
    Deno.stdout.isTerminal = () => true

    // Ensure NO_COLOR is not set
    const originalNoColor = Deno.env.get("NO_COLOR")
    if (originalNoColor != null) {
      Deno.env.delete("NO_COLOR")
    }

    try {
      assertEquals(shouldShowSpinner(), true)
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal
      if (originalNoColor != null) {
        Deno.env.set("NO_COLOR", originalNoColor)
      }
    }
  },
})

Deno.test({
  name: "shouldShowSpinner - returns false when NO_COLOR is empty string",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal
    Deno.stdout.isTerminal = () => true

    // Set NO_COLOR to empty string (still counts as set)
    const originalNoColor = Deno.env.get("NO_COLOR")
    Deno.env.set("NO_COLOR", "")

    try {
      assertEquals(shouldShowSpinner(), false)
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal
      if (originalNoColor != null) {
        Deno.env.set("NO_COLOR", originalNoColor)
      } else {
        Deno.env.delete("NO_COLOR")
      }
    }
  },
})

// Tests for shouldEnableHyperlinks (ensure it has same behavior)

Deno.test({
  name: "shouldEnableHyperlinks - returns false when NO_COLOR is set",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal
    Deno.stdout.isTerminal = () => true

    // Set NO_COLOR
    const originalNoColor = Deno.env.get("NO_COLOR")
    Deno.env.set("NO_COLOR", "1")

    try {
      assertEquals(shouldEnableHyperlinks(), false)
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal
      if (originalNoColor != null) {
        Deno.env.set("NO_COLOR", originalNoColor)
      } else {
        Deno.env.delete("NO_COLOR")
      }
    }
  },
})

Deno.test({
  name: "shouldEnableHyperlinks - returns false when stdout is not a terminal",
  fn() {
    // Mock stdout.isTerminal to return false
    const originalIsTerminal = Deno.stdout.isTerminal
    Deno.stdout.isTerminal = () => false

    // Ensure NO_COLOR is not set
    const originalNoColor = Deno.env.get("NO_COLOR")
    if (originalNoColor != null) {
      Deno.env.delete("NO_COLOR")
    }

    try {
      assertEquals(shouldEnableHyperlinks(), false)
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal
      if (originalNoColor != null) {
        Deno.env.set("NO_COLOR", originalNoColor)
      }
    }
  },
})

Deno.test({
  name:
    "shouldEnableHyperlinks - returns true when terminal and NO_COLOR not set",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal
    Deno.stdout.isTerminal = () => true

    // Ensure NO_COLOR is not set
    const originalNoColor = Deno.env.get("NO_COLOR")
    if (originalNoColor != null) {
      Deno.env.delete("NO_COLOR")
    }

    try {
      assertEquals(shouldEnableHyperlinks(), true)
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal
      if (originalNoColor != null) {
        Deno.env.set("NO_COLOR", originalNoColor)
      }
    }
  },
})
