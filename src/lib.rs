pub mod comments;
pub mod headers;

use std::{
    fs::{create_dir, File},
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::Result;
use bzip2::read::MultiBzDecoder;
use globwalk::{DirEntry, GlobWalkerBuilder};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use pgn_reader::{BufferedReader, Visitor};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::Serialize;
use zstd::stream::read::Decoder as ZstdDecoder;

enum Compression {
    None,
    Bzip2,
    Zstd,
}

struct Pgn {
    path: PathBuf,
}

impl From<DirEntry> for Pgn {
    fn from(dir_entry: DirEntry) -> Self {
        Pgn {
            path: dir_entry.into_path(),
        }
    }
}

impl Pgn {
    fn csv_path(&self, csv_dir: &Path) -> PathBuf {
        let mut csv_path = csv_dir.to_path_buf();
        csv_path.push(self.path.file_name().unwrap_or_default());
        csv_path.set_extension("csv");
        csv_path
    }

    fn compression(&self) -> Compression {
        match self.path.extension() {
            Some(ext) => match ext.to_str() {
                Some("bz2") => Compression::Bzip2,
                Some("zst") => Compression::Zstd,
                _ => Compression::None,
            },
            None => Compression::None,
        }
    }

    fn reader(&self) -> Result<BufferedReader<Box<dyn Read>>> {
        let file = File::open(&self.path)?;
        let reader: Box<dyn Read> = match self.compression() {
            Compression::None => Box::new(file),
            Compression::Bzip2 => Box::new(MultiBzDecoder::new(file)),
            Compression::Zstd => Box::new(ZstdDecoder::new(file)?),
        };
        Ok(BufferedReader::new(reader))
    }

    fn process<P>(&self, processor: &mut P, csv: &mut Csv) -> Result<()>
    where
        P: Visitor + GameProcessor,
    {
        let mut pgn_reader = self.reader()?;
        while let Ok(Some(_)) = pgn_reader.read_game(processor) {
            if processor.skip() {
                continue;
            }
            csv.write_row(processor.row())?;
        }
        csv.flush()?;
        Ok(())
    }
}

fn dir_pgns(dir: &Path) -> Result<Vec<Pgn>> {
    let exts = ["*.pgn", "*.pgn.bz2", "*.pgn.zst"];
    let pgns = GlobWalkerBuilder::from_patterns(dir, &exts)
        .max_depth(1)
        .build()?
        .filter_map(Result::ok)
        .map(Pgn::from)
        .collect();
    Ok(pgns)
}

struct Csv {
    writer: csv::Writer<File>,
}

impl Csv {
    fn new(csv_dir: &Path, pgn: &Pgn) -> Result<Self> {
        let csv_path = pgn.csv_path(csv_dir);
        let file = File::create(csv_path)?;
        let writer = csv::Writer::from_writer(file);
        Ok(Self { writer })
    }

    fn write_row(&mut self, row: impl Serialize) -> Result<()> {
        self.writer.serialize(row)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

pub trait GameProcessor: Default {
    type Row: Default + Serialize;

    fn skip(&self) -> bool {
        false
    }

    fn row(&mut self) -> Self::Row;
}

fn progress_bar(n: usize, message: &str) -> Result<ProgressBar> {
    let pb = ProgressBar::new(u64::try_from(n)?);
    let template = format!("{{spinner:.green}} {message}: [{{elapsed}}] [{{bar:.cyan/blue}}] {{human_pos}}/{{human_len}} ({{eta}})");
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&template)?
            .progress_chars("#>-"),
    );
    Ok(pb)
}

/// Converts PGN files to CSVs. Reads one or two command line arguments: the
/// path to a directory containing PGN files, and the path to a directory to
/// write CSV files; if the second argument is not provided, the CSV files will
/// be written to the same directory as the PGN files. The CSV files will have
/// the same name as the PGN files, but with the extension replaced with `.csv`.
/// To customize the data that you collect into the CSVs, you provide the
/// generic type parameter `P` to the function, which must implement the
/// `Visitor` and `GameProcessor` traits. See the README for more information.
///
/// # Errors
///
/// Returns an error if there is an issue with reading or writing files.
pub fn pgn2csv<P>() -> Result<()>
where
    P: Visitor + GameProcessor,
{
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 && args.len() != 3 {
        println!("Usage: {} <pgn dir> [csv dir]", args[0]);
        std::process::exit(1);
    }
    let pgn_dir = Path::new(&args[1]);
    let csv_dir = if args.len() == 3 {
        Path::new(&args[2])
    } else {
        pgn_dir
    };

    if !csv_dir.exists() {
        create_dir(csv_dir)?;
    }

    let pgns = dir_pgns(pgn_dir)?;

    let pb = progress_bar(pgns.len(), "Processing PGNs")?;

    pgns.par_iter()
        .progress_with(pb)
        .try_for_each(|pgn| -> Result<()> {
            let mut csv = Csv::new(csv_dir, pgn)?;
            let mut processor = P::default();
            pgn.process(&mut processor, &mut csv)?;
            Ok(())
        })?;
    Ok(())
}
