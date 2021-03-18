
-- source: https://gist.github.com/colophonemes/9701b906c5be572a40a84b08f4d2fa4e

CREATE OR REPLACE FUNCTION notify_trigger() RETURNS trigger AS $trigger$
DECLARE
    rec RECORD;
    payload TEXT;
    column_name TEXT;
    column_value TEXT;
    payload_items TEXT[];
BEGIN
    -- Set record row depending on operation
    CASE TG_OP
        WHEN 'INSERT', 'UPDATE' THEN
            rec := NEW;
        WHEN 'DELETE' THEN
            rec := OLD;
        ELSE
            RAISE EXCEPTION 'Unknown TG_OP: "%". Should not occur!', TG_OP;
        END CASE;

    -- Get required fields
    FOREACH column_name IN ARRAY TG_ARGV LOOP
            EXECUTE format('SELECT to_json($1.%I)', column_name)
                INTO column_value
                USING rec;
            payload_items := array_append(payload_items, '"' || replace(column_name, '"', '\"') || '":' || column_value);
        END LOOP;

    -- Build the payload
    payload := ''
            || '{'
            || '"timestamp":'  || to_json(current_timestamp at time zone 'utc') || ','
            || '"operation":"' || TG_OP || '",'
            || '"schema":"'    || TG_TABLE_SCHEMA || '",'
            || '"table":"'     || TG_TABLE_NAME || '",'
            || '"data":{'      || array_to_string(payload_items, ',') || '}'
            || '}';

    -- Notify the channel
    PERFORM pg_notify('db_notify', payload);

    RETURN rec;
END;
$trigger$ LANGUAGE plpgsql;

CREATE TRIGGER coin_dominance_notify AFTER INSERT OR UPDATE ON coin_dominance
    FOR EACH ROW EXECUTE PROCEDURE notify_trigger(
        'id',
        'provenance_uuid',
        'object_id',
        'timestamp_utc',
        'imported_at_utc',
        'agent',
        'coin_id',
        'coin_name',
        'market_cap_usd',
        'market_dominance_percentage'
    );
