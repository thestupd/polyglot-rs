import { readFile, writeFile } from 'node:fs/promises';
import { extname } from 'node:path';
import { Polyglot } from 'polyglot-rs';

// Carry an arbitrary payload inside a PNG, GIF or JPEG image. The output opens
// as the image; the payload is stored unchanged and addressable by byte range.
//
//   node examples/embed.ts <carrier.png|gif|jpeg> <payload-file> <output>
//   node examples/embed.ts --carrier png <payload-file> <output>   (1x1 carrier)
async function main() {
  const args = process.argv.slice(2);
  let carrierFormat: string | undefined;
  if (args[0] === '--carrier') {
    carrierFormat = args[1];
    args.splice(0, 2);
  }
  const [a, b, c] = args;
  const [imagePath, payloadPath, outputPath] = carrierFormat ? [undefined, a, b] : [a, b, c];
  if (!payloadPath || !outputPath) {
    throw new Error('Usage: node examples/embed.ts [<carrier-image> | --carrier png|gif|jpeg] <payload-file> <output>');
  }

  const payload = await readFile(payloadPath);
  const inputs = imagePath
    ? [
        { format: extname(imagePath).slice(1).toLowerCase(), data: await readFile(imagePath) },
        { format: 'payload', data: payload },
      ]
    : [{ format: 'payload', data: payload }];
  const result = await Polyglot.create({ inputs, strategy: carrierFormat ?? 'auto' });

  await writeFile(outputPath, result.toBuffer(), { flag: 'wx' });
  console.log(`strategy: ${result.report.strategy}`);
  console.log(`payload byte range (length@offset): ${result.byteRange()}`);
  console.log(JSON.stringify(result.report.regions, null, 2));
}

void main().catch((error: unknown) => { console.error(error); process.exitCode = 1; });
