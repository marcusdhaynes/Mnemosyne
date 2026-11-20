import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';

const root = new URL('../dist/', import.meta.url);
const index = await readFile(new URL('index.html', root), 'utf8');
const references = [...index.matchAll(/(?:src|href)=["']([^"']+)["']/g)].map(match => match[1]);
assert.ok(index.includes('<title>Mnemosyne'), 'Expected Mnemosyne page title');
assert.ok(references.includes('./main.js'), 'Required main script is missing');
assert.ok(references.includes('./styles.css'), 'Required styles are missing');
for (const reference of references) {
  if (!reference.startsWith('./')) continue;
  const rel = reference.slice(2);
  assert.ok(!rel.includes('..'), `Unsafe relative asset path: ${reference}`);
  const file = new URL(rel, root);
  assert.ok((await stat(file)).isFile(), `Required asset not found: ${reference}`);
}
const main = await readFile(new URL('main.js', root), 'utf8');
for (const [, modulePath] of main.matchAll(/from\s+["'](\.\/[^"']+)["']/g)) {
  assert.ok((await stat(new URL(modulePath, root))).isFile(), `Required module missing: ${modulePath}`);
}
assert.ok(!references.some(ref => ref.startsWith('/')), 'Absolute-root links break project GitHub Pages paths');
console.log('GitHub Pages asset path checks passed.');
