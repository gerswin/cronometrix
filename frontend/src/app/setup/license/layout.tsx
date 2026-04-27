import type { Metadata } from "next"

export const metadata: Metadata = {
  title: "Cronometrix — License Activation",
}

export default function LicenseLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return <>{children}</>
}
