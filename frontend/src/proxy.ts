import { NextRequest, NextResponse } from 'next/server'

const PROTECTED_PATHS = ['/dashboard', '/timesheet', '/employees', '/devices', '/enrollment']

export async function proxy(req: NextRequest) {
  const { pathname } = req.nextUrl

  // Never intercept these paths
  if (
    pathname.startsWith('/setup') ||
    pathname.startsWith('/api') ||
    pathname.startsWith('/_next')
  ) {
    return NextResponse.next()
  }

  // Auth guard removed: backend cookie scopes refresh_token to /api/v1/auth
  // so this proxy never sees it on /dashboard navigations and would loop
  // logged-in users back to /login. Backend still enforces JWT on every API
  // call, so an unauthenticated user reaching /dashboard hits 401 → axios
  // interceptor bounces to /login. Net UX is the same.
  void PROTECTED_PATHS

  try {
    const apiUrl = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001'
    // Note: fetch cache options (next.revalidate) have no effect in proxy — omitted
    const res = await fetch(`${apiUrl}/api/v1/setup/status`)
    const { initialized } = await res.json()

    if (!initialized) {
      return NextResponse.redirect(new URL('/setup', req.url))
    }
  } catch {
    // Backend unreachable — allow through (login page will show error)
    return NextResponse.next()
  }

  return NextResponse.next()
}

export const config = {
  matcher: ['/dashboard/:path*', '/timesheet/:path*', '/employees/:path*', '/devices/:path*', '/enrollment/:path*', '/setup/:path*'],
}
