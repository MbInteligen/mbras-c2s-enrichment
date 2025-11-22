-- Migration 011: Add party references to property_transactions and backfill
-- Date: 2025-11-22
-- Purpose: Introduce buyer_party_id / seller_party_id mapped from legacy entity IDs

BEGIN;

-- 1) Add new nullable party columns
ALTER TABLE core.property_transactions
    ADD COLUMN IF NOT EXISTS buyer_party_id UUID,
    ADD COLUMN IF NOT EXISTS seller_party_id UUID;

-- 2) Backfill from legacy entity IDs using cpf_cnpj match
UPDATE core.property_transactions pt
SET buyer_party_id = p.id
FROM core.entities e
JOIN core.parties p ON p.cpf_cnpj = e.national_id
WHERE pt.buyer_entity_id = e.entity_id
  AND pt.buyer_party_id IS NULL;

UPDATE core.property_transactions pt
SET seller_party_id = p.id
FROM core.entities e
JOIN core.parties p ON p.cpf_cnpj = e.national_id
WHERE pt.seller_entity_id = e.entity_id
  AND pt.seller_party_id IS NULL;

-- 3) Add indexes for new columns
CREATE INDEX IF NOT EXISTS idx_transactions_buyer_party
    ON core.property_transactions (buyer_party_id)
    WHERE buyer_party_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_transactions_seller_party
    ON core.property_transactions (seller_party_id)
    WHERE seller_party_id IS NOT NULL;

-- 4) Add FKs to parties (keep legacy FKs for now)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_property_transactions_buyer_party'
    ) THEN
        ALTER TABLE core.property_transactions
            ADD CONSTRAINT fk_property_transactions_buyer_party
            FOREIGN KEY (buyer_party_id) REFERENCES core.parties(id) ON DELETE SET NULL;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_property_transactions_seller_party'
    ) THEN
        ALTER TABLE core.property_transactions
            ADD CONSTRAINT fk_property_transactions_seller_party
            FOREIGN KEY (seller_party_id) REFERENCES core.parties(id) ON DELETE SET NULL;
    END IF;
END $$;

-- 5) Verification summary
DO $$
DECLARE
    buyer_count INTEGER;
    seller_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO buyer_count FROM core.property_transactions WHERE buyer_party_id IS NOT NULL;
    SELECT COUNT(*) INTO seller_count FROM core.property_transactions WHERE seller_party_id IS NOT NULL;

    RAISE NOTICE 'property_transactions buyer_party_id populated: %', buyer_count;
    RAISE NOTICE 'property_transactions seller_party_id populated: %', seller_count;
END $$;

COMMIT;
