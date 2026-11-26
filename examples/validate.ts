import { readFile } from 'node:fs/promises';
import { Polyglot } from 'polyglot-rs';

async function main() {
  const [path, ...formats] = process.argv.slice(2);
  if (!path || !formats.length) throw new Error('Usage: node examples/validate.ts <file> <format> [...]');
  const results = await Polyglot.validate(await readFile(path), { formats });
  console.log(JSON.stringify(results, null, 2));
  if (results.some((result) => !result.valid)) process.exitCode = 1;
}
void main().catch((error: unknown) => { console.error(error); process.exitCode = 1; });
