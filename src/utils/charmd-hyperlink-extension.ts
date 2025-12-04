/**
 * charmd extension to add OSC-8 hyperlinks to image URLs
 *
 * When enabled, image paths in the rendered output become clickable hyperlinks
 * in terminal emulators that support OSC-8.
 */

import type { Extension, Node, Options } from "@littletof/charmd"
import { formatPathHyperlink, resolveHyperlinkFormat } from "./hyperlink.ts"

/**
 * Create a charmd extension that wraps image URLs in OSC-8 hyperlinks
 */
export function createHyperlinkExtension(hyperlinkFormat: string): Extension {
  const resolvedFormat = resolveHyperlinkFormat(hyperlinkFormat)

  return {
    generateNode(
      _generatorFn: (
        node: Node,
        parent: Node,
        options: Options,
      ) => string | undefined,
      node: Node,
      _parent: Node | undefined,
      _options: Options,
    ): string | void {
      if (node.type === "image") {
        const alt = node.alt || ""
        const url = node.url || ""

        // Create hyperlinked URL
        const hyperlinkedUrl = formatPathHyperlink(url, url, resolvedFormat)

        // Return image representation with hyperlinked path
        return `Image: ![${alt}](${hyperlinkedUrl})`
      }
    },
  }
}
