import type { Metadata } from "next"
import { Roboto, Roboto_Mono, Roboto_Serif } from "next/font/google"
import "./globals.css"
import { Providers } from "@/components/providers"

const roboto = Roboto({
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
  variable: "--font-sans",
})

const robotoMono = Roboto_Mono({
  subsets: ["latin"],
  weight: ["400", "500", "700"],
  variable: "--font-mono",
})

const robotoSerif = Roboto_Serif({
  subsets: ["latin"],
  weight: ["400", "700"],
  style: ["italic"],
  variable: "--font-serif",
})

export const metadata: Metadata = {
  title: "Cronometrix",
  description: "Biometric time & attendance management for businesses using Hikvision facial recognition devices.",
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html lang="en" className={`${roboto.variable} ${robotoMono.variable} ${robotoSerif.variable} h-full antialiased`}>
      <body className="min-h-full flex flex-col">
        <Providers>{children}</Providers>
      </body>
    </html>
  )
}
