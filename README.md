A small rust crate that adds some minor conveniences on top of [rust-pgn-reader](https://github.com/niklasf/rust-pgn-reader) together with [rayon](https://github.com/rayon-rs/rayon) to process in parallel a directory of compressed or uncompressed PGN files into CSVs.

To generate a new set of CSVs containing data to your specific requirements, you write a binary file whose `main()` function calls `pgn2csv::pgn2csv::<P>()`, where `P` is a type that you create that implements the traits `Default`, `pgn_reader::Visitor`, and `pgn2csv::GameProcessor`. `GameProcessor` has two methods, `skip()` and `row()`, that respectively define whether a specific game's data is relevant to you and should be included as a row in the csv, and what data that row should hold. The latter should return a type that implements `Default` and `serde::Serialize`. There are a couple examples of usage in `src/bin`.

## Usage

To use one of the existing binaries in `src/bin`, you must have [Rust installed](https://www.rust-lang.org/tools/install) on your system. Then, from the root directory of this repository, run e.g.:

```
cargo run --release --bin time-odds path/to/pgns path/to/csvs
```

where `time-odds` can be replaced with the name of any of the binaries in `src/bin`. This will convert `.pgn`, `.pgn.bz2`, or `.pgn.zst` files in directory `path/to/pgns` to `.csv` files in directory `path/to/csvs`. Running the command with just the first argument will write the CSVs to the same directory as the pgns. In either case, the CSVs will have the same name as the PGNs, but with the final extension replaced with `.csv`.
