"use client"

import { Suspense, useState } from "react"
import { useRouter, useSearchParams } from "next/navigation"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { Eye, EyeOff, Loader2, AlertCircle } from "lucide-react"
import axios from "axios"

import { loginSchema, type LoginFormData } from "@/lib/validations"
import { API_BASE, setAccessToken } from "@/lib/api"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"

type ServerError = { message: string } | null

/**
 * CR-02 mitigation: only allow same-origin relative paths as redirects.
 * Rejects protocol-relative (`//evil.com`), absolute URLs, and any value
 * that does not begin with a single `/`.
 */
function safeRedirect(raw: string | null): string {
  if (!raw) return "/"
  if (!raw.startsWith("/")) return "/"
  if (raw.startsWith("//")) return "/"
  // Defensive: collapse backslash variants that some clients normalize to `/`
  if (raw.startsWith("/\\") || raw.startsWith("\\")) return "/"
  return raw
}

function LoginPageInner() {
  const router = useRouter()
  const searchParams = useSearchParams()
  const [showPassword, setShowPassword] = useState(false)
  const [serverError, setServerError] = useState<ServerError>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  const form = useForm<LoginFormData>({
    resolver: zodResolver(loginSchema),
    defaultValues: {
      username: "",
      password: "",
    },
  })

  async function onSubmit(values: LoginFormData) {
    setIsSubmitting(true)
    setServerError(null)

    try {
      const { data } = await axios.post(
        `${API_BASE}/api/v1/auth/login`,
        { username: values.username, password: values.password },
        { withCredentials: true }
      )
      setAccessToken(data.access_token)
      // CR-02: validate redirect to prevent open-redirect via ?redirect=//evil.com
      router.push(safeRedirect(searchParams.get("redirect")))
    } catch (err) {
      if (axios.isAxiosError(err)) {
        const status = err.response?.status
        if (status === 401) {
          // T-01-19: Generic error — do not reveal which field is wrong
          setServerError({ message: "Invalid username or password." })
        } else {
          setServerError({ message: "Something went wrong. Please try again." })
        }
      } else {
        setServerError({ message: "Something went wrong. Please try again." })
      }
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center px-4">
      <Card className="max-w-md w-full shadow-md">
        <CardHeader>
          <CardTitle className="text-2xl font-semibold">
            Log in to Cronometrix
          </CardTitle>
        </CardHeader>
        <CardContent>
          {serverError && (
            <div
              className="flex items-center gap-3 p-4 mb-4 rounded border-l-4 border-destructive bg-destructive/10"
              role="alert"
            >
              <AlertCircle className="h-4 w-4 text-destructive shrink-0" />
              <p className="text-sm text-destructive">{serverError.message}</p>
            </div>
          )}

          <Form {...form}>
            <form
              onSubmit={form.handleSubmit(onSubmit)}
              className="flex flex-col gap-4"
              noValidate
            >
              <FormField
                control={form.control}
                name="username"
                render={({ field, fieldState }) => (
                  <FormItem>
                    <FormLabel>Username</FormLabel>
                    <FormControl>
                      <Input
                        {...field}
                        autoComplete="username"
                        aria-describedby={fieldState.error ? "login-username-error" : undefined}
                        aria-invalid={!!fieldState.error}
                      />
                    </FormControl>
                    <FormMessage id="login-username-error" />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="password"
                render={({ field, fieldState }) => (
                  <FormItem>
                    <FormLabel>Password</FormLabel>
                    <FormControl>
                      <div className="relative">
                        <Input
                          {...field}
                          type={showPassword ? "text" : "password"}
                          autoComplete="current-password"
                          aria-describedby={fieldState.error ? "login-password-error" : undefined}
                          aria-invalid={!!fieldState.error}
                          className="pr-10"
                        />
                        <button
                          type="button"
                          onClick={() => setShowPassword((v) => !v)}
                          aria-label={showPassword ? "Hide password" : "Show password"}
                          className="absolute inset-y-0 right-0 flex items-center px-3 text-muted-foreground hover:text-foreground"
                          tabIndex={-1}
                        >
                          {showPassword ? (
                            <EyeOff className="h-4 w-4" />
                          ) : (
                            <Eye className="h-4 w-4" />
                          )}
                        </button>
                      </div>
                    </FormControl>
                    <FormMessage id="login-password-error" />
                  </FormItem>
                )}
              />

              <Button
                type="submit"
                className="w-full"
                aria-disabled={isSubmitting}
                onClick={isSubmitting ? (e) => e.preventDefault() : undefined}
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Logging in…
                  </>
                ) : (
                  "Log in"
                )}
              </Button>
            </form>
          </Form>
        </CardContent>
      </Card>
    </div>
  )
}

// Next.js 16 requires useSearchParams() to live under a Suspense boundary
// for static prerendering — wrap the inner page in a fallback that mirrors
// the loading skeleton used by the rest of the auth wizard pages.
export default function LoginPage() {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen flex items-center justify-center">
          <Loader2
            className="h-6 w-6 animate-spin text-primary"
            aria-label="Loading login"
          />
        </div>
      }
    >
      <LoginPageInner />
    </Suspense>
  )
}
