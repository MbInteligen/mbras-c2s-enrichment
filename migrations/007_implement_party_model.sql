-- Migration: Implement Party Model
-- Date: 2025-11-22
-- Purpose: Create normalized tables for Party Model (People, Companies, Contacts, etc.)

-- 1. Create Enums
DO $$ BEGIN
    CREATE TYPE core.party_type_enum AS ENUM ('person', 'company');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE core.contact_type_enum AS ENUM ('email', 'phone', 'whatsapp');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE core.ownership_type_enum AS ENUM ('full', 'partial', 'usufruct');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE core.party_relationship_enum AS ENUM ('parent', 'child', 'spouse', 'partner', 'associate', 'other');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 2. Create Tables

-- core.people (Extension of core.parties for PF)
CREATE TABLE IF NOT EXISTS core.people (
    party_id uuid PRIMARY KEY REFERENCES core.parties(id),
    full_name text,
    mothers_name text,
    birth_date date,
    sex character(1),
    marital_status text,
    document_cpf text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);

-- core.companies (Extension of core.parties for PJ)
CREATE TABLE IF NOT EXISTS core.companies (
    party_id uuid PRIMARY KEY REFERENCES core.parties(id),
    legal_name text,
    trade_name text,
    cnpj text,
    company_size text,
    industry text,
    foundation_date date,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);

-- core.party_contacts
CREATE TABLE IF NOT EXISTS core.party_contacts (
    contact_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid NOT NULL REFERENCES core.parties(id),
    contact_type core.contact_type_enum NOT NULL,
    value text NOT NULL,
    is_primary boolean DEFAULT false,
    is_verified boolean DEFAULT false,
    is_whatsapp boolean DEFAULT false,
    source text,
    confidence numeric DEFAULT 0,
    valid_from timestamp with time zone DEFAULT now(),
    valid_to timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_party_contacts_party ON core.party_contacts(party_id);
CREATE INDEX IF NOT EXISTS idx_party_contacts_value ON core.party_contacts(value);

-- core.party_enrichments
CREATE TABLE IF NOT EXISTS core.party_enrichments (
    enrichment_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid NOT NULL REFERENCES core.parties(id),
    provider text,
    raw_payload jsonb,
    normalized_data jsonb,
    quality_score numeric,
    enriched_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_party_enrichments_party ON core.party_enrichments(party_id);

-- core.ownerships
CREATE TABLE IF NOT EXISTS core.ownerships (
    property_id uuid NOT NULL REFERENCES core.real_estate_properties(property_id),
    party_id uuid NOT NULL REFERENCES core.parties(id),
    ownership_type core.ownership_type_enum DEFAULT 'full',
    ownership_percent numeric,
    start_date date DEFAULT CURRENT_DATE,
    end_date date,
    is_current boolean DEFAULT true,
    source text,
    confidence numeric DEFAULT 0,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    PRIMARY KEY (property_id, party_id, start_date)
);
CREATE INDEX IF NOT EXISTS idx_ownerships_party ON core.ownerships(party_id);

-- core.party_relationships
CREATE TABLE IF NOT EXISTS core.party_relationships (
    relationship_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    source_party_id uuid NOT NULL REFERENCES core.parties(id),
    target_party_id uuid NOT NULL REFERENCES core.parties(id),
    relationship_type core.party_relationship_enum NOT NULL,
    start_date date DEFAULT CURRENT_DATE,
    end_date date,
    is_current boolean DEFAULT true,
    metadata jsonb DEFAULT '{}'::jsonb,
    confidence numeric DEFAULT 0,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_party_relationships_source ON core.party_relationships(source_party_id);
CREATE INDEX IF NOT EXISTS idx_party_relationships_target ON core.party_relationships(target_party_id);
