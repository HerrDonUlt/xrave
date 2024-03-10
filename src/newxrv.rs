use std::io::prelude::*;
use std::{collections::HashMap, fs::File, io::BufReader};

#[derive(Debug)]
enum LineKind {
    Table,
    Style,
    Record,
}

enum ExpectField {
    Name,
    Colon,
    Value,
    Skip,
    Qoute,
}

#[derive(Debug)]
struct LineField<'b> {
    buffer: &'b [u8],
    kind: LineKind,
    name: &'b str,
    fields: Vec<Field<'b>>,
}

#[derive(Debug, Clone)]
struct Field<'b> {
    name: &'b str,
    value: &'b str,
}

#[derive(Debug)]
struct LineLink<'b> {
    buffer: &'b [u8],
    kind: LineKind,
    name: &'b [u8],
    links: Vec<Link>,
}

#[derive(Debug)]
struct Link {
    name_start: usize,
    name_end: usize,
    value_start: usize,
    value_end: usize,
}

impl<'b, 'l> TryFrom<LineLink<'l>> for LineField<'b>
where
    LineLink<'l>: 'b,
{
    type Error = XRVErr;
    fn try_from(value: LineLink<'b>) -> Result<Self, Self::Error> {
        let mut fields: Vec<Field<'b>> = Vec::new();
        let linename: &str = match std::str::from_utf8(value.name) {
            Err(_) => return Err(XRVErr::CantParseFieldName),
            Ok(s) => s,
        };
        for link in value.links {
            let name: &'b str =
                match std::str::from_utf8(&value.buffer[link.name_start..link.name_end]) {
                    Err(_) => return Err(XRVErr::CantParseFieldStrName),
                    Ok(s) => s,
                };
            let value: &'b str =
                match std::str::from_utf8(&value.buffer[link.value_start..link.value_end]) {
                    Err(_) => return Err(XRVErr::CantParseFieldStrValue),
                    Ok(s) => s,
                };
            fields.push(Field { name, value });
        }

        Ok(Self {
            buffer: value.buffer,
            kind: value.kind,
            name: linename,
            fields,
        })
    }
}

struct Pair {
    start: usize,
    end: usize,
}

const TABLE_ID: u8 = b't';
const STYLE_ID: u8 = b's';
const RECORD_ID: u8 = b'r';

const COLON_CHAR: u8 = b':';
const QUOTE_CHAR: u8 = b'"';
const SPACE_CHAR: u8 = b' ';
const CR_CHAR: u8 = b'\r';
const NL_CHAR: u8 = b'\n';

impl<'b, 'l> TryFrom<&'l [u8]> for LineLink<'b>
where
    &'l [u8]: 'b,
{
    type Error = XRVErr;
    fn try_from(value: &'l [u8]) -> Result<Self, XRVErr> {
        let mut state = ExpectField::Name;
        let mut seek: usize = 0;
        let mut idx: usize = 0;

        let mut pairs: Vec<Pair> = Vec::new();
        for byte in value {
            match state {
                ExpectField::Name => match *byte {
                    COLON_CHAR | QUOTE_CHAR | CR_CHAR | NL_CHAR => {
                        return Err(XRVErr::ExpectSpaceOrAlpha)
                    }
                    SPACE_CHAR => continue,
                    _ => {
                        seek = idx;
                        state = ExpectField::Colon;
                    }
                },
                ExpectField::Colon => match *byte {
                    COLON_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        seek = idx + 1;
                        state = ExpectField::Value;
                    }
                    QUOTE_CHAR => return Err(XRVErr::NameMustNotContainQoutes),
                    SPACE_CHAR | CR_CHAR | NL_CHAR => return Err(XRVErr::NameMustFolowedByColon),
                    _ => continue,
                },
                ExpectField::Value => match *byte {
                    COLON_CHAR | SPACE_CHAR | CR_CHAR | NL_CHAR => return Err(XRVErr::ExpectAlpha),
                    QUOTE_CHAR => {
                        seek += 1;
                        state = ExpectField::Qoute;
                    }
                    _ => state = ExpectField::Skip,
                },
                ExpectField::Skip => match *byte {
                    COLON_CHAR | QUOTE_CHAR => return Err(XRVErr::ExpectingSpaceOrNewline),
                    SPACE_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        seek = idx;
                        state = ExpectField::Name;
                    }
                    CR_CHAR | NL_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        break;
                    }
                    _ => continue,
                },
                ExpectField::Qoute => match *byte {
                    COLON_CHAR | SPACE_CHAR => continue,
                    CR_CHAR | NL_CHAR => return Err(XRVErr::ExpectingQouteNotNewline),
                    QUOTE_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        seek = idx + 1;
                        state = ExpectField::Skip;
                    }
                    _ => continue,
                },
            };
            idx += 1;
        }

        let mut links: Vec<Link> = Vec::new();
        let mut pairs_it = pairs.into_iter();

        let kind: LineKind = match pairs_it.next() {
            None => return Err(XRVErr::FailToGetLineKind),
            Some(k) => match value[k.start..k.end] {
                [b't'] => LineKind::Table,
                [b's'] => LineKind::Style,
                [b'r'] => LineKind::Record,
                _ => return Err(XRVErr::UnkwnownLineKind),
            },
        };

        let name: &'b [u8] = match pairs_it.next() {
            None => return Err(XRVErr::FailToGetLineName),
            Some(n) => &value[n.start..n.end],
        };

        loop {
            match pairs_it.next() {
                None => break,
                Some(name) => match pairs_it.next() {
                    None => return Err(XRVErr::FailedToConsumePairs),
                    Some(value) => links.push(Link {
                        name_start: name.start,
                        name_end: name.end,
                        value_start: value.start,
                        value_end: value.end,
                    }),
                },
            }
        }

        Ok(Self {
            buffer: &value,
            kind,
            name: &name,
            links,
        })
    }
}

impl<'b> TryInto<usize> for Field<'b> {
    type Error = XRVErr;
    fn try_into(self) -> Result<usize, Self::Error> {
        match self.value.parse::<usize>() {
            Err(_) => Err(XRVErr::CantParseFieldUsizeValue),
            Ok(u) => Ok(u),
        }
    }
}

#[derive(Debug)]
struct TableLine<'b> {
    id: &'b str,
    name: &'b str,
    pos: usize,
    len: usize,
    cols: Vec<Field<'b>>,
}

impl<'b> TryFrom<LineField<'b>> for TableLine<'b> {
    type Error = XRVErr;
    fn try_from(value: LineField<'b>) -> Result<Self, Self::Error> {
        match value.kind {
            LineKind::Table => {
                let id: &'b str = value.fields[0].value;
                let name: &'b str = match value.fields[1].name {
                    "name" => value.fields[1].value,
                    _ => return Err(XRVErr::FirstTableFieldMustBeName),
                };

                let pos: usize = match value.fields[2].name {
                    "pos" => value.fields[2].clone().try_into()?,
                    _ => return Err(XRVErr::SecondTableFieldMustBePos),
                };
                let len: usize = match value.fields[3].name {
                    "len" => value.fields[3].clone().try_into()?,
                    _ => return Err(XRVErr::ThirdTableFieldMustBeLen),
                };

                let mut cols: Vec<Field<'b>> = value.fields[4..].to_owned();

                Ok(TableLine {
                    id,
                    name,
                    pos,
                    len,
                    cols,
                })
            }
            _ => return Err(XRVErr::NotTableLine),
        }
    }
}

struct StyleLine<'b> {
    id: &'b str,
    cols: Vec<Field<'b>>,
}

impl<'b> TryFrom<LineField<'b>> for StyleLine<'b> {
    type Error = XRVErr;
    fn try_from(value: LineField<'b>) -> Result<Self, Self::Error> {
        match value.kind {
            LineKind::Style => {
                let id: &'b str = value.fields[0].value;
                let mut cols: Vec<Field<'b>> = Vec::new();
                for col in value.fields[1..].iter() {
                    cols.push(col.clone());
                }

                Ok(StyleLine { id, cols })
            }
            _ => return Err(XRVErr::NotStyleLine),
        }
    }
}

struct RecordLine<'b> {
    id: &'b str,
    cols: Vec<Field<'b>>,
}

impl<'b> TryFrom<LineField<'b>> for RecordLine<'b> {
    type Error = XRVErr;
    fn try_from(value: LineField<'b>) -> Result<Self, Self::Error> {
        match value.kind {
            LineKind::Record => {
                let id: &'b str = value.fields[0].value;
                let mut cols: Vec<Field<'b>> = Vec::new();
                for col in value.fields[1..].iter() {
                    cols.push(col.clone());
                }

                Ok(RecordLine { id, cols })
            }
            _ => Err(XRVErr::NotRecordLine),
        }
    }
}

#[derive(Debug)]
pub struct Reader<'b> {
    buffer: Vec<u8>,
    pub line: usize,
    pub seek: usize,
    pub styles: HashMap<Vec<u8>, Vec<TableLine<'b>>>,
    pub tables: HashMap<Vec<u8>, Vec<TableLine<'b>>>,
}

impl<'b> Reader<'b> {
    pub fn new(path: String) -> Result<Reader<'b>, XRVErr> {
        match File::open(path) {
            Err(err) => Err(XRVErr::FailToOpenFile(err)),
            Ok(file) => Ok(Reader {
                buffer: Reader::buffer(path)?,
                line: 0,
                seek: 0,
                styles: HashMap::new(),
                tables: HashMap::new(),
            }),
        }
    }

    fn buffer(path: String) -> Result<Vec<u8>, XRVErr> {
        match File::open(path) {
            Err(err) => Err(XRVErr::FailToOpenFile(err)),
            Ok(mut file) => {
                let capacity: usize = file.metadata().unwrap().len() as usize;
                let mut buffer: Vec<u8> = Vec::with_capacity(capacity);
                file.read_to_end(&mut buffer);
                Ok(buffer)
            }
        }
    }

    pub fn next(&mut self) -> Result<LineLink<'b>, XRVErr> {
        let mut buffer: &'b mut Vec<u8>;
        buffer = &mut Vec::new();
        // let start = match self.buffer.stream_position() {
        //     Err(err) => return Err(XRVErr::FailToGetStreamPosition(err)),
        //     Ok(pos) => pos,
        // };
        self.buffer.buffer()
        match self.buffer.read_until(NL_CHAR, &mut buffer) {
            Err(err) => return Err(XRVErr::FailToReadUntil(err)),
            Ok(len) => match len {
                0 => Err(XRVErr::ZeroLine(self.line)),
                _ => {
                    self.line += 1;
                    let some: LineLink<'b> = buf.to_vec().as_slice().try_into()?;
                    return Ok(some);
                }
            },
        }
    }
}

#[derive(Debug)]
pub enum XRVErr {
    FailToOpenFile(std::io::Error),
    FailToReadUntil(std::io::Error),
    FailToEnumerateByte(usize),

    FieldNameFailedToParse(usize, usize),
    FieldValueFailedToParse(usize, usize),
    NextFieldFailedToParse(usize, usize),
    FieldBracketFailedToParse(usize, usize),

    ZeroLine(usize),

    FailedToConsumeRefs(usize, usize),

    FailToGetStreamPosition(std::io::Error),

    NotExpectingNewline(usize, usize),
    ExpectEndNotMidOrStart(usize, usize),
    ExpectColon(usize, usize),
    ExpectSpace(usize, usize),

    FailToGetLinkRight(usize, usize),

    TableNameMustBeInBrackets(usize, usize),
    SecondFieldLeftMustBeName(usize, usize),
    FailToGetTableName(usize, usize),
    FailGetStrFrombuffer(usize, usize),
    FailGetUsizeFromStr(usize, usize),
    ThirdFieldLeftMustBePos(usize, usize),
    FailToGetTablePos(usize, usize),
    ForthFieldLeftMustBeLen(usize, usize),
    FailToGetTableLen(usize, usize),
    NameMustFolowedByColon,
    NameMustNotContainQoutes,
    ExpectSpaceOrAlpha,
    ExpectAlpha,
    ExpectingSpaceOrNewline,
    ExpectingQouteNotNewline,
    FailedToConsumePairs,
    FailToGetLineKind,
    FailToGetLineName,
    NotTableLine,
    CantParseFieldUsizeValue,
    CantGetFieldUsizeValue,
    CantParseFieldStrName,
    CantParseFieldStrValue,
    CantParseFieldName,
    FirstTableFieldMustBeName,
    SecondTableFieldMustBePos,
    ThirdTableFieldMustBeLen,
    FillBufNotAvailable,
    NotStyleLine,
    NotRecordLine,
    UnkwnownLineKind,
}
