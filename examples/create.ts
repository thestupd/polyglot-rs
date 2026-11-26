import { readFile, writeFile } from 'node:fs/promises';
import { Polyglot } from 'polyglot-rs';

async function main() {
  const [formatA, pathA, formatB, pathB, outputPath] = process.argv.slice(2);
  if (!formatA || !pathA || !formatB || !pathB || !outputPath) {
    throw new Error('Usage: node examples/create.ts <format-a> <file-a> <format-b> <file-b> <output>');
  }
  const compatibility = await Polyglot.checkCompatibility({ formats: [formatA, formatB] });
  console.log(JSON.stringify(compatibility, null, 2));
  const [a, b] = await Promise.all([readFile(pathA), readFile(pathB)]);
  const result = await Polyglot.create({ inputs: [
    { format: formatA, data: a }, { format: formatB, data: b },
  ] });
  console.log(JSON.stringify(result.report, null, 2));
  await writeFile(outputPath, result.toBuffer(), { flag: 'wx' });
}

void main().catch((error: unknown) => { console.error(error); process.exitCode = 1; });
