'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');

const project = readFileSync(join(__dirname, '..', '..', 'project.yml'), 'utf8');

test('deploys verified TLS and immutable source metadata to both functions', () => {
  assert.match(project, /DATABASE_CA_CERT_BASE64:\s*"\$\{DATABASE_CA_CERT_BASE64\}"/);
  assert.match(project, /SOURCE_SHA:\s*"\$\{SOURCE_SHA\}"/);
});
