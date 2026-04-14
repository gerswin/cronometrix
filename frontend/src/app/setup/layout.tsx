import type { Metadata } from "next"

export const metadata: Metadata = {
  title: "Cronometrix — Setup",
}

export default function SetupLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return <>{children}</>
}
