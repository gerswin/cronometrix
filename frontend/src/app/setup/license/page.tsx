"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { Loader2, AlertCircle, ShieldCheck, Key } from "lucide-react"
import axios from "axios"

import { licenseSchema, type LicenseFormData } from "@/lib/validations"
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

// UI-SPEC §Form Validation Contract — backend error code → banner copy.
const ERROR_MESSAGES: Record<string, string> = {
  LICENSE_NOT_FOUND:
    "License key not found. Check the key and try again.",
  HARDWARE_MISMATCH:
    "This license is registered to different hardware. Contact support to transfer your license.",
  ALREADY_ACTIVATED:
    "This license is already active on another installation.",
  ACTIVATION_UNREACHABLE:
    "Could not reach the activation server. Check your internet connection and try again.",
  VALIDATION_ERROR:
    "License key must be in XXXX-XXXX-XXXX-XXXX format.",
  // 403 from the backend may surface as either FORBIDDEN (generic RBAC) or
  // UNLICENSED (gate already wired). Treat both as a hardware-mismatch hint
  // so the operator gets actionable copy.
  FORBIDDEN:
    "This license is registered to different hardware. Contact support to transfer your license.",
}

export default function LicenseActivationPage() {
  const router = useRouter()
  const [serverError, setServerError] = useState<ServerError>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [checkingStatus, setCheckingStatus] = useState(true)
  const [success, setSuccess] = useState(false)

  const form = useForm<LicenseFormData>({
    resolver: zodResolver(licenseSchema),
    defaultValues: { license_key: "" },
  })

  // UI-SPEC §Status Check on Mount: branch on (licensed, initialized).
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const res = await fetch(`${API_BASE}/api/v1/setup/status`)
        const data = await res.json()
        if (data.licensed === true && data.initialized === false) {
          router.push("/setup")
          return
        }
        if (data.licensed === true && data.initialized === true) {
          router.push("/login")
          return
        }
        // licensed === false: show form
      } catch {
        // Backend unreachable — show form anyway (allow retry)
      } finally {
        setCheckingStatus(false)
      }
    }
    checkStatus()
  }, [router])

  async function onSubmit(values: LicenseFormData) {
    setIsSubmitting(true)
    setServerError(null)
    try {
      await axios.post(`${API_BASE}/api/v1/setup/activate`, {
        license_key: values.license_key.toUpperCase(),
      })
      setSuccess(true)
      setTimeout(() => router.push("/setup"), 1500)
    } catch (err) {
      if (axios.isAxiosError(err)) {
        const status = err.response?.status
        const code = err.response?.data?.error?.code as string | undefined
        const message =
          (code && ERROR_MESSAGES[code]) ||
          (status === 422 && err.response?.data?.error?.message) ||
          "Could not reach the activation server. Check your internet connection and try again."
        setServerError({ message })
      } else {
        setServerError({
          message:
            "Could not reach the activation server. Check your internet connection and try again.",
        })
      }
    } finally {
      setIsSubmitting(false)
    }
  }

  if (checkingStatus) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <Loader2
          className="h-6 w-6 animate-spin text-primary"
          aria-label="Checking license status"
        />
      </div>
    )
  }

  return (
    <div className="min-h-screen flex items-center justify-center px-4">
      <Card className="max-w-md w-full shadow-md">
        <CardHeader>
          <Key
            className="h-4 w-4 text-muted-foreground"
            aria-hidden="true"
          />
          <CardTitle className="text-2xl font-semibold">
            Activate your license
          </CardTitle>
          <CardDescription>
            Enter the license key provided with your Cronometrix installation.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {serverError && (
            <div
              className="flex items-center gap-3 p-4 mb-4 rounded border-l-4 border-destructive bg-destructive/10"
              role="alert"
            >
              <AlertCircle className="h-4 w-4 text-destructive shrink-0" />
              <p className="text-sm text-destructive">
                {serverError.message}
              </p>
            </div>
          )}

          {success && (
            <div
              className="flex items-center gap-3 p-4 mb-4 rounded border-l-4 border-green-600 bg-green-50 dark:bg-green-950/30"
              role="alert"
            >
              <ShieldCheck className="h-4 w-4 text-green-600 shrink-0" />
              <p className="text-sm text-green-700 dark:text-green-400">
                License activated. Continuing setup…
              </p>
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
                name="license_key"
                render={({ field, fieldState }) => (
                  <FormItem>
                    <FormLabel>License key</FormLabel>
                    <FormControl>
                      <Input
                        {...field}
                        autoComplete="off"
                        spellCheck={false}
                        maxLength={19}
                        placeholder="XXXX-XXXX-XXXX-XXXX"
                        className="font-mono uppercase"
                        aria-describedby={
                          fieldState.error ? "license_key-error" : undefined
                        }
                        aria-invalid={!!fieldState.error}
                      />
                    </FormControl>
                    <FormMessage id="license_key-error" />
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
                    Activating…
                  </>
                ) : (
                  "Activate license"
                )}
              </Button>
            </form>
          </Form>
        </CardContent>
      </Card>
    </div>
  )
}
