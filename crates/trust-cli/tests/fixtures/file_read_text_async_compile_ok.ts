export async function main(): number {
  let s: string = await readFileTextAsync("./README.md");
  return s.length;
}
