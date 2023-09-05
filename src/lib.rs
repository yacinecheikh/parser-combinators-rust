use std::ops::Deref;
use crate::Result::*;

// parsing types
// the [derive] is to check equality in tests
#[derive(Eq, PartialEq, Debug)]
enum Result<T> {
    Fail,
    Success(usize, T),
}

/*
Parse trait: create() -> Parser; parse()
Parser type: clone(); parse()
*/

trait Parse<T> {
    fn create(&self) -> Parser<T>; // create a Box<dyn Parse> trait object
    fn parse(&self, position: usize, source: &[u8]) -> Result<T>;
}

// Sync is for static definitions (thread-safety)
type Parser<T> = Box<dyn Parse<T> + Sync>;

impl<T> Parse<T> for Parser<T> {
    // create() is not strictly required (clone is used already)
    // the trait name is reused because i'm lazy
    fn create(&self) -> Parser<T> {
        todo!()
    }

    fn parse(&self, position: usize, source: &[u8]) -> Result<T> {
        self.deref().parse(position, source)
    }
}

impl<T> Clone for Parser<T> {
    fn clone(&self) -> Self {
        self.deref().create()
    }
}



// base parser

struct CharParser {}


impl Parse<u8> for CharParser {
    fn create(&self) -> Parser<u8> {
        Box::new(CharParser{})
    }

    fn parse(&self, position: usize, source: &[u8]) -> Result<u8> {
        if position < source.len() {
            Success(position + 1, source[position])
        } else {
            Fail
        }
    }
}

fn readchar() -> Parser<u8> {
    CharParser{}.create()
}


// parser combinators

struct AndParser<T> {
    parsers: Vec<Parser<T>>
}

impl<T: 'static> Parse<Vec<T>> for AndParser<T> {
    fn create(&self) -> Parser<Vec<T>> {
        //let parsers = self.parsers.clone();
        Box::new(AndParser { parsers: self.parsers.clone() })
    }

    fn parse(&self, position: usize, source: &[u8]) -> Result<Vec<T>> {
        let mut cursor = position;
        let mut parsed = Vec::new();
        for p in &self.parsers {
            let r = p.parse(cursor, source);
            match r {
                Fail => {
                    return Fail
                }
                Success(pos, data) => {
                    parsed.push(data);
                    cursor = pos;
                }
            }
        }
        Success(cursor, parsed)
    }
}

fn concat<T: 'static>(parsers: Vec<Parser<T>>) -> Parser<Vec<T>> {
    AndParser { parsers }.create()
}


struct OrParser<T> {
    parsers: Vec<Parser<T>>
}

impl<T: 'static> Parse<T> for OrParser<T> {
    fn create(&self) -> Parser<T> {
        Box::new(OrParser { parsers: self.parsers.clone() })
    }

    fn parse(&self, position: usize, source: &[u8]) -> Result<T> {
        for p in &self.parsers {
            match p.parse(position, source) {
                Fail => (),
                Success(pos, data) => return Success(pos, data)
            }
        }
        Fail
    }
}

fn oneof<T: 'static>(parsers: Vec<Parser<T>>) -> Parser<T> {
    OrParser {parsers}.create()
}

// only accept results that are matched by the filter function
struct FilterParser<T> {
    parser: Parser<T>,
    filter: fn(&T) -> bool
}

impl<T: 'static> Parse<T> for FilterParser<T> {
    fn create(&self) -> Parser<T> {
        Box::new(FilterParser{parser: self.parser.clone(), filter: self.filter.clone() })
    }

    fn parse(&self, position: usize, source: &[u8]) -> Result<T> {
        match self.parser.parse(position, source) {
            Fail => {
                Fail
            }
            Success(position, data) => {
                if (self.filter)(&data) {
                    Success(position, data)
                } else {
                    Fail
                }
            }
        }
    }
}

fn require<T: 'static>(f: fn(&T) -> bool, p: Parser<T>) -> Parser<T> {
    FilterParser { parser: p, filter: f }.create()
}


// apply a function to the result of a successful parsing
struct MapParser<T, U> {
    parser: Parser<T>,
    f: fn(T) -> U
}

impl<T: 'static, U: 'static> Parse<U> for MapParser<T, U> {
    fn create(&self) -> Parser<U> {
        Box::new(MapParser { parser: self.parser.clone(), f: self.f })
    }

    fn parse(&self, position: usize, source: &[u8]) -> Result<U> {
        let result = self.parser.parse(position, source);
        match result {
            Fail => {
                Fail
            }
            Success(position, data) => {
                Success(position, (self.f)(data))
            }
        }
    }
}

fn process<T: 'static, U: 'static>(f: fn(T) -> U, parser: Parser<T>) -> Parser<U> {
    MapParser { parser, f }.create()
}

// make a parser able to repeat as much as possible
struct StarParser<T> {
    parser: Parser<T>
}

impl<T: 'static> Parse<Vec<T>> for StarParser<T> {
    fn create(&self) -> Parser<Vec<T>> {
        Box::new(StarParser {parser: self.parser.clone()})
    }

    fn parse(&self, position: usize, source: &[u8]) -> Result<Vec<T>> {
        let mut cursor = position;
        let mut results = Vec::new();
        loop {
            match self.parser.parse(cursor, source) {
                Fail => {
                    break
                }
                Success(position, data) => {
                    results.push(data);
                    cursor = position;
                }
            }
        }
        // star() always succeeds, even if nothing is parsed
        Success(cursor, results)
    }
}

fn star<T: 'static>(parser: Parser<T>) -> Parser<Vec<T>> {
    StarParser {parser}.create()
}

// TODO: additional combinators (chain, const, many, tag,...)
// these ones do not need any more struct/trait implementation
// (they are just shortcuts to quickly implement parsers)



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starred() {
        let p = readchar();
        let p = star(p);
        let result = p.parse(0, "test".as_bytes());
        assert!(matches!(result, Success(4, _)));
        if let Success(_position, chars) = result {
            let str = String::from_utf8(chars).unwrap();
            assert_eq!(str, "test");
        }

        // star combined with mapped
        let p = process(|chars| String::from_utf8(chars).unwrap(), p);
        let result = p.parse(0, "test".as_bytes());
        assert!(matches!(result, Success(4, _)));
        if let Success(4, s) = result {
            assert_eq!(s, "test");
        }
    }

    #[test]
    fn mapped() {
        let string = process(|c| { String::from_utf8(vec![c]).unwrap() }, readchar());
        let result = string.parse(0, "test".as_bytes());
        assert!(matches!(result, Success(1, _)));
        if let Success(_, s) = result {
            assert_eq!(s, "t");
        }
    }

    #[test]
    fn filtered() {
        let p = readchar();
        let f: fn(&u8) -> bool = |c| { *c == 't' as u8};
        let p = require(f, p);

        let result = p.parse(0, "test".as_bytes());
        assert!(matches!(result, Success(1, _)));
        if let Success(1, ch) = result {
            assert_eq!(ch, 't' as u8)
        }

        let p = require(| c | { *c == 'x' as u8}, readchar());
        let result = p.parse(0, "test".as_bytes());
        assert!(matches!(result, Fail));
    }

    #[test]
    fn or() {
        let p = oneof(vec![readchar(), readchar()]);
        let result = p.parse(0, "test".as_bytes());
        assert!(matches!(result, Success(1, _)));
        if let Success(1, ch) = result {
            assert_eq!(ch, 't' as u8)
        }
    }

    #[test]
    fn and() {
        // just clone all parsers
        // parsers are read-only once created, but it's not like they're expansive to clone anyway
        // (and i'm a rust beginner)
        let p = concat(vec![
            readchar(),
            readchar(),
            readchar(),
            readchar()
        ]);

        // parse all the characters
        let result = p.parse(0, "test".as_bytes());
        assert!(matches!(result, Success(4, _)));
        if let Success(4, chars) = result {
            assert_eq!("test", String::from_utf8(chars).unwrap());
        }

        // not enough characters -> Fail to parse
        let result = p.parse(0, "tes".as_bytes());
        assert_eq!(result, Fail)
    }

    #[test]
    fn char() {
        let result = readchar().parse(0, "test".as_bytes());
        assert_eq!(result, Success(1, "t".as_bytes()[0]));
    }
}
