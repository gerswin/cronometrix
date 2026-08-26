'use strict';

function invalidInput(name) {
  return new Error(`${name} is missing or invalid`);
}

function buildDatabaseUrl(adminUrl, { username, password, database } = {}) {
  if (
    typeof username !== 'string' || username === ''
    || typeof password !== 'string' || password === ''
    || typeof database !== 'string' || !/^[a-z][a-z0-9_]*$/.test(database)
  ) {
    throw invalidInput('database URL options');
  }

  let url;
  try {
    url = new URL(adminUrl);
  } catch {
    throw invalidInput('CRONOMETRIX_AIVEN_ADMIN_URL');
  }
  if (!['postgres:', 'postgresql:'].includes(url.protocol) || !url.hostname) {
    throw invalidInput('CRONOMETRIX_AIVEN_ADMIN_URL');
  }

  url.username = username;
  url.password = password;
  url.pathname = `/${database}`;
  return url.toString();
}

function retargetDatabaseUrl(adminUrl, database) {
  let parsed;
  try {
    parsed = new URL(adminUrl);
  } catch {
    throw invalidInput('CRONOMETRIX_AIVEN_ADMIN_URL');
  }
  return buildDatabaseUrl(adminUrl, {
    username: decodeURIComponent(parsed.username),
    password: decodeURIComponent(parsed.password),
    database,
  });
}

module.exports = { buildDatabaseUrl, retargetDatabaseUrl };
