use std::collections::HashMap;
use std::io::{prelude::*, SeekFrom};
use std::{fs::File, io::BufReader};

#[derive(Debug)]
struct FieldRef {
    pos: usize,
    len: usize,
}

#[derive(Debug, Clone)]
pub struct Field {
    name: Vec<u8>,
    value: Vec<u8>,
}

#[derive(Debug)]
enum FieldState {
    ExpectMid,
    ExpectEnd,
    ExpectStart,
}

#[derive(Debug)]
pub enum XRVErr {
    FailToOpenFile(std::io::Error),
    FailToReadUntil(std::io::Error),
    FailToEnumerateByte(usize),

    FieldNameFailedToParse(usize, usize),
    FieldValueFailedToParse(usize, usize),
    NextFieldFailedToParse(usize, usize),

    EmptyLine(Line),

    FailedToConsumeRefs(usize, usize),

    UnknownLine(Line),
    FailToGetStreamPosition(std::io::Error),
}

pub enum Lines {
    JumpLine(Line),
    TableLine(Line),
    StyleLine(Line),
    RecordLine(Line),
}

#[derive(Debug)]
pub struct Line {
    buffer: Vec<u8>,
    start: u64,
    len: usize,
}

#[derive(Debug)]
pub struct XRVReader {
    buffer: BufReader<File>,
    pub seek: usize,
    pub line: usize,
    pub jumps: HashMap<Vec<u8>, Vec<u8>>,
    pub tables: HashMap<Vec<u8>, Vec<Field>>,
    pub styles: HashMap<Vec<u8>, Vec<Field>>,
}

const TABLECHAR: u8 = b't';
const STYLECHAR: u8 = b's';
const RECORDCHAR: u8 = b'r';
const JUMPCHAR: u8 = b'j';
const COLON: u8 = b':';
const SPACE: u8 = b' ';
const BRACKET: u8 = b'"';
const CR: u8 = b'\r';
const NEWLINE: u8 = b'\n';

impl XRVReader {
    pub fn new(path: String) -> Result<XRVReader, XRVErr> {
        match File::open(path) {
            Err(err) => Err(XRVErr::FailToOpenFile(err)),
            Ok(file) => Ok(XRVReader {
                buffer: BufReader::new(file),
                seek: 0,
                line: 0,
                jumps: HashMap::new(),
                tables: HashMap::new(),
                styles: HashMap::new(),
            }),
        }
    }

    pub fn parse_next(&mut self) -> Result<(), XRVErr> {
        let line = self.next()?;
        self.parse_line(XRVReader::linekind(line)?)?;
        Ok(())
    }

    fn next(&mut self) -> Result<Line, XRVErr> {
        let mut buffer: Vec<u8> = Vec::new();
        let start = match self.buffer.stream_position() {
            Err(err) => return Err(XRVErr::FailToGetStreamPosition(err)),
            Ok(pos) => pos,
        };
        match self.buffer.read_until(NEWLINE, &mut buffer) {
            Err(err) => return Err(XRVErr::FailToReadUntil(err)),
            Ok(len) => {
                self.line += 1;
                return Ok(Line { buffer, start, len });
            }
        }
    }

    fn linekind(line: Line) -> Result<Lines, XRVErr> {
        match line.buffer[0] {
            RECORDCHAR => Ok(Lines::RecordLine(line)),
            STYLECHAR => Ok(Lines::StyleLine(line)),
            TABLECHAR => Ok(Lines::TableLine(line)),
            JUMPCHAR => Ok(Lines::JumpLine(line)),
            CR => Err(XRVErr::EmptyLine(line)),
            NEWLINE => Err(XRVErr::EmptyLine(line)),
            _ => Err(XRVErr::UnknownLine(line)),
        }
    }

    fn parse_line(&mut self, input_line: Lines) -> Result<(), XRVErr> {
        match input_line {
            Lines::JumpLine(line) => {
                self.jumps = match XRVReader::meta_fields(line) {
                    Err(err) => return Err(err),
                    Ok(jumps) => XRVReader::to_hashmap(jumps[1..].to_vec()),
                }
            }
            Lines::TableLine(line) => match XRVReader::meta_fields(line) {
                Err(err) => return Err(err),
                Ok(table_fields) => {
                    self.tables
                        .insert(table_fields[0].value.clone(), table_fields[1..].to_vec());
                }
            },
            Lines::StyleLine(line) => match XRVReader::meta_fields(line) {
                Err(err) => return Err(err),
                Ok(style_fields) => {
                    self.styles
                        .insert(style_fields[0].value.clone(), style_fields[1..].to_vec());
                }
            },
            Lines::RecordLine(line) => {
                todo!();
            }
        };
        Ok(())
    }

    fn to_hashmap(fields: Vec<Field>) -> HashMap<Vec<u8>, Vec<u8>> {
        let mut hm: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        for field in fields {
            hm.insert(field.name, field.value);
        }
        hm
    }

    fn meta_fields(line: Line) -> Result<Vec<Field>, XRVErr> {
        let mut state = FieldState::ExpectMid;
        let mut seek: usize = 0;
        let it = line.buffer.bytes();

        let mut refs: Vec<FieldRef> = Vec::new();

        for (i, b) in it.enumerate() {
            match b {
                Err(err) => return Err(XRVErr::FailToEnumerateByte(i)),
                Ok(byte) => {
                    match (byte, &state) {
                        (COLON, FieldState::ExpectMid) => {
                            refs.push(FieldRef {
                                pos: seek,
                                len: i - seek,
                            });
                            seek = i + 1;
                            state = FieldState::ExpectEnd;
                        }

                        (COLON, _) => {
                            return Err(XRVErr::FieldNameFailedToParse(line.start as usize, i))
                        }
                        (SPACE, FieldState::ExpectEnd) => {
                            refs.push(FieldRef {
                                pos: seek,
                                len: i - seek,
                            });
                            seek = i + 1;
                            state = FieldState::ExpectStart;
                        }
                        (SPACE, _) => {
                            return Err(XRVErr::FieldValueFailedToParse(line.start as usize, i))
                        }
                        (CR, FieldState::ExpectEnd) => {
                            refs.push(FieldRef {
                                pos: seek,
                                len: i - seek,
                            });
                            break;
                        }
                        (NEWLINE, FieldState::ExpectEnd) => {
                            refs.push(FieldRef {
                                pos: seek,
                                len: i - seek,
                            });
                            break;
                        }
                        (_, FieldState::ExpectStart) => match byte {
                            COLON => {
                                return Err(XRVErr::NextFieldFailedToParse(line.start as usize, i))
                            }
                            SPACE => continue,
                            CR => {
                                return Err(XRVErr::NextFieldFailedToParse(line.start as usize, i))
                            }
                            NEWLINE => {
                                return Err(XRVErr::NextFieldFailedToParse(line.start as usize, i))
                            }
                            _ => {
                                state = FieldState::ExpectMid;
                            }
                        },
                        (_, FieldState::ExpectMid) => continue,
                        (_, FieldState::ExpectEnd) => {
                            continue;
                        }
                    };
                }
            }
        }
        let mut fields: Vec<Field> = Vec::new();
        let mut ref_it = refs.into_iter();

        let mut consume_idx: usize = 0;
        loop {
            match ref_it.next() {
                None => break,
                Some(fname) => match ref_it.next() {
                    None => {
                        return Err(XRVErr::FailedToConsumeRefs(
                            line.start as usize,
                            consume_idx,
                        ))
                    }
                    Some(fvalue) => {
                        fields.push(Field {
                            name: line.buffer[fname.pos..fname.pos + fname.len].to_vec(),
                            value: line.buffer[fvalue.pos..fvalue.pos + fvalue.len].to_vec(),
                        });
                        consume_idx += 1;
                    }
                },
            }
        }
        Ok(fields)
    }
}
