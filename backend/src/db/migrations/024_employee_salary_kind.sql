-- H-08: `base_salary_cents` no decía en qué unidad estaba. money.rs lo trataba
-- como el pago de una jornada ordinaria (salario diario) mientras la interfaz
-- solo mostraba "Sueldo Base (USD)". Un salario mensual ahí multiplicaba el
-- período por ~30.
--
-- Sin DEFAULT a propósito: un valor por defecto es exactamente cómo vuelve la
-- ambigüedad. SQLite exige un DEFAULT para añadir una columna NOT NULL a una
-- tabla con filas, así que se añade nullable y la capa de servicio la exige;
-- no hay datos productivos, de modo que en la práctica ninguna fila queda sin
-- unidad.
ALTER TABLE employees ADD COLUMN salary_kind TEXT
    CHECK (salary_kind IN ('hourly', 'daily', 'monthly'));
