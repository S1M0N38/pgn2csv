A small rust program that uses [rust-pgn-reader](https://github.com/niklasf/rust-pgn-reader) together with [rayon](https://github.com/rayon-rs/rayon) to process in parallel a directory of compressed or uncompressed PGN files into CSVs. The data to be compiled into the CSVs is hardcoded for my particular use case, but this could be used as a starting point to do something similar for some other use. This is much faster than doing the equivalent in e.g. `python-chess`; in my case, I was able to process at about 100 GB/h from `.pgn.bz2` files. If I have some other reason to process lichess PGNs into CSVs in the future, I will probably break this out into a separate repo and make the code generic, so that the particular data desired can be specified in a config file or as command line arguments.
## Usage
To convert `.pgn` or `.pgn.bz2` files in directory `path/to/pgns` to `.csv` files in directory `path/to/csvs`:
```
cargo run --release path/to/pgns path/to/csvs
```
