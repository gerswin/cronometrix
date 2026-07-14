import type { Employee } from './api'

declare const employee: Employee

// The employee wire DTO is canonical: legacy `cedula` must not be accepted.
// @ts-expect-error `cedula` belongs to report rows, not Employee.
employee.cedula
