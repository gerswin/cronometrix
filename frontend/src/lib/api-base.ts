export function resolvePublicApiBase(raw: string | undefined): string {
  return raw === undefined ? 'http://localhost:3001' : raw
}

export function resolveInternalApiBase(
  internal: string | undefined,
  publicBase: string | undefined,
): string {
  if (internal) return internal
  const resolvedPublic = resolvePublicApiBase(publicBase)
  return resolvedPublic || 'http://localhost:3001'
}
