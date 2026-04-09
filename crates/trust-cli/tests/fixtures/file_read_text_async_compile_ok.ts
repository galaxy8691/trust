export async function main(): Promise<number> {
  let s: string = await readFileTextAsync("./README.md");
  return s.length;
}
