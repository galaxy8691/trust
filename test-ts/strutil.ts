// 多文件链第三站：仅 `export function`；由 math.ts import（main → math → strutil）。
export function utf16_len(s: string): number {
  return s.length;
}

export function aux_label(): string {
  return "aux";
}
