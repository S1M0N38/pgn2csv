use std::{env, io, time, str};
use std::fs::{File, create_dir};
use std::path::Path;
use globwalk::{DirEntry,GlobWalkerBuilder};
use humantime::format_duration;
use bzip2::read::MultiBzDecoder;
use serde::Serialize;
use regex::Regex;
use lazy_static::lazy_static;
use pgn_reader::{BufferedReader, RawComment, RawHeader, Visitor, Skip};
use rayon::prelude::*;

#[derive(Debug, Default, Serialize)]
struct Row {
    white_rating: String,
    black_rating: String,
    time: u32,
    berserk: u8,
    result: u8,
    termination: u8
}

struct GameProcessor {
    row: Row,
    writer: csv::Writer<File>,
    moves_with_clk: u8,
    white_berserked: bool,
    black_berserked: bool,
    skip_game: bool
}

impl GameProcessor {
    fn new(csv_path_str: &str) -> GameProcessor {
        GameProcessor {
            row: Row::default(), 
            writer: csv::Writer::from_path(csv_path_str)
                .expect("create csv writer from path"), 
            moves_with_clk: 0,
            white_berserked: false,
            black_berserked: false,
            skip_game: false
        }
    }
}

impl Visitor for GameProcessor {
    type Result = ();

    fn header(&mut self, key: &[u8], value: RawHeader<'_>) {
        match key {
            b"WhiteElo" => self.row.white_rating = String::from_utf8(value.as_bytes().to_vec())
                .expect("convert bytes to string"),
            b"BlackElo" => self.row.black_rating = String::from_utf8(value.as_bytes().to_vec())
                .expect("convert bytes to string"),
            b"Event" => {
                let event = str::from_utf8(value.as_bytes())
                    .expect("convert bytes to string");
                // we only want arena games (is there a way to disclude swiss?)
                if !event.contains("tournament") { 
                    self.skip_game = true;
                }
            }
            b"TimeControl" => {
                let time_control = str::from_utf8(value.as_bytes())
                    .expect("convert bytes to string");
                // time control in pgn is time+inc, with both time and inc in seconds
                let t = time_control.split("+").collect::<Vec<&str>>();
                if t.len() == 2 {
                    let time = t[0].parse::<u32>()
                        .expect("parse str to int");
                    if time != 60 && time != 180 { // we only want 1+0 and 3+0 games
                        self.skip_game = true;
                        return
                    } else {
                        self.row.time = time; 
                    }
                    let increment = t[1].parse::<u32>()
                        .expect("parse str to int");
                    if increment > 0 {
                        self.skip_game = true; // not interested in increment games
                    }
                } else {
                    self.skip_game = true;
                }
                
            }
            b"Termination" => {
                match value.as_bytes() {
                    b"Normal" => self.row.termination = 0,
                    b"Time forfeit" => self.row.termination = 1,
                    _ => self.skip_game = true // aborted or rules violation
                }
            }
            b"Result" => {
                match value.as_bytes() {
                    b"0-1" => self.row.result = 0,
                    b"1/2-1/2" => self.row.result = 1,
                    b"1-0" => self.row.result = 2,
                    _ => self.skip_game = true
                }
            }
            _ => ()
        }

        
    }

    fn end_headers(&mut self) -> Skip {
        if self.skip_game { // will we be recording this game?
            Skip(true) // no, so skip past the moves
        } else {
            Skip(false) // maybe, need to check the moves
        }
       
    }

    fn comment(&mut self, comment: RawComment<'_>) {
        // we need white and black to each have made one move and to each have a %clk.
        // we assume there is no more than one comment per move (ply, really)
        if !self.skip_game && self.moves_with_clk < 2 {
            lazy_static! {
                // we already filtered for only 1+0 and 3+0 games in the headers, 
                // so we don't care about the hours here
                static ref CLK_RE: Regex = Regex::new(r"%clk \d+:(\d+):(\d+)")
                    .expect("create regex from pattern");
            }
            match CLK_RE.captures(str::from_utf8(comment.as_bytes())
                .expect("convert bytes to string")
            ) {
                Some(caps) => { 
                    let minutes = caps.get(1).map_or(
                        0, 
                        |m| m
                            .as_str()
                            .parse::<u32>()
                            .expect("parse str to int")
                    );
                    let seconds = caps.get(2).map_or(
                        0, 
                        |m| m
                            .as_str()
                            .parse::<u32>()
                            .expect("parse str to int")
                    );
                    let t = minutes * 60 + seconds; // total time in seconds
                    if self.moves_with_clk == 0 { // white's first move
                        if self.row.time > t { 
                            self.white_berserked = true;
                        }
                    } else if self.moves_with_clk == 1 { // black's first move
                        if self.row.time > t { 
                            self.black_berserked = true;
                        }
                    }
                    self.moves_with_clk += 1;
                }
                None => { // no %clk, we're not interested in this game
                    self.skip_game = true;
                }
            }
        }
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true) 
    }

    fn end_game(&mut self) {
        // again, we want white and black to each have made at least one move
        if !self.skip_game && self.moves_with_clk == 2 {
            if !self.white_berserked {
                if !self.black_berserked {
                    self.row.berserk = 0; // neither side berserk
                } else {
                    self.row.berserk = 2; // black berserked but white didn't
                }
            } else {
                if !self.black_berserked {
                    self.row.berserk = 1; // white berserked but black didn't
                } else {
                    self.row.berserk = 3; // both sides berserked
                }
            }

            // since we are only looking at 1+0 and 3+0, 
            // we can write time in minutes to save space
            self.row.time = self.row.time / 60; 

            self.writer.serialize(&self.row)
                .expect("write csv row");

        }
        self.skip_game = false;   
        self.moves_with_clk = 0;
        self.black_berserked = false;
        self.white_berserked = false;
    }
}

fn main() -> Result<(), io::Error> {
    let total_time = time::Instant::now();
    let args: Vec<String> = env::args().skip(1).collect();
    let pgns_dir; 
    let csvs_dir;
    match args.len() {
        1 => {
            pgns_dir = Path::new(&args[0]);
            csvs_dir = pgns_dir;
        }
        2 => {
            pgns_dir = Path::new(&args[0]);
            csvs_dir = Path::new(&args[1]);
        }
        _ => panic!("provide either one or two arguments: input dir or input and output dirs")
    }

    if !csvs_dir.is_dir() {
        create_dir(csvs_dir)?;
    }

    let dir_entries: Vec<DirEntry> = GlobWalkerBuilder::from_patterns(
        pgns_dir, &["*.pgn", "*.pgn.bz2"]
        )
        .max_depth(1)
        .build()?
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    // uses rayon to parallelize processing the files 
    dir_entries.par_iter().for_each(|dir_entry: &DirEntry| {
        let now = time::Instant::now();

        let path = dir_entry.path();

        let path_str = path.to_str().expect("convert path to str");

        let file_name = path
            .file_name().expect("get file name from path")
            .to_str().expect("convert OsStr to str");

        println!("Processing {}...", path_str);

        let file = File::open(&path).expect("open file from path");

        let uncompressed: Box<dyn io::Read> = if path_str.ends_with(".bz2") {
            Box::new(MultiBzDecoder::new(file))
        } else {
            Box::new(file)
        };

        let mut reader = BufferedReader::new(uncompressed);

        lazy_static! {
            static ref EXT_RE: Regex = Regex::new(r"\.pgn(\.bz2)?$")
                .expect("create regex from pattern");
        }

        let csv_file_name = EXT_RE.replace(&file_name, ".csv");
        let csv_path = csvs_dir.join(Path::new(csv_file_name.as_ref()));
        let csv_path_str = csv_path.to_str().expect("convert path to str");
        let mut game_processor = GameProcessor::new(&csv_path_str);

        reader.read_all(&mut game_processor).expect("read pgn");

        game_processor.writer.flush().expect("flush csv writer");

        let duration = now.elapsed();
        println!("Wrote {}.\n\tElapsed time: {}.",
            csv_path_str,
            format_duration(duration).to_string());
        
    });
    let duration = total_time.elapsed();
    println!("All done. Total elapsed time: {}.", format_duration(duration).to_string());

    Ok(())
}