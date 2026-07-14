"use client"

import { Suspense, useState } from "react"
import { useRouter, useSearchParams } from "next/navigation"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import {
  AlertCircle,
  Clock4,
  Eye,
  EyeOff,
  Loader2,
  Lock,
  LogIn,
  ScanFace,
  User,
  ShieldCheck,
  Timer,
} from "lucide-react"
import axios from "axios"
import { toast } from "sonner"

import { loginSchema, type LoginFormData } from "@/lib/validations"
import { loginWithCredentials } from "@/lib/api"
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form"
import { Input } from "@/components/ui/input"

type ServerError = { message: string } | null

/**
 * CR-02 mitigation: only allow same-origin relative paths as redirects.
 * Rejects protocol-relative (`//evil.com`), absolute URLs, and any value
 * that does not begin with a single `/`.
 */
function safeRedirect(raw: string | null): string {
  if (!raw) return "/dashboard"
  if (!raw.startsWith("/")) return "/dashboard"
  if (raw.startsWith("//")) return "/dashboard"
  if (raw.startsWith("/\\") || raw.startsWith("\\")) return "/dashboard"
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
      await loginWithCredentials(values.username, values.password)
      // CR-02: validate redirect to prevent open-redirect via ?redirect=//evil.com
      router.push(safeRedirect(searchParams.get("redirect")))
    } catch (err) {
      if (axios.isAxiosError(err)) {
        const status = err.response?.status
        if (status === 401) {
          // T-01-19: Generic error — do not reveal which field is wrong
          setServerError({ message: "Usuario o contraseña inválidos." })
        } else {
          setServerError({ message: "Ocurrió un error. Inténtelo de nuevo." })
        }
      } else {
        setServerError({ message: "Ocurrió un error. Inténtelo de nuevo." })
      }
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="min-h-screen flex">
      {/* ── LEFT PANEL ── */}
      {/* TODO: replace gradient with /public/login-bg.jpg when asset is available */}
      <div
        className="hidden md:flex md:w-[640px] lg:w-[640px] flex-col relative overflow-hidden"
        style={{
          background:
            "linear-gradient(to bottom, #0D1A5CDD 0%, #0D1A5C99 50%, #0D1A5CFF 100%)",
          backgroundColor: "#0D1A5C",
        }}
      >
        {/* Blueprint / circuit-board pattern overlay (CSS-only fallback) */}
        <div
          className="absolute inset-0 opacity-10"
          style={{
            backgroundImage: `
              linear-gradient(rgba(85,136,221,0.4) 1px, transparent 1px),
              linear-gradient(90deg, rgba(85,136,221,0.4) 1px, transparent 1px),
              linear-gradient(rgba(85,136,221,0.15) 1px, transparent 1px),
              linear-gradient(90deg, rgba(85,136,221,0.15) 1px, transparent 1px)
            `,
            backgroundSize: "80px 80px, 80px 80px, 20px 20px, 20px 20px",
          }}
          aria-hidden="true"
        />

        {/* Top: Logo */}
        <div className="relative z-10 flex items-center gap-[10px] p-12 pb-0">
          <Timer size={32} className="text-white shrink-0" />
          <span
            className="text-white text-[28px] leading-none"
            style={{ fontFamily: "var(--font-sans)", fontWeight: 700 }}
          >
            Cronometrix
          </span>
        </div>

        {/* Bottom: tagline + features */}
        <div className="relative z-10 mt-auto flex flex-col gap-6 p-12 pt-0">
          <h1
            className="text-white text-[32px] leading-[1.2] max-w-[544px]"
            style={{ fontFamily: "var(--font-sans)", fontWeight: 700 }}
          >
            Control total de asistencia y gestión del tiempo
          </h1>

          <p
            className="text-[14px] leading-[1.5] max-w-[544px]"
            style={{
              fontFamily: "var(--font-serif)",
              fontStyle: "italic",
              color: "#FFFFFFAA",
            }}
          >
            Plataforma integral de gestión de fuerza laboral con verificación
            biométrica, control de horarios y generación de reportes en tiempo
            real.
          </p>

          {/* Feature pills */}
          <div className="flex items-center gap-6 flex-wrap">
            <div className="flex items-center gap-2">
              <ScanFace size={16} style={{ color: "#5588DD" }} />
              <span
                className="text-[12px]"
                style={{
                  fontFamily: "var(--font-sans)",
                  fontWeight: 500,
                  color: "#FFFFFFCC",
                }}
              >
                Biometría Facial
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Clock4 size={16} style={{ color: "#5588DD" }} />
              <span
                className="text-[12px]"
                style={{
                  fontFamily: "var(--font-sans)",
                  fontWeight: 500,
                  color: "#FFFFFFCC",
                }}
              >
                Tiempo Real
              </span>
            </div>
            <div className="flex items-center gap-2">
              <ShieldCheck size={16} style={{ color: "#5588DD" }} />
              <span
                className="text-[12px]"
                style={{
                  fontFamily: "var(--font-sans)",
                  fontWeight: 500,
                  color: "#FFFFFFCC",
                }}
              >
                Seguridad Avanzada
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* ── RIGHT PANEL ── */}
      <div className="flex-1 bg-white flex items-center justify-center px-6 py-12">
        <div className="flex flex-col gap-8 w-full max-w-[400px]">

          {/* Server error banner */}
          {serverError && (
            <div
              className="flex items-center gap-3 px-4 py-3 rounded border-l-4 bg-red-50"
              style={{ borderColor: "#DC2626" }}
              role="alert"
            >
              <AlertCircle className="h-4 w-4 shrink-0" style={{ color: "#DC2626" }} />
              <p className="text-sm" style={{ color: "#DC2626" }}>
                {serverError.message}
              </p>
            </div>
          )}

          {/* Header */}
          <div className="flex flex-col gap-2">
            <h2
              className="text-[28px] leading-tight"
              style={{
                fontFamily: "var(--font-sans)",
                fontWeight: 700,
                color: "#1A1A1A",
              }}
            >
              Iniciar Sesión
            </h2>
            <p
              className="text-[14px]"
              style={{
                fontFamily: "var(--font-sans)",
                fontWeight: 400,
                color: "#666666",
              }}
            >
              Ingrese sus credenciales para acceder al sistema
            </p>
          </div>

          {/* Form */}
          <Form {...form}>
            <form
              onSubmit={form.handleSubmit(onSubmit)}
              className="flex flex-col gap-5"
              noValidate
            >
              {/* Email / username field */}
              <FormField
                control={form.control}
                name="username"
                render={({ field, fieldState }) => (
                  <FormItem className="flex flex-col gap-[6px]">
                    <FormLabel
                      className="text-[13px] leading-none"
                      style={{
                        fontFamily: "var(--font-sans)",
                        fontWeight: 500,
                        color: "#1A1A1A",
                      }}
                    >
                      Usuario
                    </FormLabel>
                    <div className="relative">
                      <span
                        className="pointer-events-none absolute inset-y-0 left-[14px] flex items-center"
                        aria-hidden="true"
                      >
                        <User size={16} style={{ color: "#666666" }} />
                      </span>
                      <FormControl>
                        <Input
                          {...field}
                          autoComplete="username"
                          placeholder="usuario"
                          aria-describedby={
                            fieldState.error ? "login-username-error" : undefined
                          }
                          aria-invalid={!!fieldState.error}
                          className="h-[44px] rounded pl-[42px] pr-[14px] border text-sm"
                          style={{ borderColor: "#EEF0F2" }}
                        />
                      </FormControl>
                    </div>
                    <FormMessage id="login-username-error" />
                  </FormItem>
                )}
              />

              {/* Password field */}
              <FormField
                control={form.control}
                name="password"
                render={({ field, fieldState }) => (
                  <FormItem className="flex flex-col gap-[6px]">
                    <FormLabel
                      className="text-[13px] leading-none"
                      style={{
                        fontFamily: "var(--font-sans)",
                        fontWeight: 500,
                        color: "#1A1A1A",
                      }}
                    >
                      Contraseña
                    </FormLabel>
                    <div className="relative">
                      <span
                        className="pointer-events-none absolute inset-y-0 left-[14px] flex items-center"
                        aria-hidden="true"
                      >
                        <Lock size={16} style={{ color: "#666666" }} />
                      </span>
                      <FormControl>
                        <Input
                          {...field}
                          type={showPassword ? "text" : "password"}
                          autoComplete="current-password"
                          placeholder="••••••••"
                          aria-describedby={
                            fieldState.error ? "login-password-error" : undefined
                          }
                          aria-invalid={!!fieldState.error}
                          className="h-[44px] rounded pl-[42px] pr-[44px] border text-sm"
                          style={{ borderColor: "#EEF0F2" }}
                        />
                      </FormControl>
                      <button
                        type="button"
                        onClick={() => setShowPassword((v) => !v)}
                        aria-label={showPassword ? "Ocultar contraseña" : "Mostrar contraseña"}
                        className="absolute inset-y-0 right-0 flex items-center px-3"
                        style={{ color: "#666666" }}
                        tabIndex={-1}
                      >
                        {showPassword ? (
                          <EyeOff size={16} />
                        ) : (
                          <Eye size={16} />
                        )}
                      </button>
                    </div>
                    <FormMessage id="login-password-error" />
                  </FormItem>
                )}
              />

              {/* Options row */}
              <div className="flex items-center justify-between">
                <label className="flex items-center gap-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    className="h-4 w-4 rounded border accent-[#1E3FB8]"
                    style={{ borderColor: "#D1D5DB" }}
                  />
                  <span
                    className="text-[13px]"
                    style={{
                      fontFamily: "var(--font-sans)",
                      fontWeight: 400,
                      color: "#1A1A1A",
                    }}
                  >
                    Recordar sesión
                  </span>
                </label>
                <button
                  type="button"
                  className="text-[13px] bg-transparent border-none p-0 cursor-pointer"
                  style={{
                    fontFamily: "var(--font-sans)",
                    fontWeight: 500,
                    color: "#1E3FB8",
                  }}
                  onClick={() => toast.info("Funcionalidad próxima")}
                >
                  ¿Olvidó su contraseña?
                </button>
              </div>

              {/* Primary submit button */}
              <button
                type="submit"
                disabled={isSubmitting}
                className="flex items-center justify-center gap-2 w-full h-[48px] rounded text-white transition-colors disabled:opacity-70 disabled:cursor-not-allowed"
                style={{
                  fontFamily: "var(--font-sans)",
                  fontWeight: 600,
                  fontSize: "15px",
                  backgroundColor: "#1E3FB8",
                }}
                onMouseEnter={(e) => {
                  if (!isSubmitting)
                    (e.currentTarget as HTMLButtonElement).style.backgroundColor =
                      "#1A37A0"
                }}
                onMouseLeave={(e) => {
                  if (!isSubmitting)
                    (e.currentTarget as HTMLButtonElement).style.backgroundColor =
                      "#1E3FB8"
                }}
              >
                {isSubmitting ? (
                  <>
                    <Loader2 size={18} className="animate-spin" />
                    Iniciando sesión…
                  </>
                ) : (
                  <>
                    <LogIn size={18} />
                    Iniciar Sesión
                  </>
                )}
              </button>

            </form>
          </Form>

          {/* Footer */}
          <p
            className="text-[11px] text-center"
            style={{
              fontFamily: "var(--font-sans)",
              fontWeight: 400,
              color: "#666666",
            }}
          >
            © 2026 Cronometrix. Todos los derechos reservados.
          </p>
        </div>
      </div>
    </div>
  )
}

// Next.js requires useSearchParams() to live under a Suspense boundary
// for static prerendering — wrap the inner page in a fallback that mirrors
// the loading skeleton used by the rest of the auth wizard pages.
export default function LoginPage() {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen flex items-center justify-center">
          <Loader2
            className="h-6 w-6 animate-spin"
            style={{ color: "#1E3FB8" }}
            aria-label="Cargando inicio de sesión"
          />
        </div>
      }
    >
      <LoginPageInner />
    </Suspense>
  )
}
