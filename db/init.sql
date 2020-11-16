
create table object_storage (
    id bigint primary key generated always as identity,
    sha256 bytea not null unique check (length(sha256) = 32 /* bytes */),
    data bytea not null,
    mime text
);

create table provenance (
    uuid uuid primary key,
    object_id bigint not null references object_storage(id),
    timestamp_utc timestamp not null,
    agent text not null,
    request_metadata jsonb,
    response_metadata jsonb,
    unique (uuid, object_id)
);

create table coin_dominance (
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

create index on coin_dominance(timestamp_utc);

create role domfi_coingecko_ro
    login password 'development_only';
grant select on table object_storage to domfi_coingecko_ro;
grant select on table provenance to domfi_coingecko_ro;
grant select on table coin_dominance to domfi_coingecko_ro;

create role domfi_coingecko_loader
    login password 'development_only';
grant select, update, insert on table object_storage to domfi_coingecko_loader;
grant select, update, insert on table provenance to domfi_coingecko_loader;
grant select, update, insert on table coin_dominance to domfi_coingecko_loader;
