use anyhow::{anyhow, Error, Result};
use bstr::ByteSlice;
use bstr_parse::BStrParse;
use pgn_reader::RawHeader;
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct Rating(u16);

impl TryFrom<RawHeader<'_>> for Rating {
    type Error = Error;

    fn try_from(value: RawHeader<'_>) -> Result<Self> {
        Ok(Rating(value.as_bytes().parse::<u16>()?))
    }
}

/// A time control header like e.g. 300+0. This is the only time control
/// format currently supported; there is a [variety of other formats in the PGN
/// spec](http://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm#c9.6.1).
#[derive(Default, Serialize)]
pub struct TimeControl {
    pub initial_time: u32,
    pub increment: u32,
}

impl TryFrom<RawHeader<'_>> for TimeControl {
    type Error = Error;
    fn try_from(value: RawHeader<'_>) -> Result<Self> {
        let (initial_time, increment) = value
            .as_bytes()
            .split_once_str(&"+")
            .ok_or_else(|| anyhow!("expected time control with form time+inc"))?;
        Ok(TimeControl {
            initial_time: initial_time.parse::<u32>()?,
            increment: increment.parse::<u32>()?,
        })
    }
}

/// The variants are the possible values for Termination in lichess PGNs.
#[derive(Default, Serialize)]
pub enum Termination {
    #[default]
    Normal,
    TimeForfeit,
    Abandoned,
    RulesInfraction,
    Unterminated,
    Unknown,
}

impl TryFrom<RawHeader<'_>> for Termination {
    type Error = Error;

    fn try_from(header: RawHeader<'_>) -> Result<Self> {
        match header.as_bytes() {
            b"Normal" => Ok(Termination::Normal),
            b"Time forfeit" => Ok(Termination::TimeForfeit),
            b"Abandoned" => Ok(Termination::Abandoned),
            b"Rules infraction" => Ok(Termination::RulesInfraction),
            b"Unterminated" => Ok(Termination::Unterminated),
            b"Unknown" => Ok(Termination::Unknown),
            _ => Err(anyhow!("unexpected termination type")),
        }
    }
}

#[derive(Default, Serialize)]
pub enum PgnResult {
    WhiteWin,
    Draw,
    BlackWin,
    #[default]
    Other,
}

impl TryFrom<RawHeader<'_>> for PgnResult {
    type Error = Error;

    fn try_from(header: RawHeader<'_>) -> Result<Self> {
        match header.as_bytes() {
            b"1-0" => Ok(PgnResult::WhiteWin),
            b"1/2-1/2" => Ok(PgnResult::Draw),
            b"0-1" => Ok(PgnResult::BlackWin),
            b"*" => Ok(PgnResult::Other), // other/unfinished
            _ => Err(anyhow!("unexpected result type")),
        }
    }
}
