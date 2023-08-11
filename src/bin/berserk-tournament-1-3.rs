// Get all lichess arena tournament games with time control 1+0 or 3+0 where at
// least one player berserked.

use pgn2csv::{
    comments::Clock,
    headers::{PgnResult, Rating, Termination, TimeControl},
    pgn2csv, GameProcessor,
};

use std::{env, mem};

use anyhow::Result;
use bstr::ByteSlice;
use pgn_reader::{RawComment, RawHeader, Skip, Visitor};
use serde::Serialize;

#[derive(Default, Serialize)]
struct Row {
    white_rating: Rating,
    black_rating: Rating,
    time: u32,
    berserk: u8,
    result: u8,
    termination: u8,
}

#[derive(Default)]
struct Scratch {
    moves_with_clk: u8,
    white_berserked: bool,
    black_berserked: bool,
    skip_game: bool,
}

impl Scratch {
    fn berserk_code(&self) -> u8 {
        if self.white_berserked {
            if self.black_berserked {
                3 // both sides berserked
            } else {
                1 // white berserked but black didn't
            }
        } else if self.black_berserked {
            2 // black berserked but white didn't
        } else {
            0 // neither side berserk
        }
    }

    fn reset(&mut self) {
        self.moves_with_clk = 0;
        self.white_berserked = false;
        self.black_berserked = false;
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
            b"WhiteElo" => match Rating::try_from(value) {
                Ok(rating) => {
                    self.row.white_rating = rating;
                }
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"BlackElo" => match Rating::try_from(value) {
                Ok(rating) => {
                    self.row.black_rating = rating;
                }
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"Event" => {
                // we only want arena games (is there a way to disclude swiss?)
                if !value.as_bytes().contains_str("tournament") {
                    self.scratch.skip_game = true;
                }
            }
            b"TimeControl" => match TimeControl::try_from(value) {
                Ok(tc) => {
                    if tc.increment > 0 || (tc.initial_time != 60 && tc.initial_time != 180) {
                        self.scratch.skip_game = true;
                        return;
                    }
                    self.row.time = tc.initial_time;
                }
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"Termination" => match Termination::try_from(value) {
                Ok(termination) => match termination {
                    Termination::Normal => self.row.termination = 0,
                    Termination::TimeForfeit => self.row.termination = 1,
                    _ => self.scratch.skip_game = true,
                },
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            b"Result" => match PgnResult::try_from(value) {
                Ok(result) => match result {
                    PgnResult::WhiteWin => self.row.result = 2,
                    PgnResult::Draw => self.row.result = 1,
                    PgnResult::BlackWin => self.row.result = 0,
                    PgnResult::Other => self.scratch.skip_game = true,
                },
                Err(_) => {
                    self.scratch.skip_game = true;
                }
            },
            _ => (),
        }
    }

    fn end_headers(&mut self) -> Skip {
        if self.skip() {
            // will we be recording this game?
            Skip(true) // no, so skip past the moves
        } else {
            Skip(false) // maybe, need to check the moves
        }
    }

    // this logic assumes only one comment per move
    fn comment(&mut self, comment: RawComment<'_>) {
        if self.skip() || self.scratch.moves_with_clk >= 2 {
            return;
        }

        match Clock::try_from(comment) {
            Ok(clock) => {
                let t = clock.total_seconds();
                if self.scratch.moves_with_clk == 0 {
                    // white's first move
                    if self.row.time > t {
                        self.scratch.white_berserked = true;
                    }
                } else if self.scratch.moves_with_clk == 1 {
                    // black's first move
                    if self.row.time > t {
                        self.scratch.black_berserked = true;
                    }
                }
                self.scratch.moves_with_clk += 1;
            }
            Err(_) => {
                self.scratch.skip_game = true;
            }
        }
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true)
    }

    fn end_game(&mut self) {
        if self.skip() {
            return;
        }
        if self.scratch.moves_with_clk < 2 {
            self.scratch.skip_game = true;
            return;
        }
        // since we are only looking at 1+0 and 3+0, we can write time in
        // minutes to save space
        self.row.time /= 60;
        self.row.berserk = self.scratch.berserk_code();
    }
}

fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "1");
    pgn2csv::<Processor>()?;
    Ok(())
}
