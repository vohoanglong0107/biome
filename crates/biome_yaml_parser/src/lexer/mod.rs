use biome_parser::{
    diagnostic::ParseDiagnostic,
    lexer::{Lexer, LexerCheckpoint},
};
use biome_rowan::{TextLen, TextRange, TextSize};
use biome_unicode_table::{Dispatch::WHS, lookup_byte};
use biome_yaml_syntax::{T, YamlSyntaxKind, YamlSyntaxKind::*};
mod tests;

pub(crate) struct YamlLexer<'src> {
    /// Source text
    source: &'src str,

    current_coordinate: TextCoordinate,

    /// diagnostics emitted during the parsing phase
    diagnostics: Vec<ParseDiagnostic>,

    scopes: Vec<LexScope>,

    tokens: Vec<LexToken>,

    token_index: usize,
}

impl<'src> YamlLexer<'src> {
    pub fn from_str(source: &'src str) -> Self {
        Self {
            source,
            diagnostics: Vec::new(),
            scopes: Default::default(),
            current_coordinate: Default::default(),
            tokens: vec![LexToken::default()],
            token_index: 0,
        }
    }

    /// Push a token
    fn consume_tokens(&mut self) -> Vec<LexToken> {
        let Some(current) = self.current_byte() else {
            let mut tokens = Vec::new();
            while let Some(scope) = self.scopes.pop() {
                if let Some(token) = scope.close_token() {
                    tokens.push(LexToken {
                        kind: token,
                        start: self.current_coordinate,
                        end: self.current_coordinate,
                    });
                }
            }
            tokens.push(LexToken {
                kind: EOF,
                start: self.current_coordinate,
                end: self.current_coordinate,
            });
            return tokens;
        };
        let start = self.text_position();

        let tokens = match current {
            _ if self.is_first_plain_char(current) => self.consume_plain_literal(),
            b':' => vec![self.consume_byte(T![:])],
            b',' => vec![self.consume_byte(T![,])],
            b'#' => vec![self.consume_comment()],
            b'\'' => vec![self.consume_single_quoted_literal()],
            b'"' => vec![self.consume_double_quoted_literal()],
            b'[' | b']' | b'{' | b'}' => self.consume_flow_start(current),
            b'?' => self.consume_question(),
            b'-' => self.consume_dash(),
            _ if is_space(current) || is_break(current) => self.consume_blank(),
            _ => vec![self.consume_unexpected_token()],
        };

        debug_assert!(self.text_position() > start, "Lexer did not advance");
        tokens
    }

    /// Bumps the current byte and creates a lexed token of the passed in kind.
    #[inline]
    fn consume_byte(&mut self, tok: YamlSyntaxKind) -> LexToken {
        let start = self.current_coordinate;
        self.advance(1);
        LexToken {
            kind: tok,
            start,
            end: self.current_coordinate,
        }
    }

    fn consume_dash(&mut self) -> Vec<LexToken> {
        self.assert_byte(b'-');
        let mut tokens = Vec::new();
        let start = self.current_coordinate;
        if self.current_scope().should_enter_sequence(start) {
            tokens.push(LexToken {
                kind: SEQUENCE_START,
                start,
                end: start,
            });
            self.enter_scope(LexScope::new_sequence_scope(start));
        }
        self.advance(1);
        tokens.push(LexToken {
            kind: T![-],
            start,
            end: self.current_coordinate,
        });
        tokens
    }

    fn consume_question(&mut self) -> Vec<LexToken> {
        self.assert_byte(b'?');
        let mut tokens = Vec::new();
        let start = self.current_coordinate;
        if self
            .current_scope()
            .should_enter_mapping(self.current_coordinate)
        {
            tokens.push(LexToken {
                kind: MAPPING_START,
                start,
                end: self.current_coordinate,
            });
            self.enter_scope(LexScope::BlockMap(self.current_coordinate.column));
        }
        self.advance(1);
        tokens.push(LexToken {
            kind: T![?],
            start,
            end: self.current_coordinate,
        });
        tokens
    }

    fn consume_comment(&mut self) -> LexToken {
        self.assert_byte(b'#');
        let start = self.current_coordinate;
        while let Some(c) = self.current_byte() {
            if c == b'\n' {
                break;
            }
            self.advance(1);
        }
        LexToken {
            kind: COMMENT,
            start,
            end: self.current_coordinate,
        }
    }

    fn consume_flow_start(&mut self, current: u8) -> Vec<LexToken> {
        debug_assert!(matches!(current, b'[' | b']' | b'{' | b'}'));
        let mut tokens = Vec::new();
        let start = self.current_coordinate;
        if let Some(LexScope::FlowCollection { num_nested, .. }) = self.scopes.last_mut() {
            let token = match current {
                b'[' => {
                    *num_nested += 1;
                    self.consume_byte(T!['['])
                }
                b']' => {
                    *num_nested = num_nested.saturating_sub(1);
                    self.consume_byte(T![']'])
                }
                b'{' => {
                    *num_nested += 1;
                    self.consume_byte(T!['{'])
                }
                b'}' => {
                    *num_nested = num_nested.saturating_sub(1);
                    self.consume_byte(T!['}'])
                }
                _ => self.consume_unexpected_token(),
            };
            return vec![token];
        }

        self.enter_scope(LexScope::new_flow_scope(self.current_scope()));

        loop {
            tokens.extend(self.consume_tokens());
            let LexScope::FlowCollection { num_nested, .. } = self.current_scope() else {
                break;
            };
            if *num_nested == 0 {
                break;
            }
        }

        if self.is_at_mapping_indicator() {
            let Some(scope) = self.scopes.last_mut() else {
                tokens.insert(0, LexToken::new(FLOW_START, start, start));
                tokens.push(LexToken::new(
                    FLOW_END,
                    self.current_coordinate,
                    self.current_coordinate,
                ));
                return tokens;
            };
            *scope = LexScope::new_mapping_scope(self.current_coordinate);
            tokens.insert(0, LexToken::new(MAPPING_START, start, start));
        } else {
            if let LexScope::FlowCollection { .. } = self.current_scope() {
                self.scopes.pop();
            }
            tokens.insert(0, LexToken::new(FLOW_START, start, start));
            tokens.push(LexToken::new(
                FLOW_END,
                self.current_coordinate,
                self.current_coordinate,
            ));
        }
        tokens
    }

    // https://yaml.org/spec/1.2.2/#rule-ns-plain
    // TODO: parse multiline plain scalar at current indentation level
    fn consume_plain_literal(&mut self) -> Vec<LexToken> {
        self.assert_current_char_boundary();
        debug_assert!(
            self.current_byte()
                .is_some_and(|c| self.is_first_plain_char(c))
        );
        let start = self.current_coordinate;
        let mut tokens = Vec::new();
        self.advance_char_unchecked();
        self.consume_single_plain_literal_line();

        if self.current_scope().should_enter_mapping(start) && self.is_at_mapping_indicator() {
            self.enter_scope(LexScope::BlockMap(start.column));

            tokens.push(LexToken::new(MAPPING_START, start, start));
            tokens.push(LexToken::new(PLAIN_LITERAL, start, self.current_coordinate))
        } else if self.current_scope().is_mapping_key(start)
            || matches!(self.current_scope(), LexScope::FlowCollection { .. })
        {
            tokens.push(LexToken::new(PLAIN_LITERAL, start, self.current_coordinate));
        } else {
            tokens.extend([
                LexToken::new(FLOW_START, start, start),
                LexToken::new(PLAIN_LITERAL, start, self.current_coordinate),
                LexToken::new(FLOW_END, self.current_coordinate, self.current_coordinate),
            ]);
        }
        tokens
    }

    fn consume_single_plain_literal_line(&mut self) {
        self.assert_current_char_boundary();
        while let Some(c) = self.current_byte() {
            // https://yaml.org/spec/1.2.2/#rule-ns-plain-char
            if is_plain_safe(c, self.current_scope()) && c != b':' && c != b'#' {
                self.advance_char_unchecked();
            } else if is_non_space_char(c) && self.peek_byte().is_some_and(|c| c == b'#') {
                self.advance_char_unchecked();
                self.advance(1); // '#'
            } else if c == b':'
                && self
                    .peek_byte()
                    .is_some_and(|c| is_plain_safe(c, self.current_scope()))
            {
                self.advance(1); // ':'
                self.advance_char_unchecked();
            }
            // Yes plain token can contain spaces
            // For example:
            // a bc: x yz
            // Is a valid mapping
            else if is_space(c) {
                self.advance(1);
            } else {
                break;
            }
        }
    }

    fn is_at_mapping_indicator(&self) -> bool {
        self.current_byte().is_some_and(|c| c == b':')
            && self.peek_byte().is_none_or(|c| is_break(c) || is_space(c))
    }

    // https://yaml.org/spec/1.2.2/#rule-ns-plain-first
    fn is_first_plain_char(&self, c: u8) -> bool {
        (is_non_space_char(c) && !is_indicator(c))
            || ((c == b'?' || c == b':' || c == b'-')
                && self
                    .peek_byte()
                    .is_some_and(|c| is_plain_safe(c, self.current_scope())))
    }

    // https://yaml.org/spec/1.2.2/#731-double-quoted-style
    fn consume_double_quoted_literal(&mut self) -> LexToken {
        self.assert_byte(b'"');
        let start = self.current_coordinate;
        self.advance(1);

        let kind = loop {
            match self.current_byte() {
                Some(b'\\') => {
                    if matches!(self.peek_byte(), Some(b'"')) {
                        self.advance(2)
                    } else {
                        self.advance(1)
                    }
                }
                Some(b'"') => {
                    self.advance(1);
                    break DOUBLE_QUOTED_LITERAL;
                }
                Some(_) => self.advance(1),
                None => break DOUBLE_QUOTED_LITERAL,
            }
        };
        LexToken {
            kind,
            start,
            end: self.current_coordinate,
        }
    }

    // https://yaml.org/spec/1.2.2/#732-single-quoted-style
    fn consume_single_quoted_literal(&mut self) -> LexToken {
        self.assert_byte(b'\'');
        let start = self.current_coordinate;
        self.advance(1);

        let kind = loop {
            match self.current_byte() {
                Some(b'\'') => {
                    if matches!(self.peek_byte(), Some(b'\'')) {
                        self.advance(2)
                    } else {
                        self.advance(1);
                        break SINGLE_QUOTED_LITERAL;
                    }
                }
                Some(_) => self.advance(1),
                None => break SINGLE_QUOTED_LITERAL,
            }
        };
        LexToken {
            kind,
            start,
            end: self.current_coordinate,
        }
    }

    fn consume_blank(&mut self) -> Vec<LexToken> {
        let mut tokens = Vec::new();
        let mut is_newline = false;
        let start = self.current_coordinate;
        while let Some(current) = self.current_byte() {
            if is_space(current) {
                self.consume_whitespaces();
            } else if is_break(current) {
                self.consume_newline();
                self.current_coordinate.enter_new_line();
                is_newline = true;
            } else {
                break;
            }
        }
        if is_newline {
            tokens.push(LexToken {
                kind: NEWLINE,
                start,
                end: self.current_coordinate,
            });
            for scope in self.eject_exited_scope(self.current_coordinate) {
                if let Some(token) = scope.close_token() {
                    tokens.push(LexToken {
                        kind: token,
                        start: self.current_coordinate,
                        end: self.current_coordinate,
                    });
                }
            }
        } else {
            tokens.push(LexToken {
                kind: WHITESPACE,
                start,
                end: self.current_coordinate,
            });
        }
        tokens
    }

    fn consume_unexpected_token(&mut self) -> LexToken {
        self.assert_current_char_boundary();
        let start = self.current_coordinate;

        let char = self.current_char_unchecked();
        let err = ParseDiagnostic::new(
            format!("unexpected character `{char}`"),
            self.text_position()..self.text_position() + char.text_len(),
        );
        self.diagnostics.push(err);
        self.advance(char.len_utf8());
        LexToken {
            kind: ERROR_TOKEN,
            start,
            end: self.current_coordinate,
        }
    }

    fn current_token(&self) -> LexToken {
        // shouldn't brick the server just because a user open a malformed YAML file
        self.tokens
            .get(self.token_index)
            .copied()
            .unwrap_or(LexToken {
                kind: EOF,
                start: self.current_coordinate,
                end: self.current_coordinate,
            })
    }

    fn current_scope(&self) -> &LexScope {
        self.scopes.last().unwrap_or(&LexScope::Document)
    }

    fn enter_scope(&mut self, scope: LexScope) {
        self.scopes.push(scope);
    }

    fn eject_exited_scope(&mut self, cursor: TextCoordinate) -> Vec<LexScope> {
        let mut ejected = Vec::new();
        while let Some(scope) = self.scopes.pop() {
            if scope.contain(cursor) {
                self.scopes.push(scope);
                break;
            }
            ejected.push(scope);
        }
        ejected
    }
}

impl<'src> Lexer<'src> for YamlLexer<'src> {
    const NEWLINE: Self::Kind = YamlSyntaxKind::NEWLINE;
    const WHITESPACE: Self::Kind = YamlSyntaxKind::WHITESPACE;

    type Kind = YamlSyntaxKind;
    type LexContext = ();
    type ReLexContext = ();

    fn source(&self) -> &'src str {
        self.source
    }

    fn current(&self) -> Self::Kind {
        self.current_token().kind
    }

    fn current_range(&self) -> TextRange {
        self.current_token().text_range()
    }

    #[inline]
    fn advance(&mut self, n: usize) {
        self.current_coordinate.advance(n);
    }

    #[inline]
    fn advance_char_unchecked(&mut self) {
        let c = self.current_char_unchecked();
        self.advance(c.len_utf8());
    }

    #[inline]
    fn current_start(&self) -> TextSize {
        self.current_token().start()
    }

    fn next_token(&mut self, _context: Self::LexContext) -> Self::Kind {
        self.token_index += 1;
        if self.tokens.len() <= self.token_index {
            let tokens = self.consume_tokens();
            self.tokens.extend(tokens);
        }
        self.current_token().kind
    }

    fn has_preceding_line_break(&self) -> bool {
        false
    }

    fn has_unicode_escape(&self) -> bool {
        false
    }

    fn rewind(&mut self, _: LexerCheckpoint<Self::Kind>) {
        unimplemented!()
    }

    fn finish(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    fn push_diagnostic(&mut self, diagnostic: ParseDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    fn position(&self) -> usize {
        self.current_coordinate.offset
    }

    /// Consumes all whitespace until a non-whitespace or a newline is found.
    ///
    /// ## Safety
    /// Must be called at a valid UT8 char boundary
    fn consume_whitespaces(&mut self) {
        self.assert_current_char_boundary();

        while let Some(c) = self.current_byte() {
            let dispatch = lookup_byte(c);
            if !matches!(dispatch, WHS) {
                break;
            }

            if is_space(c) {
                self.advance(1);
            } else if is_break(c) {
                break;
            } else {
                let start = self.text_position();
                self.advance(1);

                self.push_diagnostic(
                    ParseDiagnostic::new(
                        "The YAML standard allows only two types of whitespace characters: tabs and spaces",
                        start..self.text_position(),
                    )
                        .with_hint("Use a regular whitespace character instead. For more detail, please check https://yaml.org/spec/1.2.2/#55-white-space-characters"),
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LexToken {
    start: TextCoordinate,
    end: TextCoordinate,
    kind: YamlSyntaxKind,
}

impl Default for LexToken {
    fn default() -> Self {
        Self {
            kind: TOMBSTONE,
            start: TextCoordinate::default(),
            end: TextCoordinate::default(),
        }
    }
}

impl LexToken {
    fn new(kind: YamlSyntaxKind, start: TextCoordinate, end: TextCoordinate) -> Self {
        Self { kind, start, end }
    }
    fn text_range(&self) -> TextRange {
        TextRange::new(self.start.into(), self.end.into())
    }

    fn start(&self) -> TextSize {
        self.start.into()
    }
}

#[derive(Debug)]
enum LexScope {
    Document,
    BlockSequence(usize),
    BlockMap(usize),
    FlowCollection { border: usize, num_nested: usize },
}

impl LexScope {
    fn new_flow_scope(parent_scope: &LexScope) -> Self {
        match parent_scope {
            Self::Document => Self::FlowCollection {
                border: 0,
                num_nested: 0,
            },
            Self::BlockSequence(border) => Self::FlowCollection {
                border: border + 1,
                num_nested: 0,
            },
            Self::BlockMap(border) => Self::FlowCollection {
                border: border + 1,
                num_nested: 0,
            },
            Self::FlowCollection { border, num_nested } => Self::FlowCollection {
                border: *border,
                num_nested: num_nested + 1,
            },
        }
    }

    fn new_mapping_scope(coordinate: TextCoordinate) -> Self {
        Self::BlockMap(coordinate.column)
    }

    fn new_sequence_scope(coordinate: TextCoordinate) -> Self {
        Self::BlockSequence(coordinate.column)
    }

    fn contain(&self, coordinate: TextCoordinate) -> bool {
        match self {
            Self::Document => true,
            Self::BlockSequence(border) => coordinate.column >= *border,
            Self::BlockMap(border) => coordinate.column >= *border,
            Self::FlowCollection { border, .. } => coordinate.column >= *border,
        }
    }

    fn should_enter_mapping(&self, position: TextCoordinate) -> bool {
        match self {
            Self::Document => true,
            Self::BlockSequence(border) => position.column > *border,
            Self::BlockMap(border) => position.column > *border,
            Self::FlowCollection { .. } => false,
        }
    }

    fn is_mapping_key(&self, position: TextCoordinate) -> bool {
        let LexScope::BlockMap(border) = self else {
            return false;
        };
        *border == position.column
    }

    fn should_enter_sequence(&self, position: TextCoordinate) -> bool {
        match self {
            LexScope::BlockSequence(border) => position.column > *border,
            LexScope::BlockMap(border) => position.column >= *border,
            LexScope::FlowCollection { .. } => false,
            LexScope::Document => true,
        }
    }

    fn close_token(&self) -> Option<YamlSyntaxKind> {
        match self {
            Self::Document => None,
            Self::BlockSequence(_) => Some(SEQUENCE_END),
            Self::BlockMap(_) => Some(MAPPING_END),
            // Depending on whether this collection is a mapping key, the lexer might not want to
            // emit a FLOW_END token.
            Self::FlowCollection { .. } => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct TextCoordinate {
    /// The byte position in the source text.
    offset: usize,
    /// The number of bytes since the last new line.
    column: usize,
}

impl From<TextCoordinate> for TextSize {
    fn from(value: TextCoordinate) -> Self {
        Self::from(value.offset as u32)
    }
}

impl TextCoordinate {
    fn advance(&mut self, n: usize) {
        self.offset += n;
        self.column += n;
    }

    fn enter_new_line(&mut self) {
        self.column = 0;
    }
}

// https://yaml.org/spec/1.2.2/#rule-ns-plain-safe
fn is_plain_safe(c: u8, scope: &LexScope) -> bool {
    match scope {
        LexScope::FlowCollection { .. } => is_non_space_char(c) && !is_flow_indicator(c),
        _ => is_non_space_char(c),
    }
}

// https://yaml.org/spec/1.2.2/#rule-ns-char
fn is_non_space_char(c: u8) -> bool {
    !is_space(c) && !is_break(c)
}

// https://yaml.org/spec/1.2.2/#rule-s-white
fn is_space(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

// https://yaml.org/spec/1.2.2/#rule-b-char
fn is_break(c: u8) -> bool {
    c == b'\n' || c == b'\r'
}

// https://yaml.org/spec/1.2.2/#rule-c-indicator
fn is_indicator(c: u8) -> bool {
    c == b'-'
        || c == b'?'
        || c == b':'
        || c == b'#'
        || c == b'&'
        || c == b'*'
        || c == b'!'
        || c == b'|'
        || c == b'>'
        || c == b'\''
        || c == b'"'
        || c == b'%'
        || c == b'@'
        || c == b'`'
        || is_flow_indicator(c)
}

// https://yaml.org/spec/1.2.2/#rule-c-flow-indicator
fn is_flow_indicator(c: u8) -> bool {
    c == b',' || c == b'[' || c == b']' || c == b'{' || c == b'}'
}
