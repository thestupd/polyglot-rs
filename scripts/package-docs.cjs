const { copyFileSync, mkdirSync } = require('node:fs');
const { join } = require('node:path');
const root = join(__dirname, '..');
for (const name of ['README.md', 'LICENSE', 'CHANGELOG.md']) {
  copyFileSync(join(root, name), join(root, 'packages/node', name));
}
const docs = join(root, 'packages/node/docs');
mkdirSync(docs, { recursive: true });
for (const name of ['roadmap.md', 'releasing.md']) {
  copyFileSync(join(root, 'docs', name), join(docs, name));
}
