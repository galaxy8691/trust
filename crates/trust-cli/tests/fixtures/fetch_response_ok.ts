export async function main(): Promise<void> {
  console.log((await fetch("https://example.com")).status);
  console.log((await fetch("https://example.com")).ok);
  await (await fetch("https://example.com")).text();
}
