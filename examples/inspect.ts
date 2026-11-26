import { readFile } from 'node:fs/promises';
import { Polyglot } from 'polyglot-rs';

async function main() {
  const path = process.argv[2];
  if (!path) throw new Error('Usage: node examples/inspect.ts <file>');
  const result = await Polyglot.inspect(await readFile(path));
  console.log(JSON.stringify(result, null, 2));
}
void main().catch((error: unknown) => { console.error(error); process.exitCode = 1; });
