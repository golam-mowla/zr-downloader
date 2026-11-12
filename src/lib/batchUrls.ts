/**
 * Expand URL patterns containing numeric ranges such as:
 *   https://example.com/file[001-100].zip
 *   https://example.com/ep[1-12].mp4
 * Supports multi-line input mixed with plain URLs and comments (#).
 */
export function expandSingleUrlPattern(raw: string): string[] {
  const line = raw.trim();
  if (!line || line.startsWith("#")) return [];

  const match = line.match(/\[(\d+)-(\d+)\]/);
  if (!match || match.index === undefined) {
    return /^https?:\/\//i.test(line) || line.startsWith("magnet:?") ? [line] : [];
  }

  const startStr = match[1];
  const endStr = match[2];
  const startNum = parseInt(startStr, 10);
  const endNum = parseInt(endStr, 10);
  if (!Number.isFinite(startNum) || !Number.isFinite(endNum)) {
    return [line];
  }

  const step = startNum <= endNum ? 1 : -1;
  const count = Math.abs(endNum - startNum) + 1;
  // Cap single expansion at 500 items to keep preview responsive
  const safeCount = Math.min(count, 500);
  const padWidth = startStr.length > 1 && startStr.startsWith("0") ? startStr.length : 0;

  const prefix = line.slice(0, match.index);
  const suffix = line.slice(match.index + match[0].length);

  // Recursively expand any additional [a-b] range in the suffix
  const suffixVariants = /\[(\d+)-(\d+)\]/.test(suffix)
    ? expandSingleUrlPattern(suffix.startsWith("http") ? suffix : `https://dummy.invalid/${suffix}`).map(
        (s) => s.replace(/^https:\/\/dummy\.invalid\//, ""),
      )
    : [suffix];

  const results: string[] = [];
  for (let i = 0; i < safeCount; i++) {
    const val = startNum + i * step;
    const formatted = padWidth > 0 ? String(Math.abs(val)).padStart(padWidth, "0") : String(val);
    for (const suf of suffixVariants) {
      results.push(`${prefix}${formatted}${suf}`);
    }
  }
  return results;
}

export function expandBatchUrlInput(text: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const rawLine of text.split(/\r?\n/)) {
    for (const expanded of expandSingleUrlPattern(rawLine)) {
      if (!seen.has(expanded)) {
        seen.add(expanded);
        out.push(expanded);
      }
    }
  }
  return out;
}

export function extractUrlsFromText(text: string): string[] {
  const urls = new Set<string>();
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const matches = trimmed.match(/(https?:\/\/[^\s"'<>]+|magnet:\?xt=urn:[^\s"'<>]+)/gi);
    if (matches) {
      for (const m of matches) urls.add(m);
    }
  }
  return Array.from(urls);
}
