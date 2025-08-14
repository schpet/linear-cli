export async function getEditor(): Promise<string | null> {
  // Try git config first
  try {
    const process = new Deno.Command("git", {
      args: ["config", "--global", "core.editor"],
    });
    const { stdout, success } = await process.output();
    if (success) {
      const editor = new TextDecoder().decode(stdout).trim();
      if (editor) return editor;
    }
  } catch {
    // Fall through to next option
  }

  // Try EDITOR environment variable
  const editor = Deno.env.get("EDITOR");
  if (editor) return editor;

  return null;
}

export async function openEditor(): Promise<string | undefined> {
  const editor = await getEditor();
  if (!editor) {
    console.error(
      "No editor found. Please set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>",
    );
    return undefined;
  }

  // Create a temporary file
  const tempFile = await Deno.makeTempFile({ suffix: ".md" });

  try {
    // Open the editor
    const process = new Deno.Command(editor, {
      args: [tempFile],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });

    const { success } = await process.output();

    if (!success) {
      console.error("Editor exited with an error");
      return undefined;
    }

    // Read the content back
    const content = await Deno.readTextFile(tempFile);
    const cleaned = content.trim();

    return cleaned.length > 0 ? cleaned : undefined;
  } catch (error) {
    console.error(
      "Failed to open editor:",
      error instanceof Error ? error.message : String(error),
    );
    return undefined;
  } finally {
    // Clean up the temporary file
    try {
      await Deno.remove(tempFile);
    } catch {
      // Ignore cleanup errors
    }
  }
}
