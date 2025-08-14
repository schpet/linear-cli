import { unicodeWidth } from "@std/cli";

export function padDisplay(s: string, width: number): string {
  const w = unicodeWidth(s);
  return s + " ".repeat(Math.max(0, width - w));
}

export function stripConsoleFormat(s: string): string {
  return s.replace(/%c/g, "");
}

export function padDisplayFormatted(s: string, width: number): string {
  const plain = stripConsoleFormat(s);
  const w = unicodeWidth(plain);
  return s + " ".repeat(Math.max(0, width - w));
}

export function getTimeAgo(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return "just now";
  if (diffMins < 60) return `${diffMins} minutes ago`;
  if (diffHours < 24) {
    return `about ${diffHours} hour${diffHours === 1 ? "" : "s"} ago`;
  }
  return `${diffDays} day${diffDays === 1 ? "" : "s"} ago`;
}

export function getPriorityDisplay(priority: number): string {
  if (priority === 0) {
    return "---";
  } else if (priority === 1) {
    return "⚠⚠⚠";
  } else if (priority === 2) {
    return "▄▆█";
  } else if (priority === 3) {
    return "▄▆ ";
  } else if (priority === 4) {
    return "▄  ";
  }
  return priority.toString();
}
