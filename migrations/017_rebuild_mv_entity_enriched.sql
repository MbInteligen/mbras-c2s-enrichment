-- Migration 017: Rebuild mv_entity_enriched to use Party Model tables
-- Date: 2025-11-22

BEGIN;

DROP MATERIALIZED VIEW IF EXISTS core.mv_entity_enriched;

CREATE MATERIALIZED VIEW core.mv_entity_enriched AS
SELECT
    p.id AS party_id,
    p.party_type,
    p.cpf_cnpj,
    p.full_name,
    p.normalized_name,
    COALESCE(pe.sex, p.sex) AS sex,
    COALESCE(pe.birth_date, p.birth_date) AS birth_date,
    COALESCE(pe.mothers_name, p.mother_name) AS mother_name,
    p.father_name,
    p.rg,
    pe.marital_status,
    p.company_type,
    p.company_size,
    p.opening_date,
    p.enriched,
    COALESCE(
        (
            SELECT jsonb_agg(
                jsonb_build_object(
                    'email', pc.value,
                    'type', 'email',
                    'is_primary', pc.is_primary,
                    'verified', pc.is_verified
                )
                ORDER BY pc.is_primary DESC NULLS LAST, pc.created_at
            )
            FROM core.party_contacts pc
            WHERE pc.party_id = p.id
              AND pc.contact_type = 'email'
            LIMIT 10
        ),
        '[]'::jsonb
    ) AS emails,
    COALESCE(
        (
            SELECT jsonb_agg(
                jsonb_build_object(
                    'phone', pc.value,
                    'type', pc.contact_type,
                    'is_primary', pc.is_primary,
                    'verified', pc.is_verified,
                    'is_whatsapp', pc.is_whatsapp
                )
                ORDER BY pc.is_primary DESC NULLS LAST, pc.created_at
            )
            FROM core.party_contacts pc
            WHERE pc.party_id = p.id
              AND pc.contact_type IN ('phone','whatsapp')
            LIMIT 10
        ),
        '[]'::jsonb
    ) AS phones,
    COALESCE(
        (
            SELECT jsonb_agg(
                jsonb_build_object(
                    'street', a.street,
                    'number', a.number,
                    'complement', a.complement,
                    'neighborhood', a.neighborhood,
                    'city', a.city,
                    'state', a.state,
                    'zip_code', a.zip_code,
                    'address_type', pa.address_type,
                    'is_current', pa.is_current,
                    'latitude', a.latitude,
                    'longitude', a.longitude,
                    'confidence', pa.confidence_score
                )
                ORDER BY pa.is_primary DESC NULLS LAST, pa.created_at DESC
            )
            FROM core.party_addresses pa
            JOIN core.addresses a ON pa.address_id = a.id
            WHERE pa.party_id = p.id
            LIMIT 5
        ),
        '[]'::jsonb
    ) AS addresses,
    COALESCE(
        (
            SELECT jsonb_agg(
                jsonb_build_object(
                    'related_party_id',
                    CASE
                        WHEN pr.source_party_id = p.id THEN pr.target_party_id
                        ELSE pr.source_party_id
                    END,
                    'related_party_name',
                    CASE
                        WHEN pr.source_party_id = p.id THEN pt.full_name
                        ELSE ps.full_name
                    END,
                    'relationship_type', pr.relationship_type,
                    'confidence', pr.confidence
                )
                ORDER BY pr.confidence DESC NULLS LAST
            )
            FROM core.party_relationships pr
            LEFT JOIN core.parties ps ON pr.source_party_id = ps.id
            LEFT JOIN core.parties pt ON pr.target_party_id = pt.id
            WHERE pr.source_party_id = p.id
               OR pr.target_party_id = p.id
            LIMIT 20
        ),
        '[]'::jsonb
    ) AS relationships
FROM core.parties p
LEFT JOIN core.people pe ON pe.party_id = p.id;

COMMIT;
