export async function main(): Promise<void> {
  await Promise.all([
    fetch("https://example.com/a"),
    fetch("https://example.com/b"),
  ]);
}
