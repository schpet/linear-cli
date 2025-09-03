import {
  blue,
  bold,
  cyan,
  gray,
  green,
  red,
  underline,
  yellow,
} from "@std/fmt/colors"

// Common styling patterns for consistency
export const error = (text: string) => red(bold(text))
export const success = (text: string) => green(bold(text))
export const info = (text: string) => blue(text)
export const warning = (text: string) => yellow(text)
export const muted = (text: string) => gray(text)
export const highlight = (text: string) => cyan(bold(text))
export const header = (text: string) => bold(underline(text))
