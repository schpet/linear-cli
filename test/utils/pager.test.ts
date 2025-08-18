import { assertEquals } from "@std/assert";
import { shouldUsePager } from "../../src/utils/pager.ts";

Deno.test({
  name: "shouldUsePager - returns false when usePager is false",
  fn() {
    const outputLines = Array.from({ length: 100 }, (_, i) => `Line ${i + 1}`);
    assertEquals(shouldUsePager(outputLines, false), false);
  },
});

Deno.test({
  name: "shouldUsePager - returns false when not in terminal",
  fn() {
    // Mock stdout.isTerminal to return false
    const originalIsTerminal = Deno.stdout.isTerminal;
    Deno.stdout.isTerminal = () => false;

    try {
      const outputLines = Array.from(
        { length: 100 },
        (_, i) => `Line ${i + 1}`,
      );
      assertEquals(shouldUsePager(outputLines, true), false);
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal;
    }
  },
});

Deno.test({
  name:
    "shouldUsePager - returns true when content is long and conditions are met",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal;
    Deno.stdout.isTerminal = () => true;

    // Mock consoleSize to return a small terminal
    const originalConsoleSize = Deno.consoleSize;
    Deno.consoleSize = () => ({ columns: 80, rows: 10 });

    try {
      // Create output longer than terminal height (10 - 2 = 8)
      const outputLines = Array.from({ length: 20 }, (_, i) => `Line ${i + 1}`);
      assertEquals(shouldUsePager(outputLines, true), true);
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal;
      Deno.consoleSize = originalConsoleSize;
    }
  },
});

Deno.test({
  name: "shouldUsePager - returns false when content is short",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal;
    Deno.stdout.isTerminal = () => true;

    // Mock consoleSize to return a large terminal
    const originalConsoleSize = Deno.consoleSize;
    Deno.consoleSize = () => ({ columns: 80, rows: 50 });

    try {
      // Create output shorter than terminal height (50 - 2 = 48)
      const outputLines = Array.from({ length: 10 }, (_, i) => `Line ${i + 1}`);
      assertEquals(shouldUsePager(outputLines, true), false);
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal;
      Deno.consoleSize = originalConsoleSize;
    }
  },
});

Deno.test({
  name: "shouldUsePager - handles empty output",
  fn() {
    // Mock stdout.isTerminal to return true
    const originalIsTerminal = Deno.stdout.isTerminal;
    Deno.stdout.isTerminal = () => true;

    try {
      const outputLines: string[] = [];
      assertEquals(shouldUsePager(outputLines, true), false);
    } finally {
      Deno.stdout.isTerminal = originalIsTerminal;
    }
  },
});
