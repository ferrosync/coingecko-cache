
create table if not exists object_storage (
    id bigint primary key generated always as identity,
    sha256 bytea not null unique check (length(sha256) = 32 /* bytes */),
    data bytea not null,
    mime text
);

create table if not exists provenance (
    uuid uuid primary key,
    object_id bigint not null references object_storage(id),
    timestamp_utc timestamp not null,
    agent text not null,
    request_metadata jsonb,
    response_metadata jsonb,
    unique (uuid, object_id)
);

create table if not exists coin_dominance (
    id bigint primary key generated always as identity,
    provenance_uuid uuid not null,
    object_id bigint not null,
    timestamp_utc timestamp not null,
    imported_at_utc timestamp not null default (now() at time zone 'utc'),
    agent text not null,
    coin_id text not null,
    coin_name text not null,
    market_cap_usd numeric not null check (market_cap_usd >= 0),
    market_dominance_percentage numeric not null check (market_dominance_percentage >= 0 and market_dominance_percentage <= 100),
    unique (object_id, coin_id),
    foreign key (provenance_uuid, object_id) references provenance(uuid, object_id)
);

create index if on coin_dominance(timestamp_utc);

DO $$
BEGIN
    CREATE ROLE domfi_coingecko_ro LOGIN PASSWORD 'development_only';

EXCEPTION
    WHEN DUPLICATE_OBJECT THEN
        RAISE NOTICE 'CREATE ROLE already exists. Ignoring.';
END
$$;

grant select on table object_storage to domfi_coingecko_ro;
grant select on table provenance to domfi_coingecko_ro;
grant select on table coin_dominance to domfi_coingecko_ro;

DO $$
BEGIN
    CREATE ROLE domfi_coingecko_loader LOGIN PASSWORD 'development_only';

EXCEPTION
    WHEN DUPLICATE_OBJECT THEN
        RAISE NOTICE 'CREATE ROLE already exists. Ignoring.';
END
$$;
grant select, update, insert on table object_storage to domfi_coingecko_loader;
grant select, update, insert on table provenance to domfi_coingecko_loader;
grant select, update, insert on table coin_dominance to domfi_coingecko_loader;
