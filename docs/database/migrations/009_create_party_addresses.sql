-- Migration 009: Create core.party_addresses table and backfill from legacy
-- Date: 2025-11-22
-- Purpose: Migrate 11,530 addresses from entity_addresses to Party Model

-- ============================================================================
-- STEP 1: Create core.party_addresses table
-- ============================================================================

CREATE TABLE IF NOT EXISTS core.party_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id UUID NOT NULL REFERENCES core.parties(id) ON DELETE CASCADE,
    address_id UUID NOT NULL REFERENCES core.addresses(id) ON DELETE CASCADE,
    address_type TEXT CHECK (address_type IN ('residential', 'commercial', 'billing', 'family_member', 'other')),
    is_primary BOOLEAN DEFAULT false,
    is_current BOOLEAN DEFAULT true,
    verified BOOLEAN DEFAULT false,
    confidence_score NUMERIC(3,2) DEFAULT 0.80 CHECK (confidence_score >= 0 AND confidence_score <= 1),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Add comments
COMMENT ON TABLE core.party_addresses IS 'Party-address relationships with confidence scoring and temporal tracking';
COMMENT ON COLUMN core.party_addresses.confidence_score IS 'Data quality score: 0-1 scale, higher is better';
COMMENT ON COLUMN core.party_addresses.is_primary IS 'Primary address for party (should be unique per party)';
COMMENT ON COLUMN core.party_addresses.is_current IS 'Currently active address';
COMMENT ON COLUMN core.party_addresses.metadata IS 'Additional metadata: source, migration info, ownership details';

-- ============================================================================
-- STEP 2: Create indexes
-- ============================================================================

-- Lookup by party
CREATE INDEX idx_party_addresses_party
ON core.party_addresses(party_id);

-- Lookup by address (reverse: which parties at this address?)
CREATE INDEX idx_party_addresses_address
ON core.party_addresses(address_id);

-- Find primary address for party (partial index for performance)
CREATE UNIQUE INDEX idx_party_addresses_primary
ON core.party_addresses(party_id)
WHERE is_primary = true;

-- Find current addresses (most common query)
CREATE INDEX idx_party_addresses_current
ON core.party_addresses(party_id, is_current)
WHERE is_current = true;

-- High-confidence addresses only
CREATE INDEX idx_party_addresses_confidence
ON core.party_addresses(confidence_score)
WHERE confidence_score >= 0.75;

-- ============================================================================
-- STEP 3: Create unique constraint (prevent duplicate party-address links)
-- ============================================================================

CREATE UNIQUE INDEX uq_party_address_link
ON core.party_addresses(party_id, address_id);

-- ============================================================================
-- STEP 4: Create update trigger
-- ============================================================================

CREATE OR REPLACE FUNCTION update_party_addresses_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_party_addresses_updated_at
    BEFORE UPDATE ON core.party_addresses
    FOR EACH ROW
    EXECUTE FUNCTION update_party_addresses_timestamp();

-- ============================================================================
-- STEP 5: Backfill from legacy entity_addresses
-- ============================================================================

-- Insert addresses from legacy schema
-- Strategy: Match entity_id → party_id via cpf_cnpj
INSERT INTO core.party_addresses (
    party_id,
    address_id,
    address_type,
    is_primary,
    is_current,
    verified,
    confidence_score,
    metadata,
    created_at
)
SELECT
    p.id as party_id,
    ea.address_id,
    COALESCE(ea.address_type, 'residential') as address_type,
    COALESCE(ea.is_primary, false) as is_primary,
    true as is_current, -- Assume legacy addresses are current
    COALESCE(ea.verified, false) as verified,
    CASE
        WHEN ea.verified THEN 0.95  -- Verified addresses high confidence
        WHEN ea.is_primary THEN 0.85 -- Primary addresses good confidence
        ELSE 0.75                    -- Other addresses decent confidence
    END as confidence_score,
    jsonb_build_object(
        'source', 'legacy_migration',
        'migration_date', CURRENT_TIMESTAMP,
        'original_entity_id', e.entity_id,
        'original_entity_uuid', ea.entity_id,
        'legacy_created_at', ea.created_at
    ) as metadata,
    COALESCE(ea.created_at, CURRENT_TIMESTAMP) as created_at
FROM core.entity_addresses ea
JOIN core.entities e ON ea.entity_id = e.entity_id
JOIN core.parties p ON p.cpf_cnpj = e.national_id
WHERE ea.address_id IS NOT NULL  -- Skip invalid addresses
ON CONFLICT (party_id, address_id) DO UPDATE SET
    -- If duplicate exists (shouldn't happen), preserve higher confidence
    confidence_score = GREATEST(core.party_addresses.confidence_score, EXCLUDED.confidence_score),
    is_primary = EXCLUDED.is_primary OR core.party_addresses.is_primary,
    verified = EXCLUDED.verified OR core.party_addresses.verified,
    metadata = core.party_addresses.metadata || EXCLUDED.metadata,
    updated_at = CURRENT_TIMESTAMP;

-- ============================================================================
-- STEP 6: Verification queries
-- ============================================================================

-- Show migration summary
DO $$
DECLARE
    legacy_count INTEGER;
    migrated_count INTEGER;
    primary_count INTEGER;
    verified_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO legacy_count FROM core.entity_addresses;
    SELECT COUNT(*) INTO migrated_count FROM core.party_addresses;
    SELECT COUNT(*) INTO primary_count FROM core.party_addresses WHERE is_primary = true;
    SELECT COUNT(*) INTO verified_count FROM core.party_addresses WHERE verified = true;

    RAISE NOTICE '============================================';
    RAISE NOTICE 'Party Addresses Migration Summary';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Legacy addresses (entity_addresses): %', legacy_count;
    RAISE NOTICE 'Migrated addresses (party_addresses): %', migrated_count;
    RAISE NOTICE 'Primary addresses: %', primary_count;
    RAISE NOTICE 'Verified addresses: %', verified_count;
    RAISE NOTICE '============================================';

    IF migrated_count < legacy_count THEN
        RAISE WARNING 'Not all addresses migrated! Missing: % addresses', (legacy_count - migrated_count);
    ELSE
        RAISE NOTICE '✅ All addresses successfully migrated!';
    END IF;
END $$;

-- Sample migrated addresses
SELECT
    p.cpf_cnpj,
    p.full_name,
    a.street,
    a.number,
    a.neighborhood,
    a.city,
    a.state,
    a.zip_code,
    pa.address_type,
    pa.is_primary,
    pa.confidence_score,
    pa.metadata->>'source' as source
FROM core.party_addresses pa
JOIN core.parties p ON pa.party_id = p.id
JOIN core.addresses a ON pa.address_id = a.id
ORDER BY pa.created_at DESC
LIMIT 10;

-- Check for parties with multiple primary addresses (data quality issue)
SELECT
    party_id,
    COUNT(*) as primary_count
FROM core.party_addresses
WHERE is_primary = true
GROUP BY party_id
HAVING COUNT(*) > 1;

-- ============================================================================
-- STEP 7: Create helper functions
-- ============================================================================

-- Function to get primary address for a party
CREATE OR REPLACE FUNCTION get_party_primary_address(p_party_id UUID)
RETURNS TABLE (
    address_id UUID,
    street TEXT,
    city TEXT,
    state TEXT,
    zip_code TEXT,
    confidence_score NUMERIC
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        a.id,
        a.street,
        a.city,
        a.state,
        a.zip_code,
        pa.confidence_score
    FROM core.party_addresses pa
    JOIN core.addresses a ON pa.address_id = a.id
    WHERE pa.party_id = p_party_id
      AND pa.is_primary = true
      AND pa.is_current = true
    LIMIT 1;
END;
$$ LANGUAGE plpgsql;

-- Function to set new primary address (demotes others)
CREATE OR REPLACE FUNCTION set_party_primary_address(
    p_party_id UUID,
    p_address_id UUID
)
RETURNS VOID AS $$
BEGIN
    -- Demote all current primary addresses for this party
    UPDATE core.party_addresses
    SET is_primary = false,
        updated_at = CURRENT_TIMESTAMP
    WHERE party_id = p_party_id
      AND is_primary = true
      AND address_id != p_address_id;

    -- Promote the new primary address
    UPDATE core.party_addresses
    SET is_primary = true,
        updated_at = CURRENT_TIMESTAMP
    WHERE party_id = p_party_id
      AND address_id = p_address_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Migration complete!
-- ============================================================================
