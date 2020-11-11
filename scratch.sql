
create table data_origin (
    uuid uuid primary key,
    agent text not null,
    timestamp_utc timestamp not null,
    data text not null,
    metadata text[][]
);

create table coin (
    id text primary key,
    symbol text not null
);

create table coin_dominance (
    id bigint primary key generated always as identity,
    data_origin_uuid uuid not null references data_origin(uuid),
    timestamp_utc timestamp not null,
    imported_at_utc timestamp not null default (now() at time zone 'utc'),
    agent text not null,
    coin_id text not null,
    coin_name text not null,
    market_cap_usd numeric not null check (market_cap_usd >= 0),
    market_dominance_percentage numeric not null check (market_dominance_percentage >= 0 and market_dominance_percentage <= 100),
    unique (agent, timestamp_utc, coin_id)
);

create index on coin_dominance(timestamp_utc);
