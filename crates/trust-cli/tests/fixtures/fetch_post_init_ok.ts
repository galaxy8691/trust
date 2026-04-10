export async function main(): void {
  await fetch("https://example.com/echo", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
}
