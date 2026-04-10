export async function main(): void {
  const r: HttpResponse = await fetch("https://example.com");
  const reader: ReadableStreamDefaultReader = r.body.getReader();
  let total: number = 0;
  while (true) {
    const chunk: StreamReadResult = await reader.read();
    if (chunk.done) {
      break;
    }
    total = total + chunk.value.length;
  }
  console.log(total);
}
