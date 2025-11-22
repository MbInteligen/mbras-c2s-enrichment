-- Migration: Fix Orphaned Data & Add FKs
-- Date: 2025-11-21
-- Purpose: Fix orphaned values to enable FK constraints

-- ============================================================================
-- PHASE 1: FIX PROPERTY TYPES
-- ============================================================================

-- 1.1: Insert 'unknown' property type if it doesn't exist
INSERT INTO ref.property_types (code, name, category, description, is_active)
VALUES ('unknown', 'Unknown', 'Unknown', 'Legacy unknown property type', true)
ON CONFLICT (code) DO NOTHING;

-- 1.2: Add FK constraint for property_type
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_property_type_ref') THEN
    ALTER TABLE core.real_estate_properties
      ADD CONSTRAINT fk_property_type_ref
      FOREIGN KEY (property_type) REFERENCES ref.property_types(code);
    RAISE NOTICE 'Property type FK added successfully.';
  ELSE
    RAISE NOTICE 'Property type FK already exists.';
  END IF;
END $$;

-- ============================================================================
-- PHASE 2: FIX STREET TYPES
-- ============================================================================

-- 2.1: Normalize empty street types and invalid values to NULL
UPDATE core.addresses 
SET street_type = NULL 
WHERE street_type = '' 
   OR street_type LIKE '{%}' 
   OR length(street_type) > 10;

-- 2.2: Insert missing street types
INSERT INTO ref.street_type_catalog (code, description, is_active)
VALUES 
  ('R', 'Rua', true),
  ('AL', 'Alameda', true),
  ('PC', 'Praça', true),
  ('TV', 'Travessa', true),
  ('EST', 'Estrada', true),
  ('LRG', 'Largo', true),
  ('VD', 'Viaduto', true),
  ('VLA', 'Viela', true),
  ('ROD', 'Rodovia', true),
  ('AV', 'Avenida', true),
  ('ESTR', 'Estrada', true),
  ('PSG', 'Passagem', true),
  ('CAM', 'Caminho', true),
  ('CRG', 'Córrego', true),
  ('FAZ', 'Fazenda', true),
  ('PRQ', 'Parque', true),
  ('RDV', 'Rodovia', true),
  ('VIA', 'Via', true),
  ('AREA', 'Área', true),
  ('JD', 'Jardim', true),
  ('VL', 'Vila', true)
ON CONFLICT (code) DO NOTHING;

-- 2.2b: Normalize variations to standard codes
UPDATE core.addresses SET street_type = 'VIA' WHERE street_type = 'VIA ACESSO';
UPDATE core.addresses SET street_type = 'AV' WHERE street_type = 'AVENIDA';
UPDATE core.addresses SET street_type = 'VIA' WHERE street_type = 'V PEDESTRE';
UPDATE core.addresses SET street_type = NULL WHERE street_type = 'INVALIDO';

-- 2.3: Add FK constraint for street_type
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_street_type_catalog') THEN
    ALTER TABLE core.addresses
      ADD CONSTRAINT fk_street_type_catalog
      FOREIGN KEY (street_type) REFERENCES ref.street_type_catalog(code);
    RAISE NOTICE 'Street type FK added successfully.';
  ELSE
    RAISE NOTICE 'Street type FK already exists.';
  END IF;
END $$;

-- ============================================================================
-- VERIFICATION
-- ============================================================================

SELECT 
  'Constraints verification' as check_name,
  count(*) as constraint_count,
  string_agg(constraint_name, ', ') as constraints
FROM information_schema.table_constraints
WHERE constraint_name IN ('fk_property_type_ref', 'fk_street_type_catalog');
