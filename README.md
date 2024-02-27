Get prices from Pyth or Switchboard with the same function.

Pass either Switchboard or Pyth Oracles as UncheckedAccounts, then use the included `get_oracle_price` function to validate the accounts and fetch a price. Included max age and confidence checks available for both oracles.

Want to mock oracles on localnet or devnet? Build with the localnet feature to skip confidence and age checks. Uses localnet key (E6xiKCViJ2E6YyfFEa7eRZx3ngX4KPSVTSVTLywaEwJ8). Build with the devnet-deploy feature to enable those features, but use the devnet Pyth address (gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s). Building without either feature uses the mainnet Pyth address (FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH)

Visit the repo: https://github.com/mithraiclabs/psyoracleutils