use std::{iter::Peekable, str};

#[derive(Debug, PartialEq, Eq, Hash)]
enum Quotes {
    Single,
    Double,
    None,
}

#[derive(Debug)]
struct EofMatcher {
    eof:       Vec<u8>,
    complete:  bool,
    match_idx: usize,
}

impl EofMatcher {
    fn new() -> Self { EofMatcher { eof: Vec::with_capacity(10), complete: false, match_idx: 0 } }

    #[inline]
    fn next(&mut self, c: u8) -> bool {
        if self.complete && self.eof.get(self.match_idx) == Some(&c) {
            self.match_idx += 1;
        } else if self.complete {
            self.match_idx = 0;
        } else if c == b'\n' {
            self.complete = true;
        } else {
            self.eof.push(c);
        }
        self.complete && self.match_idx == self.eof.len()
    }
}

/// Serves as a buffer for storing a string until that string can be terminated.
///
/// # Examples
///
/// This example comes from the shell's REPL, which ensures that the user's input
/// will only be submitted for execution once a terminated command is supplied.
#[derive(Debug)]
pub struct Terminator<I: Iterator<Item = u8>> {
    inner:      RearPeekable<I>,
    eof:        Option<EofMatcher>,
    array:      usize,
    read:       usize,
    skip_next:  bool,
    quotes:     Quotes,
    terminated: bool,
}

impl<'a> From<&'a str> for Terminator<std::str::Bytes<'a>> {
    fn from(string: &'a str) -> Self { Terminator::new(string.bytes()) }
}

#[derive(Clone, Debug)]
pub struct RearPeekable<I: Iterator> {
    iter: Peekable<I>,
    now:  Option<I::Item>,
    last: Option<I::Item>,
}

impl<I> Iterator for RearPeekable<I>
where
    I: Iterator,
    I::Item: Copy,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        self.last = self.now;
        self.now = self.iter.next();
        self.now
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<I: Iterator> RearPeekable<I> {
    #[inline]
    pub fn peek(&mut self) -> Option<&I::Item> { self.iter.peek() }

    #[inline]
    pub fn prev(&self) -> Option<&I::Item> { self.last.as_ref() }
}

impl<I: Iterator<Item = u8>> Iterator for Terminator<I> {
    type Item = u8;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.terminated {
            return None;
        }

        let out = self
            .inner
            .next()
            .and_then(|character| {
                if let Some(matcher) = self.eof.as_mut() {
                    if matcher.next(character) {
                        self.eof = None;
                    }
                } else if self.skip_next {
                    self.skip_next = false;
                } else if self.quotes != Quotes::None && character != b'\\' {
                    match (character, &self.quotes) {
                        (b'\'', Quotes::Single) | (b'"', Quotes::Double) => {
                            self.quotes = Quotes::None;
                        }
                        _ => (),
                    }
                } else {
                    match character {
                        b'\'' => {
                            self.quotes = Quotes::Single;
                        }
                        b'"' => {
                            self.quotes = Quotes::Double;
                        }
                        b'<' if self.inner.prev() == Some(&b'<') => {
                            if let Some(&b'<') = self.inner.peek() {
                                self.skip_next = true; // avoid falling in the else at the next pass
                            } else {
                                self.eof = Some(EofMatcher::new());
                            }
                        }
                        b'[' => {
                            self.array += 1;
                        }
                        b']' => {
                            if self.array > 0 {
                                self.array -= 1;
                            }
                        }
                        b'#' if self
                            .inner
                            .prev()
                            .filter(|&c| ![b' ', b'\n'].contains(c))
                            .is_none() =>
                        {
                            return self.inner.find(|&c| c == b'\n')
                        }
                        b'\\' => {
                            if self.inner.peek() == Some(&b'\n') {
                                return self.inner.find(|&c| !(c as char).is_whitespace());
                            } else {
                                self.skip_next = true;
                            }
                        }
                        _ => {}
                    }
                }

                Some(character)
            })
            .map(|c| if c == b'\n' && self.array > 0 { b' ' } else { c });

        if let Some(character) = &out {
            let prev = self.inner.prev().cloned();
            let next = self.inner.peek();
            // println!("debug: \n\tnext: {:?}\n\tarray: {}\n\tquotes: {:?}\n\tcharacter:
            // {:?}\n\tprev: {:?}\n\ttrim: {}", next, self.array, self.quotes, character as char,
            // prev, self.trim);
            if (next == Some(&b'\n') || next == None)
                && self.eof.is_none()
                && self.array == 0
                && self.quotes == Quotes::None
                && (![b'|', b'&'].contains(&character) || prev.filter(|c| c == character).is_none())
            {
                self.terminated = true;
            }
        }

        out
    }
}

impl<I: Iterator<Item = u8>> Terminator<I> {
    /// Consumes lines until a statement is formed or the iterator runs dry, and returns the
    /// underlying `String`.
    pub fn terminate(self) -> Result<String, ()> {
        let stmt = self.collect::<Vec<_>>();
        let stmt = unsafe { String::from_utf8_unchecked(stmt) };
        // println!("statement {:?}", stmt);
        Ok(stmt)
    }

    /// Appends a string to the internal buffer.
    // fn append(&mut self, input: &str) {
    // if self.eof.as_ref().filter(|s| s.as_str() == input.trim()).is_some() {
    //     self.eof = None;
    //}

    pub fn new(inner: I) -> Terminator<I> {
        Terminator {
            inner:      RearPeekable { iter: inner.peekable(), now: None, last: None },
            eof:        None,
            array:      0,
            read:       0,
            skip_next:  false,
            quotes:     Quotes::None,
            terminated: false,
        }
    }
}
