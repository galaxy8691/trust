export async function main(): number {
  let s: string = readFileTextAsync("./README.md");
  return s.length;
}
