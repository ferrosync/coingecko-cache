
# DominationFinance - Coin Dominance Historical API
***powered by [CoinGecko](https://www.coingecko.com/)***

Since CoinGecko doesn't have a historical API for cryptocurrency coin dominance,
this functions as a workaround to cache and provide historical snapshots via a
REST API.

This contains the following subprojects:
 * `domfi_api` -- the REST API build on [`actix-web`](https://actix.rs/)
 * `domfi_loader` -- the CoinGecko API fetcher and database loader

## Building & Deploying

This project by default targets `x86_64-unknown-linux-musl` to allow it to be
compiled statically and ran on any Linux machine without having to recompile.
In other words, once you build it, you can `scp` the two binaries onto any Linux
machine.

To build everything for production, within the project root directory, run:
```
cargo build --release
```

The release binaries will be located in `target/x86_64-unknown-linux-musl/release/`.

### Postgres

The two binaries require a Postgres instance (preferably Postgres 13, but Postgres 10 or above should work).

A `docker-compose.yml` is provided which will automatically setup a Postgres 13
instance and run `ch_tbl_coin_dominance.sql` to create the database schema for you. This
requires that you have [Docker Compose installed](https://docs.docker.com/compose/install/).

To create the database, within the project root directory, run:
```
docker-compose up
```

Alternatively, simply run `ch_tbl_coin_dominance.sql` on Postgres >=10 database and update the
`.env` file accordingly.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
