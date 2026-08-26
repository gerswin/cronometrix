'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { existsSync, readFileSync } = require('node:fs');
const { join } = require('node:path');

const project = readFileSync(join(__dirname, '..', '..', 'project.yml'), 'utf8');

test('deploys verified TLS and immutable source metadata to both functions', () => {
  assert.match(project, /DATABASE_CA_CERT_BASE64:\s*"\$\{DATABASE_CA_CERT_BASE64\}"/);
  assert.match(project, /SOURCE_SHA:\s*"\$\{SOURCE_SHA\}"/);
});

for (const functionName of ['activate', 'renew']) {
  test(`${functionName} packages the shared PostgreSQL adapter`, () => {
    const functionRoot = join(__dirname, functionName);
    const include = readFileSync(join(functionRoot, '.include'), 'utf8')
      .split(/\r?\n/)
      .filter(Boolean);
    assert.deepEqual(include, [
      'index.js',
      'package.json',
      'package-lock.json',
      'node_modules',
      '../../../lib/pg-store.js',
    ]);
    assert.equal(existsSync(join(functionRoot, 'package-lock.json')), true);
    const packageJson = JSON.parse(readFileSync(join(functionRoot, 'package.json'), 'utf8'));
    assert.match(packageJson.dependencies.pg, /^\d+\.\d+\.\d+$/);
  });
}
