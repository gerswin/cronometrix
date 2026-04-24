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

  // Auth guard: protected routes require a refresh_token cookie (optimistic check;
  // backend enforces real JWT verification on every request).
  const isProtected = PROTECTED_PATHS.some(p => pathname.startsWith(p))
  if (isProtected) {
    const hasSession = req.cookies.get('refresh_token')?.value
    if (!hasSession) {
      const loginUrl = new URL('/login', req.url)
      loginUrl.searchParams.set('redirect', pathname)
      return NextResponse.redirect(loginUrl)
    }
  }

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
