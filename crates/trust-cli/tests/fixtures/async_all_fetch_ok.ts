export async function main(): void {
  await async_all([
    fetch("https://example.com/a"),
    fetch("https://example.com/b"),
  ]);
}
