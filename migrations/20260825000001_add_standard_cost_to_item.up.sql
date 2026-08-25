-- Migration: Add standard_cost to items table
-- Description: Add nullable standard_cost field for sales margin calculation
-- This is the interim unit-cost source for sales-margin compute; W4 refines it later

-- Add standard_cost column (nullable = cost unknown, never negative, company-scoped).
-- The CHECK passes NULL through (SQL CHECK treats NULL as satisfied) — unknown cost stays
-- representable; a negative value is always data-entry garbage.
ALTER TABLE catalog.items ADD COLUMN IF NOT EXISTS standard_cost NUMERIC(18, 6) CHECK (standard_cost >= 0);

-- Add comment for documentation
COMMENT ON COLUMN catalog.items.standard_cost IS 'Standard unit cost for margin calculation (nullable; selling snapshots this at order-confirm time)';
