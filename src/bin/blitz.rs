use pgn2csv::{
    headers::{PgnResult, Rating, RatingDiff},
    pgn2csv, GameProcessor,
};

use std::{env, mem};

use anyhow::Result;
use pgn_reader::{RawHeader, Skip, Visitor};
use serde::Serialize;

#[derive(Default, Serialize)]
struct Row {
    white: String,
    black: String,
    result: i8,
    utc_date: String,
    utc_time: String,
    white_elo: Rating,
    black_elo: Rating,
    white_rating_diff: RatingDiff,
    black_rating_diff: RatingDiff,
}

#[derive(Default)]
struct Scratch {
    skip_game: bool,
}

impl Scratch {
    fn reset(&mut self) {
        self.skip_game = false;
    }
}

#[derive(Default)]
struct Processor {
    row: Row,
    scratch: Scratch,
}

impl GameProcessor for Processor {
    type Row = Row;

    fn skip(&self) -> bool {
        self.scratch.skip_game
    }

    fn row(&mut self) -> Row {
        mem::take(&mut self.row)
    }
}

impl Visitor for Processor {
    type Result = ();

    fn begin_game(&mut self) {
        self.scratch.reset();
    }

    fn header(&mut self, key: &[u8], value: RawHeader<'_>) {
        if self.skip() {
            return;
        }

        match key {
            b"Event" => {
                if value.as_bytes() != b"Rated Blitz game" {
                    self.scratch.skip_game = true;
                }
            }
            b"White" =>  {
                    self.row.white = String::from_utf8_lossy(value.as_bytes()).into_owned();
            },
            b"Black" =>  {
                    self.row.black = String::from_utf8_lossy(value.as_bytes()).into_owned();
            },
            b"Result" => match PgnResult::try_from(value) {
                Ok(result) => match result {
                    PgnResult::WhiteWin => self.row.result = 1,
                    PgnResult::Draw => self.row.result = 0,
                    PgnResult::BlackWin => self.row.result = -1,
                    PgnResult::Other => self.scratch.skip_game = true,
                },
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"UTCDate" =>  {
                    self.row.utc_date = String::from_utf8_lossy(value.as_bytes()).into_owned();
            },
            b"UTCTime" =>  {
                    self.row.utc_time = String::from_utf8_lossy(value.as_bytes()).into_owned();
            },
            b"WhiteElo" => match Rating::try_from(value) {
                Ok(rating) => {
                    self.row.white_elo = rating;
                }
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"BlackElo" => match Rating::try_from(value) {
                Ok(rating) => {
                    self.row.black_elo = rating;
                }
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"WhiteRatingDiff" => match RatingDiff::try_from(value) {
                Ok(rating) => {
                    self.row.white_rating_diff = rating;
                }
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"BlackRatingDiff" => match RatingDiff::try_from(value) {
                Ok(rating) => {
                    self.row.black_rating_diff = rating;
                }
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            _ => (),
        }
    }

    fn end_headers(&mut self) -> Skip {
        if self.skip() {
            Skip(true)
        } else {
            Skip(false)
        }
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true)
    }

    fn end_game(&mut self) {}
}

fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "1");
    pgn2csv::<Processor>()?;
    Ok(())
}
