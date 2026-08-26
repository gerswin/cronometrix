'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const { buildDatabaseUrl } = require('./aiven-config');

test('derives percent-encoded runtime URL without admin credentials', () => {
  const value = buildDatabaseUrl(
    'postgres://avnadmin:admin%40secret@db.example:25362/defaultdb?sslmode=require',
    {
      username: 'cronometrix_license_runtime',
      password: 'run:/@ secret',
      database: 'cronometrix_licenses',
    },
  );
  const parsed = new URL(value);

  assert.equal(parsed.username, 'cronometrix_license_runtime');
  assert.equal(decodeURIComponent(parsed.password), 'run:/@ secret');
  assert.equal(parsed.pathname, '/cronometrix_licenses');
  assert.equal(value.includes('admin%40secret'), false);
  assert.equal(parsed.searchParams.get('sslmode'), 'require');
});

test('rejects incomplete URL derivation inputs without leaking the admin URL', () => {
  const adminUrl = 'postgres://avnadmin:do-not-leak@db.example/defaultdb';
  assert.throws(() => buildDatabaseUrl(adminUrl, {}), (error) => {
    assert.equal(error.message.includes('do-not-leak'), false);
    return true;
  });
});
