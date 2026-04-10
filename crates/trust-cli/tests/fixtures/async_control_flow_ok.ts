export async function other(): string {
  return "x";
}

export async function main(): void {
  let flag: number = 1;
  if (flag === 1) {
    await fetchText("https://example.com/a");
  }
  let i: number = 0;
  while (i < 2) {
    await fetchText("https://example.com/b");
    i = i + 1;
  }
  let _: string = await other();
}
