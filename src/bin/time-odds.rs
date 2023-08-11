// Get all lichess games where one player started the game with more time than
// the other. The vast majority of these will be from berserking.

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
    white_initial_time: u32,
    black_initial_time: u32,
    initial_time: u32,
    increment: u32,
    result: u8,
    termination: u8,
    tournament: bool,
}

#[derive(Default)]
struct Scratch {
    moves_with_clk: u8,
    white_time_odds: bool,
    black_time_odds: bool,
    white_prev_time: u32,
    black_prev_time: u32,
    skip_game: bool,
}

impl Scratch {
    fn reset(&mut self) {
        self.moves_with_clk = 0;
        self.white_time_odds = false;
        self.black_time_odds = false;
        self.white_prev_time = 0;
        self.black_prev_time = 0;
        self.skip_game = false;
    }

    fn white_move(&self) -> bool {
        self.moves_with_clk % 2 == 0
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
                self.row.tournament = value.as_bytes().contains_str("tournament");
            }
            b"TimeControl" => match TimeControl::try_from(value) {
                Ok(tc) => {
                    self.row.initial_time = tc.initial_time;
                    self.row.increment = tc.increment;
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

    fn comment(&mut self, comment: RawComment<'_>) {
        if self.skip() {
            return;
        }

        match Clock::try_from(comment) {
            Ok(clock) => {
                let t = clock.total_seconds();
                if self.scratch.moves_with_clk == 0 {
                    // white's first move
                    if self.row.initial_time != t {
                        self.scratch.white_time_odds = true;
                    }
                    self.row.white_initial_time = t;
                    self.scratch.white_prev_time = t;
                } else if self.scratch.moves_with_clk == 1 {
                    // whether or not either side's initial time is different
                    // from the time control, if the sides are equal to each
                    // other, we don't want the game.
                    if t == self.row.white_initial_time {
                        self.scratch.skip_game = true;
                        return;
                    }
                    // black's first move
                    if self.row.initial_time != t {
                        self.scratch.black_time_odds = true;
                    }
                    self.row.black_initial_time = t;
                    self.scratch.black_prev_time = t;
                } else if !self.scratch.white_time_odds && !self.scratch.black_time_odds {
                    self.scratch.skip_game = true;
                    return;
                } else if self.scratch.white_move() {
                    // don't include games where one side got extra time in the
                    // middle of a game (this won't detect all cases, but better
                    // than nothing). The +1 is to deal with rounding; I'm not
                    // sure whether/how it is done.
                    if t > self.scratch.white_prev_time + self.row.increment + 1 {
                        self.scratch.skip_game = true;
                        return;
                    }
                    self.scratch.white_prev_time = t;
                } else {
                    if t > self.scratch.black_prev_time + self.row.increment + 1 {
                        self.scratch.skip_game = true;
                        return;
                    }
                    self.scratch.black_prev_time = t;
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
        // only include games where both players made at least one move.
        if self.scratch.moves_with_clk < 2 {
            self.scratch.skip_game = true;
        }
    }
}

fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "1");
    pgn2csv::<Processor>()?;
    Ok(())
}
