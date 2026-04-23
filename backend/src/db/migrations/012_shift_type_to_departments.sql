-- 012_shift_type_to_departments.sql
-- Extends departments with Phase 3 shift configuration fields. Default 'day'/0/480
-- keeps existing Phase 1 department rows valid without rewrite.

ALTER TABLE departments ADD COLUMN shift_type TEXT NOT NULL DEFAULT 'day'
    CHECK(shift_type IN ('day', 'night', 'mixed'));
ALTER TABLE departments ADD COLUMN is_overnight_shift INTEGER NOT NULL DEFAULT 0
    CHECK(is_overnight_shift IN (0,1));
ALTER TABLE departments ADD COLUMN ordinary_daily_minutes INTEGER NOT NULL DEFAULT 480;
