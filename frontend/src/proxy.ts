import { NextRequest, NextResponse } from 'next/server'

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
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
}
