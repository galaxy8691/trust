export async function main(): Promise<number> {
  let s: string = readFileTextAsync("./README.md");
  return s.length;
}
