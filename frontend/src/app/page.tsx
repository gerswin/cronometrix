import { redirect } from "next/navigation"

export default function Home() {
  // Middleware handles setup redirect; authenticated users land here
  // and get sent to the dashboard. For now redirect to /login.
  redirect("/login")
}
