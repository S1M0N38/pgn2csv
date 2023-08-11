use anyhow::{anyhow, ensure, Error, Result};
use bstr::{ByteSlice, Split};
use bstr_parse::BStrParse;
use pgn_reader::RawComment;
use serde::Serialize;

// see https://www.enpassant.dk/chess/palview/enhancedpgn.htm
// The command string is structured as follows.
//
// A leading tag of [%
// A command name consisting of one or more alphanumeric characters.
// A space character.
// Either a single parameter or a comma delimited list of parameter values.
// A closing tag of ]
pub struct RawCommand<'a> {
    pub name: &'a [u8],
    pub params: Split<'a, 'a>,
}

impl<'a> TryFrom<&'a [u8]> for RawCommand<'a> {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self> {
        let (name, params) = value
            .split_once_str(" ")
            .ok_or_else(|| anyhow!("no space in comment command"))?;

        Ok(RawCommand {
            name,
            params: params.split_str(","),
        })
    }
}

pub struct RawCommandIterator<'a> {
    comment: &'a [u8],
}

impl<'a> Iterator for RawCommandIterator<'a> {
    type Item = RawCommand<'a>;
    fn next(&mut self) -> Option<RawCommand<'a>> {
        let start = self.comment.find("[%")?;
        let end = self.comment[start..].find("]")? + start;
        let command = &self.comment[start + 2..end];
        self.comment = &self.comment[end..];
        command.try_into().ok()
    }
}

pub trait RawCommands<'a> {
    fn raw_commands(&'a self) -> RawCommandIterator<'a>;
}

impl<'a> RawCommands<'a> for RawComment<'a> {
    fn raw_commands(&'a self) -> RawCommandIterator<'a> {
        RawCommandIterator {
            comment: self.as_bytes(),
        }
    }
}

#[derive(Default, Serialize)]
pub struct Clock {
    pub hours: u16,
    pub minutes: u8,
    pub seconds: u8,
}

impl<'a> TryFrom<&'a [u8]> for Clock {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self> {
        let mut parts = value.split_str(":");
        let hours = parts
            .next()
            .ok_or_else(|| anyhow!("no hours in clock"))?
            .parse()?;
        let minutes = parts
            .next()
            .ok_or_else(|| anyhow!("no minutes in clock"))?
            .parse()?;
        let seconds = parts
            .next()
            .ok_or_else(|| anyhow!("no seconds in clock"))?
            .parse()?;

        ensure!(parts.next().is_none(), "too many parts in clock");

        Ok(Clock {
            hours,
            minutes,
            seconds,
        })
    }
}

impl<'a> TryFrom<RawCommand<'a>> for Clock {
    type Error = Error;

    fn try_from(value: RawCommand<'a>) -> Result<Self> {
        // ensure!(value.name == b"clk", "not a clock command");
        let mut params = value.params;
        let time = params
            .next()
            .ok_or_else(|| anyhow!("no time in clock command"))?;
        ensure!(params.next().is_none(), "too many params in clock command");
        time.try_into()
    }
}

impl<'a> TryFrom<RawComment<'a>> for Clock {
    type Error = Error;

    fn try_from(value: RawComment<'a>) -> Result<Self> {
        for command in value.raw_commands() {
            if command.name == b"clk" {
                return command.try_into();
            }
        }
        Err(anyhow!("no clock command in comment"))
    }
}

impl Clock {
    #[must_use]
    pub fn total_seconds(&self) -> u32 {
        u32::from(self.hours) * 3600 + u32::from(self.minutes) * 60 + u32::from(self.seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_command() {
        let mut command = RawCommand::try_from(b"eval 0.17".as_slice()).unwrap();
        assert_eq!(command.name, b"eval");
        assert_eq!(command.params.next(), Some(b"0.17".as_slice()));
        assert_eq!(command.params.next(), None);

        command = RawCommand::try_from(b"list -0.1,2:34/5, 6, ".as_slice()).unwrap();
        assert_eq!(command.name, b"list");
        assert_eq!(command.params.next(), Some(b"-0.1".as_slice()));
        assert_eq!(command.params.next(), Some(b"2:34/5".as_slice()));
        assert_eq!(command.params.next(), Some(b" 6".as_slice()));
        assert_eq!(command.params.next(), Some(b" ".as_slice()));
    }

    #[test]
    fn raw_command_iter() {
        let comment = b" [%eval 0.17] [%clk 0:00:30] ";
        let mut iter = RawCommandIterator { comment };
        let mut command = iter.next().unwrap();
        assert_eq!(command.name, b"eval");
        assert_eq!(command.params.next(), Some(b"0.17".as_slice()));
        assert_eq!(command.params.next(), None);

        command = iter.next().unwrap();
        assert_eq!(command.name, b"clk");
        assert_eq!(command.params.next(), Some(b"0:00:30".as_slice()));
        assert_eq!(command.params.next(), None);

        assert!(iter.next().is_none());
    }
}
