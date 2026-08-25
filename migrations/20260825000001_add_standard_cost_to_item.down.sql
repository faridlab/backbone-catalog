-- Migration: Rollback standard_cost from items table
-- Description: Remove standard_cost field

-- Drop standard_cost column
ALTER TABLE catalog.items DROP COLUMN IF EXISTS standard_cost;
