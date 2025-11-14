-- Core schema for Rust C2S API (matches production Neon DB structure)

-- Create schemas
CREATE SCHEMA IF NOT EXISTS app;
CREATE SCHEMA IF NOT EXISTS core;
CREATE SCHEMA IF NOT EXISTS dim;

-- Parties table (replaces separate customers/companies tables)
CREATE TABLE IF NOT EXISTS core.parties (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_type text NOT NULL,
    cpf_cnpj text NOT NULL,
    full_name text NOT NULL,
    normalized_name text,
    sex character(1),
    birth_date date,
    mother_name text,
    father_name text,
    rg text,
    fantasy_name text,
    normalized_fantasy_name text,
    opening_date date,
    registration_status_date date,
    company_type text,
    company_size text,
    enriched boolean DEFAULT false,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_parties_cpf_cnpj ON core.parties(cpf_cnpj);
CREATE INDEX IF NOT EXISTS idx_parties_type ON core.parties(party_type);
CREATE INDEX IF NOT EXISTS idx_parties_normalized_name ON core.parties(normalized_name);

-- IPTUs table
CREATE TABLE IF NOT EXISTS app.iptus (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    contributor_number text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    property_street_name text,
    property_number text,
    property_postal_code text,
    address_id uuid,
    owner_name text,
    built_area_m2 numeric,
    land_area_m2 numeric
);

CREATE INDEX IF NOT EXISTS idx_iptus_contributor_number ON app.iptus(contributor_number);

-- Party IPTUs junction table
CREATE TABLE IF NOT EXISTS core.party_iptus (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid REFERENCES core.parties(id),
    iptu_id uuid REFERENCES app.iptus(id),
    created_at timestamp with time zone DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_party_iptus_party ON core.party_iptus(party_id);
CREATE INDEX IF NOT EXISTS idx_party_iptus_iptu ON core.party_iptus(iptu_id);

-- Emails table
CREATE TABLE IF NOT EXISTS app.emails (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email text NOT NULL,
    created_at timestamp with time zone DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_emails_email ON app.emails(email);

-- Party Emails junction table
CREATE TABLE IF NOT EXISTS core.party_emails (
    party_id uuid REFERENCES core.parties(id),
    email_id uuid REFERENCES app.emails(id),
    ranking integer,
    verified boolean DEFAULT false,
    PRIMARY KEY (party_id, email_id)
);

-- Phones table
CREATE TABLE IF NOT EXISTS app.phones (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    number text NOT NULL,
    country_code text DEFAULT '+55',
    created_at timestamp with time zone DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phones_number ON app.phones(number);

-- Party Phones junction table
CREATE TABLE IF NOT EXISTS core.party_phones (
    party_id uuid REFERENCES core.parties(id),
    phone_id uuid REFERENCES app.phones(id),
    ranking integer,
    is_whatsapp boolean DEFAULT false,
    PRIMARY KEY (party_id, phone_id)
);

-- Materialized view for analytics
CREATE MATERIALIZED VIEW IF NOT EXISTS dim.party_iptu_summary AS
SELECT
    p.id as party_id,
    p.cpf_cnpj as national_id,
    p.full_name as name,
    p.party_type,
    COUNT(pi.iptu_id) as total_iptu_count,
    COUNT(pi.iptu_id) as verified_iptu_count
FROM core.parties p
LEFT JOIN core.party_iptus pi ON p.id = pi.party_id
GROUP BY p.id, p.cpf_cnpj, p.full_name, p.party_type;

-- Comprehensive IPTU + party enrichment view
CREATE MATERIALIZED VIEW IF NOT EXISTS public.iptu_party_enriched AS
WITH base AS (
    SELECT
        i.id                                  AS iptu_id,
        i.contributor_number,
        i.created_at          AS iptu_created_at,
        i.updated_at          AS iptu_updated_at,
        i.property_street_name,
        i.property_number,
        i.property_postal_code,
        i.address_id,
        i.owner_name,
        i.built_area_m2,
        i.land_area_m2,
        pi.party_id,
        p.party_type,
        p.cpf_cnpj            AS national_id,
        p.full_name           AS party_name,
        p.normalized_name     AS normalized_party_name,
        p.sex,
        p.birth_date,
        p.mother_name,
        p.father_name,
        p.rg,
        p.fantasy_name,
        p.normalized_fantasy_name,
        p.opening_date,
        p.registration_status_date,
        p.company_type,
        p.company_size,
        p.enriched            AS party_enriched,
        p.created_at          AS party_created_at,
        p.updated_at          AS party_updated_at
    FROM app.iptus i
    LEFT JOIN core.party_iptus pi ON pi.iptu_id = i.id
    LEFT JOIN core.parties p ON p.id = pi.party_id
),
party_emails AS (
    SELECT
        pe.party_id,
        jsonb_agg(
            jsonb_build_object(
                'email', e.email,
                'verified', pe.verified,
                'ranking', pe.ranking
            )
            ORDER BY pe.ranking NULLS LAST, e.email
        ) AS emails
    FROM core.party_emails pe
    JOIN app.emails e ON e.id = pe.email_id
    GROUP BY pe.party_id
),
party_phones AS (
    SELECT
        pp.party_id,
        jsonb_agg(
            jsonb_build_object(
                'phone', ph.number,
                'country_code', ph.country_code,
                'is_whatsapp', pp.is_whatsapp,
                'ranking', pp.ranking
            )
            ORDER BY pp.ranking NULLS LAST, ph.number
        ) AS phones
    FROM core.party_phones pp
    JOIN app.phones ph ON ph.id = pp.phone_id
    GROUP BY pp.party_id
)
SELECT
    b.*,
    COALESCE(pe.emails, '[]'::jsonb) AS emails,
    COALESCE(pp.phones, '[]'::jsonb) AS phones,
    NOW() AT TIME ZONE 'UTC' AS generated_at
FROM base b
LEFT JOIN party_emails pe ON pe.party_id = b.party_id
LEFT JOIN party_phones pp ON pp.party_id = b.party_id;

CREATE INDEX IF NOT EXISTS iptu_party_enriched_party_idx
    ON public.iptu_party_enriched (party_type, national_id);
CREATE INDEX IF NOT EXISTS iptu_party_enriched_contributor_idx
    ON public.iptu_party_enriched (contributor_number);
CREATE INDEX IF NOT EXISTS iptu_party_enriched_address_idx
    ON public.iptu_party_enriched (address_id);
