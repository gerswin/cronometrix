"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { Eye, EyeOff, Loader2, AlertCircle } from "lucide-react"
import axios from "axios"

import { setupSchema, type SetupFormData } from "@/lib/validations"
import { API_BASE } from "@/lib/api"
import {
  Card,
  CardContent,
  CardDescription,
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

export default function SetupPage() {
  const router = useRouter()
  const [showPassword, setShowPassword] = useState(false)
  const [showConfirmPassword, setShowConfirmPassword] = useState(false)
  const [serverError, setServerError] = useState<ServerError>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [checkingStatus, setCheckingStatus] = useState(true)
  const [alreadyConfigured, setAlreadyConfigured] = useState(false)

  const form = useForm<SetupFormData>({
    resolver: zodResolver(setupSchema),
    defaultValues: {
      full_name: "",
      username: "",
      password: "",
      confirm_password: "",
    },
  })

  // On mount: check if setup is already done
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const res = await fetch(`${API_BASE}/api/v1/setup/status`)
        const data = await res.json()
        if (data.initialized) {
          setAlreadyConfigured(true)
          setTimeout(() => router.push("/login"), 1500)
        }
      } catch {
        // Backend unreachable — show form anyway
      } finally {
        setCheckingStatus(false)
      }
    }
    checkStatus()
  }, [router])

  async function onSubmit(values: SetupFormData) {
    setIsSubmitting(true)
    setServerError(null)

    try {
      await axios.post(`${API_BASE}/api/v1/setup/init`, {
        full_name: values.full_name,
        username: values.username,
        password: values.password,
      })
      router.push("/login")
    } catch (err) {
      if (axios.isAxiosError(err)) {
        const status = err.response?.status
        if (status === 409) {
          setAlreadyConfigured(true)
          setTimeout(() => router.push("/login"), 1500)
          return
        }
        if (status === 422) {
          const detail = err.response?.data?.error?.message
          setServerError({ message: detail || "Validation error. Please check your inputs." })
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

  if (checkingStatus) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-primary" aria-label="Checking setup status" />
      </div>
    )
  }

  if (alreadyConfigured) {
    return (
      <div className="min-h-screen flex items-center justify-center px-4">
        <p className="text-center text-muted-foreground">
          Setup is already complete. Redirecting to login…
        </p>
      </div>
    )
  }

  return (
    <div className="min-h-screen flex items-center justify-center px-4">
      <Card className="max-w-md w-full shadow-md">
        <CardHeader>
          <CardTitle className="text-2xl font-semibold">
            Create your admin account
          </CardTitle>
          <CardDescription>
            This is a one-time setup. Once created, use these credentials to log in.
          </CardDescription>
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
                name="full_name"
                render={({ field, fieldState }) => (
                  <FormItem>
                    <FormLabel>Full name</FormLabel>
                    <FormControl>
                      <Input
                        {...field}
                        autoComplete="name"
                        aria-describedby={fieldState.error ? "full_name-error" : undefined}
                        aria-invalid={!!fieldState.error}
                      />
                    </FormControl>
                    <FormMessage id="full_name-error" />
                  </FormItem>
                )}
              />

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
                        aria-describedby={fieldState.error ? "username-error" : undefined}
                        aria-invalid={!!fieldState.error}
                      />
                    </FormControl>
                    <FormMessage id="username-error" />
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
                          autoComplete="new-password"
                          aria-describedby={fieldState.error ? "password-error" : undefined}
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
                    <FormMessage id="password-error" />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="confirm_password"
                render={({ field, fieldState }) => (
                  <FormItem>
                    <FormLabel>Confirm password</FormLabel>
                    <FormControl>
                      <div className="relative">
                        <Input
                          {...field}
                          type={showConfirmPassword ? "text" : "password"}
                          autoComplete="new-password"
                          aria-describedby={fieldState.error ? "confirm_password-error" : undefined}
                          aria-invalid={!!fieldState.error}
                          className="pr-10"
                        />
                        <button
                          type="button"
                          onClick={() => setShowConfirmPassword((v) => !v)}
                          aria-label={showConfirmPassword ? "Hide password" : "Show password"}
                          className="absolute inset-y-0 right-0 flex items-center px-3 text-muted-foreground hover:text-foreground"
                          tabIndex={-1}
                        >
                          {showConfirmPassword ? (
                            <EyeOff className="h-4 w-4" />
                          ) : (
                            <Eye className="h-4 w-4" />
                          )}
                        </button>
                      </div>
                    </FormControl>
                    <FormMessage id="confirm_password-error" />
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
                    Creating account…
                  </>
                ) : (
                  "Create account"
                )}
              </Button>
            </form>
          </Form>
        </CardContent>
      </Card>
    </div>
  )
}
