-- Migration: Audit Trail
-- Date: 2025-11-22
-- Purpose: Implement JSONB-based audit logging for critical tables

-- 1. Create Audit Schema
CREATE SCHEMA IF NOT EXISTS audit;

-- 2. Create Audit Log Table
CREATE TABLE audit.logged_actions (
    event_id bigserial PRIMARY KEY,
    schema_name text NOT NULL,
    table_name text NOT NULL,
    relid oid NOT NULL,
    session_user_name text,
    action_tstamp_tx timestamp with time zone NOT NULL,
    action_tstamp_stm timestamp with time zone NOT NULL,
    transaction_id bigint,
    application_name text,
    client_addr inet,
    client_port integer,
    client_query text,
    action TEXT NOT NULL CHECK (action IN ('I','D','U', 'T')),
    row_data jsonb,
    changed_fields jsonb,
    statement_only boolean NOT NULL
);

CREATE INDEX idx_audit_logged_actions_relid ON audit.logged_actions(relid);
CREATE INDEX idx_audit_logged_actions_tstamp ON audit.logged_actions(action_tstamp_stm);
CREATE INDEX idx_audit_logged_actions_action ON audit.logged_actions(action);

-- 3. Create Generic Audit Function
CREATE OR REPLACE FUNCTION audit.if_modified_func() RETURNS TRIGGER AS $body$
DECLARE
    audit_row audit.logged_actions;
    include_values boolean;
    log_diffs boolean;
    h_old jsonb;
    h_new jsonb;
    excluded_cols text[] = ARRAY[]::text[];
BEGIN
    IF TG_WHEN <> 'AFTER' THEN
        RAISE EXCEPTION 'audit.if_modified_func() may only run as an AFTER trigger';
    END IF;

    audit_row = ROW(
        nextval('audit.logged_actions_event_id_seq'), -- event_id
        TG_TABLE_SCHEMA::text,                        -- schema_name
        TG_TABLE_NAME::text,                          -- table_name
        TG_RELID,                                     -- relid
        session_user::text,                           -- session_user_name
        current_timestamp,                            -- action_tstamp_tx
        statement_timestamp(),                        -- action_tstamp_stm
        txid_current(),                               -- transaction_id
        current_setting('application_name'),          -- application_name
        inet_client_addr(),                           -- client_addr
        inet_client_port(),                           -- client_port
        current_query(),                              -- client_query
        substring(TG_OP,1,1),                         -- action
        NULL, NULL,                                   -- row_data, changed_fields
        'f'                                           -- statement_only
    );

    IF (TG_OP = 'UPDATE' AND TG_LEVEL = 'ROW') THEN
        h_old = to_jsonb(OLD);
        h_new = to_jsonb(NEW);
        audit_row.row_data = h_old;
        audit_row.changed_fields = (
            SELECT jsonb_object_agg(key, value)
            FROM jsonb_each(h_new)
            WHERE NOT h_old ? key OR h_new -> key <> h_old -> key
        );
        IF audit_row.changed_fields IS NULL THEN
            audit_row.changed_fields = '{}'::jsonb;
        END IF;
    ELSIF (TG_OP = 'DELETE' AND TG_LEVEL = 'ROW') THEN
        audit_row.row_data = to_jsonb(OLD);
    ELSIF (TG_OP = 'INSERT' AND TG_LEVEL = 'ROW') THEN
        audit_row.row_data = to_jsonb(NEW);
    END IF;

    INSERT INTO audit.logged_actions VALUES (audit_row.*);
    RETURN NULL;
END;
$body$ LANGUAGE plpgsql SECURITY DEFINER;

-- 4. Helper Function to Apply Audit
CREATE OR REPLACE FUNCTION audit.audit_table(target_table regclass) RETURNS void AS $body$
DECLARE
  stm_targets text = 'INSERT OR UPDATE OR DELETE OR TRUNCATE';
  _q_txt text;
BEGIN
    EXECUTE 'DROP TRIGGER IF EXISTS audit_trigger_row ON ' || target_table;
    EXECUTE 'DROP TRIGGER IF EXISTS audit_trigger_stm ON ' || target_table;

    _q_txt = 'CREATE TRIGGER audit_trigger_row AFTER INSERT OR UPDATE OR DELETE ON ' || 
             target_table || ' FOR EACH ROW EXECUTE PROCEDURE audit.if_modified_func();';
    RAISE NOTICE '%',_q_txt;
    EXECUTE _q_txt;

    _q_txt = 'CREATE TRIGGER audit_trigger_stm AFTER TRUNCATE ON ' || 
             target_table || ' FOR EACH STATEMENT EXECUTE PROCEDURE audit.if_modified_func();';
    RAISE NOTICE '%',_q_txt;
    EXECUTE _q_txt;
END;
$body$ LANGUAGE plpgsql;

-- 5. Apply Audit to Critical Tables
SELECT audit.audit_table('core.entities');
SELECT audit.audit_table('core.parties');
SELECT audit.audit_table('core.real_estate_properties');
SELECT audit.audit_table('core.property_ownerships');
SELECT audit.audit_table('core.entity_phones');
SELECT audit.audit_table('core.entity_emails');
SELECT audit.audit_table('core.entity_addresses');
