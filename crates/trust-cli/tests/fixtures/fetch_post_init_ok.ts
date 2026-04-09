export async function main(): Promise<void> {
  await fetch("https://example.com/echo", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
}
