
create table raw_response (
    id bigint primary key generated always as identity,
    timestamp_utc timestamp not null,
    status_code smallint,
    status_text text,
    body text,
    headers jsonb
);

create table coin (
    id text primary key,
    symbol text not null
);

create table coin_dominance (
    timestamp_utc timestamp not null,
    coin_id text not null references coin(id),
    market_cap_usd numeric(21, 21) check (market_cap_usd >= 0),
    market_dominance_percentage numeric(21, 21) check (market_dominance_percentage >= 0 and market_dominance_percentage <= 100),
    primary key (timestamp_utc, coin_id)
)
