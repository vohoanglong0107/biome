//! Identify string case and convert to various string cases.

#![deny(clippy::use_self)]

use std::{borrow::Cow, cmp::Ordering, ffi::OsStr};

#[cfg(feature = "biome_rowan")]
pub mod comparable_token;

/// Represents the case of a string.
///
/// Note that some cases are supersets of others.
/// For example, a name in [Case::Lower] is also in [Case::Camel], [Case::Kebab] , and [Case::Snake].
/// Thus [Case::Camel], [Case::Kebab], and [Case::Snake] are superset of [Case::Lower].
/// `Case::Unknown` is a superset of all [Case].
///
/// The relation between cases is depicted in the following diagram.
/// The arrow means "is subset of".
///
/// ```svgbob
///                     ┌──► Pascal ────────────┐
/// NumberableCapital ──┤                       │
///                     └──► Upper ─► Constant ─┤
///                                             ├──► Unknown
///                     ┌──► Camel ─────────────┤
///         ┌──► Lower ─┤                       │
///         │           └──► Kebab ─────────────┤
/// Number ─┤           │                       │
///         │           └──► Snake ─────────────┤
///         │                                   │
///         └──► Uni ───────────────────────────┘
/// ```
///
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(u16)]
pub enum Case {
    /// ASCII numbers
    Number = 1 << 0,
    /// Alphanumeric Characters that cannot be in lowercase or uppercase (numbers and syllabary)
    Uni = Case::Number as u16 | (1 << 1),
    /// A, B1, C42
    NumberableCapital = 1 << 2,
    /// UPPERCASE
    Upper = Case::NumberableCapital as u16 | (1 << 3),
    // CONSTANT_CASE
    Constant = Case::Upper as u16 | (1 << 4),
    /// PascalCase
    Pascal = Case::NumberableCapital as u16 | (1 << 5),
    /// lowercase
    Lower = Case::Number as u16 | (1 << 6),
    /// snake_case
    Snake = Case::Lower as u16 | (1 << 7),
    /// kebab-case
    Kebab = Case::Lower as u16 | (1 << 8),
    // camelCase
    Camel = Case::Lower as u16 | (1 << 9),
    /// Unknown case
    #[default]
    Unknown = Case::Camel as u16
        | Case::Kebab as u16
        | Case::Snake as u16
        | Case::Pascal as u16
        | Case::Constant as u16
        | Case::Uni as u16
        | (1 << 10),
}

impl Case {
    /// Returns the [Case] of `value`.
    ///
    /// If `strict` is `true`, then two consecutive uppercase characters are not
    /// allowed in camelCase and PascalCase.
    /// For instance, `HTTPServer` is not considered in _PascalCase_ when `strict` is `true`.
    ///
    /// A figure is considered both uppercase and lowercase.
    /// Thus, `V8_ENGINE` is in _CONSTANt_CASE_ and `V8Engine` is in _PascalCase_.
    ///
    /// ```
    /// use biome_string_case::Case;
    ///
    /// assert_eq!(Case::identify("aHttpServer", /* no effect */ true), Case::Camel);
    /// assert_eq!(Case::identify("aHTTPServer", true), Case::Unknown);
    /// assert_eq!(Case::identify("aHTTPServer", false), Case::Camel);
    /// assert_eq!(Case::identify("v8Engine", /* no effect */ true), Case::Camel);
    ///
    /// assert_eq!(Case::identify("HTTP_SERVER", /* no effect */ true), Case::Constant);
    /// assert_eq!(Case::identify("V8_ENGINE", /* no effect */ true), Case::Constant);
    ///
    /// assert_eq!(Case::identify("http-server", /* no effect */ true), Case::Kebab);
    ///
    /// assert_eq!(Case::identify("httpserver", /* no effect */ true), Case::Lower);
    ///
    /// assert_eq!(Case::identify("T", /* no effect */ true), Case::NumberableCapital);
    /// assert_eq!(Case::identify("T1", /* no effect */ true), Case::NumberableCapital);
    ///
    /// assert_eq!(Case::identify("HttpServer", /* no effect */ true), Case::Pascal);
    /// assert_eq!(Case::identify("HTTPServer", true), Case::Unknown);
    /// assert_eq!(Case::identify("HTTPServer", false), Case::Pascal);
    /// assert_eq!(Case::identify("V8Engine", /* no effect */ true), Case::Pascal);
    ///
    /// assert_eq!(Case::identify("http_server", /* no effect */ true), Case::Snake);
    ///
    /// assert_eq!(Case::identify("HTTPSERVER", /* no effect */ true), Case::Upper);
    ///
    /// assert_eq!(Case::identify("100", /* no effect */ true), Case::Number);
    /// assert_eq!(Case::identify("안녕하세요", /* no effect */ true), Case::Uni);
    ///
    /// assert_eq!(Case::identify("", /* no effect */ true), Case::Unknown);
    /// assert_eq!(Case::identify("_", /* no effect */ true), Case::Unknown);
    /// assert_eq!(Case::identify("안녕하세요abc", /* no effect */ true), Case::Unknown);
    /// ```
    pub fn identify(value: &str, strict: bool) -> Self {
        let mut chars = value.chars();
        let Some(first_char) = chars.next() else {
            return Self::Unknown;
        };
        let mut result = if first_char.is_lowercase() {
            Self::Lower
        } else if first_char.is_uppercase() {
            Self::NumberableCapital
        } else if first_char.is_ascii_digit() {
            Self::Number
        } else if first_char.is_alphanumeric() {
            Self::Uni
        } else {
            return Self::Unknown;
        };
        let mut previous_char = first_char;
        let mut has_consecutive_uppercase = false;
        for current_char in chars {
            result = match current_char {
                '-' => match result {
                    Self::Kebab | Self::Lower | Self::Number if previous_char != '-' => Self::Kebab,
                    _ => return Self::Unknown,
                },
                '_' => match result {
                    Self::Constant | Self::Snake if previous_char != '_' => result,
                    Self::NumberableCapital | Self::Upper => Self::Constant,
                    Self::Lower | Self::Number => Self::Snake,
                    _ => return Self::Unknown,
                },
                _ if current_char.is_uppercase() => {
                    has_consecutive_uppercase |= previous_char.is_uppercase();
                    match result {
                        Self::Camel | Self::Pascal if strict && has_consecutive_uppercase => {
                            return Self::Unknown;
                        }
                        Self::Camel | Self::Constant | Self::Pascal => result,
                        Self::Lower | Self::Number => Self::Camel,
                        Self::NumberableCapital | Self::Upper => Self::Upper,
                        _ => return Self::Unknown,
                    }
                }
                _ if current_char.is_lowercase() => match result {
                    Self::Number => Self::Lower,
                    Self::Camel | Self::Kebab | Self::Lower | Self::Snake => result,
                    Self::Pascal | Self::NumberableCapital => Self::Pascal,
                    Self::Upper if !strict || !has_consecutive_uppercase => Self::Pascal,
                    _ => return Self::Unknown,
                },
                '0'..='9' => result,
                _ if current_char.is_alphanumeric() => match result {
                    Self::Number | Self::Uni => Self::Uni,
                    _ => return Self::Unknown,
                },
                _ => return Self::Unknown,
            };
            previous_char = current_char;
        }
        // The last char cannot be a delimiter
        if matches!(previous_char, '-' | '_') {
            return Self::Unknown;
        }
        result
    }

    /// Convert `value` to the `self` [Case].
    ///
    /// ```
    /// use biome_string_case::Case;
    ///
    /// assert_eq!(Case::Camel.convert("Http_SERVER"), "httpServer");
    /// assert_eq!(Case::Camel.convert("v8-Engine"), "v8Engine");
    ///
    /// assert_eq!(Case::Constant.convert("HttpServer"), "HTTP_SERVER");
    /// assert_eq!(Case::Constant.convert("v8-Engine"), "V8_ENGINE");
    ///
    /// assert_eq!(Case::Kebab.convert("Http_SERVER"), "http-server");
    /// assert_eq!(Case::Kebab.convert("v8Engine"), "v8-engine");
    ///
    /// assert_eq!(Case::Lower.convert("Http_SERVER"), "httpserver");
    ///
    /// assert_eq!(Case::NumberableCapital.convert("LONG"), "L");
    ///
    /// assert_eq!(Case::Pascal.convert("http_SERVER"), "HttpServer");
    ///
    /// assert_eq!(Case::Snake.convert("HttpServer"), "http_server");
    ///
    /// assert_eq!(Case::Upper.convert("Http_SERVER"), "HTTPSERVER");
    /// ```
    pub fn convert(self, value: &str) -> String {
        if value.is_empty() || matches!(self, Self::Unknown | Self::Number) {
            return value.to_string();
        }
        let mut word_separator = matches!(self, Self::Pascal);
        let mut output = String::with_capacity(value.len());
        for ((i, current), next) in value
            .char_indices()
            .zip(value.chars().skip(1).map(Some).chain(Some(None)))
        {
            if !current.is_alphanumeric()
                || (matches!(self, Self::Uni) && (current.is_lowercase() || current.is_uppercase()))
            {
                word_separator = true;
                continue;
            }
            if let Some(next) = next {
                if i != 0 && current.is_uppercase() && next.is_lowercase() {
                    word_separator = true;
                }
            }
            if word_separator {
                match self {
                    Self::Camel
                    | Self::Lower
                    | Self::Number
                    | Self::NumberableCapital
                    | Self::Pascal
                    | Self::Unknown
                    | Self::Uni
                    | Self::Upper => (),
                    Self::Constant | Self::Snake => {
                        output.push('_');
                    }
                    Self::Kebab => {
                        output.push('-');
                    }
                }
            }
            match self {
                Self::Camel | Self::Pascal => {
                    if word_separator {
                        output.extend(current.to_uppercase())
                    } else {
                        output.extend(current.to_lowercase())
                    }
                }
                Self::Constant | Self::Upper => output.extend(current.to_uppercase()),
                Self::NumberableCapital => {
                    if i == 0 {
                        output.extend(current.to_uppercase());
                    }
                }
                Self::Kebab | Self::Snake | Self::Lower => output.extend(current.to_lowercase()),
                Self::Uni => output.extend(Some(current)),
                Self::Number | Self::Unknown => (),
            }
            word_separator = false;
            if let Some(next) = next {
                if current.is_lowercase() && next.is_uppercase() {
                    word_separator = true;
                }
            }
        }
        output
    }
}

impl std::fmt::Display for Case {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let repr = match self {
            Self::Camel => "camelCase",
            Self::Constant => "CONSTANT_CASE",
            Self::Kebab => "kebab-case",
            Self::Lower => "lowercase",
            Self::Number => "number case",
            Self::NumberableCapital => "numberable capital case",
            Self::Pascal => "PascalCase",
            Self::Snake => "snake_case",
            Self::Uni => "unicase",
            Self::Unknown => "unknown case",
            Self::Upper => "UPPERCASE",
        };
        write!(f, "{repr}")
    }
}

/// Represents a set of cases.
///
/// An instance of [Cases] supports the binary operators `|` to unionize two sets or add a new [Case].
///
/// Note that some [Case] are already sets of [Case].
/// For example, [Case::Unknown] is a set that includes all [Case].
/// So adding [Case::Unknown] to a [Cases] will supersede all other cases.
///
/// A [Cases] is iterable.
/// A Cases iterator doesn't yield a [Case] that is covered by another [Case] in the set.
/// See [CasesIterator] for more details.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Cases(u16);

impl Cases {
    /// Create an empty set.
    ///
    /// You can also obtain an empty alias using [Cases::default()].
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns `true` if the set is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if all cases of `other` are contained in the current set.
    ///
    /// ```
    /// use biome_string_case::{Cases, Case};
    ///
    /// let camel_or_kebab = (Case::Camel | Case::Kebab);
    ///
    /// assert!(camel_or_kebab.contains(Case::Camel));
    /// assert!(camel_or_kebab.contains(camel_or_kebab));
    /// ```
    pub fn contains(self, other: impl Into<Self>) -> bool {
        let other = other.into();
        self.0 & other.0 == other.0
    }
}

impl IntoIterator for Cases {
    type Item = Case;
    type IntoIter = CasesIterator;
    fn into_iter(self) -> Self::IntoIter {
        CasesIterator { rest: self }
    }
}

impl FromIterator<Case> for Cases {
    fn from_iter<T: IntoIterator<Item = Case>>(iter: T) -> Self {
        iter.into_iter()
            .fold(Self::empty(), |result, case| result | case)
    }
}

impl From<Case> for Cases {
    fn from(value: Case) -> Self {
        Self(value as u16)
    }
}

impl<Rhs: Into<Self>> core::ops::BitOr<Rhs> for Cases {
    type Output = Self;
    fn bitor(self, rhs: Rhs) -> Self::Output {
        Self(self.0 | rhs.into().0)
    }
}
impl core::ops::BitOr for Case {
    type Output = Cases;
    fn bitor(self, rhs: Self) -> Self::Output {
        Cases::from(self) | rhs
    }
}
impl<Rhs: Into<Self>> core::ops::BitOrAssign<Rhs> for Cases {
    fn bitor_assign(&mut self, rhs: Rhs) {
        self.0 |= rhs.into().0;
    }
}

/// An iterator of [Cases].
///
/// The iterator doesn't yield a [Case] that is covered by another [Case] in the iterated set.
/// For example, if the set includes [Case::Constant] and [Case::Upper],
/// the iterator only yields [Case::Constant] because [Case::Constant] covers [Case::Upper].
///
/// ```
/// use biome_string_case::{Cases, Case};
///
/// let cases = Case::Camel | Case::Kebab;
/// assert_eq!(cases.into_iter().collect::<Vec<_>>().as_slice(), &[Case::Camel, Case::Kebab]);
///
/// let cases = Case::Camel | Case::Kebab | Case::Lower;
/// assert_eq!(cases.into_iter().collect::<Vec<_>>().as_slice(), &[Case::Camel, Case::Kebab]);
/// ```
#[derive(Clone, Debug)]
pub struct CasesIterator {
    rest: Cases,
}
impl Iterator for CasesIterator {
    type Item = Case;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            None
        } else {
            let leading_bit_index = 15u16 - (self.rest.0.leading_zeros() as u16);
            let case = LEADING_BIT_INDEX_TO_CASE[leading_bit_index as usize];
            self.rest.0 &= !(case as u16);
            Some(case)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(6))
    }
}
impl core::iter::FusedIterator for CasesIterator {}

const LEADING_BIT_INDEX_TO_CASE: [Case; 11] = [
    Case::Number,
    Case::Uni,
    Case::NumberableCapital,
    Case::Upper,
    Case::Constant,
    Case::Pascal,
    Case::Lower,
    Case::Snake,
    Case::Kebab,
    Case::Camel,
    Case::Unknown,
];

/// A collator defines an order between a set of characters.
///
/// This order may differ from their binary order.
/// This is often used to provide a more natural order for humans than the binary order represents.
pub trait Collator {
    type Char: PartialEq;

    /// Returns the weight of the character `c`.
    ///
    /// A character with a smaller weight than another one, is placed before in the collation order.
    fn weight(&self, c: &Self::Char) -> impl Ord;

    /// Returns the digit value if `c` is a numeric character.
    ///
    /// This allows the collator to compare numbers in a human way (e.g. `9` < `10`).
    fn as_digit(&self, c: &Self::Char) -> Option<impl Ord>;

    /// Returns an [Ordering] between `iter1` and `iter2`.
    fn cmp(
        &self,
        iter1: impl IntoIterator<Item = Self::Char>,
        iter2: impl IntoIterator<Item = Self::Char>,
    ) -> Ordering {
        let mut iter1 = iter1.into_iter();
        let mut iter2 = iter2.into_iter();
        let mut prev = None;
        loop {
            match (iter1.next(), iter2.next()) {
                (None, None) => {
                    // All characters of `iter1` and `iter2` are equal.
                    return Ordering::Equal;
                }
                (None, Some(_)) => {
                    // `iter1` is a prefix of `iter2`.
                    return Ordering::Less;
                }
                (Some(_), None) => {
                    // `iter2` is a prefix of `iter1`.
                    return Ordering::Greater;
                }
                (Some(c1), Some(c2)) if c1 == c2 => {
                    prev = Some(c1);
                    // For now, all characters of `iter1` and `iter2` are equal.
                }
                (Some(mut c1), Some(mut c2)) => {
                    let mut number_ordering = Ordering::Equal;
                    // Compare numbers
                    // We don't skip leading zeroes.
                    loop {
                        match (self.as_digit(&c1), self.as_digit(&c2)) {
                            (None, None) => {
                                if number_ordering == Ordering::Equal {
                                    // The numbers are equal, we have to compare `c1` and `c2` that are not digits.
                                    break;
                                }
                                return number_ordering;
                            }
                            (None, Some(_)) => {
                                // We consider that we compare numbers if they have at least one digit.
                                if prev.is_some_and(|c| self.as_digit(&c).is_some()) {
                                    // right has more digits than left.
                                    // Thus left is smaller than right.
                                    return Ordering::Less;
                                }
                                break;
                            }
                            (Some(_), None) => {
                                // We consider that we compare numbers if they have at least one digit.
                                if prev.is_some_and(|c| self.as_digit(&c).is_some()) {
                                    // left has more digits than right.
                                    // Thus left is larger than right.
                                    return Ordering::Greater;
                                }
                                break;
                            }
                            (Some(n1), Some(n2)) => {
                                // Even if `number_ordering` is not `Ordering::Equal`,
                                // we cannot return `number_ordering` because we have to check how many digits there are.
                                // If one has more digits, then it is larger than the other regardless of `number_ordering`.
                                // For example, `10` is larger than `9`, while `1` is smaller than `1`.
                                number_ordering = number_ordering.then(n1.cmp(&n2));
                            }
                        }
                        match (iter1.next(), iter2.next()) {
                            (None, None) => {
                                // left and right have the same number of digits.
                                // Thus, the order of the first digit correspond to their order.
                                return number_ordering;
                            }
                            (None, Some(c2)) => {
                                return if self.as_digit(&c2).is_some() {
                                    // right has more digits than left.
                                    // Thus, left is smaller than right.
                                    Ordering::Less
                                } else {
                                    // left and right have the same number of digits.
                                    number_ordering.then(Ordering::Less)
                                };
                            }
                            (Some(c1), None) => {
                                return if self.as_digit(&c1).is_some() {
                                    // left has more digits than right.
                                    // Thus, left is larger than right.
                                    Ordering::Greater
                                } else {
                                    // left and right have the same number of digits.
                                    number_ordering.then(Ordering::Greater)
                                };
                            }
                            (Some(next1), Some(next2)) => {
                                prev = Some(c1);
                                c1 = next1;
                                c2 = next2;
                            }
                        }
                    }
                    let ordering = self.weight(&c1).cmp(&self.weight(&c2));
                    if ordering != Ordering::Equal {
                        return ordering;
                    }
                    prev = Some(c1);
                }
            }
        }
    }
}

/// An ASCII collator defines an order between ASCII characters.
///
/// The order is extended at any byte value.
/// This order may differ from their binary order.
/// This is often used to provide a more natural order for humans than the binary order represents.
pub trait AsciiCollator {
    /// Weight of a given byte.
    /// The order between two bytes is defined by the order between their weights.
    /// Usually a byte that is not a valid ASCII character is mapped to itself.
    /// You may use [ascii_collation_weight_from] to create the weight table from the an ASCII collation table.
    const WEIGHTS: [u8; 256];

    /// Compare `s1` and `s2` using [self] as collator.
    fn cmp_str(&self, s1: &str, s2: &str) -> Ordering
    where
        Self: Collator<Char = u8>,
    {
        self.cmp(s1.bytes(), s2.bytes())
    }

    /// Compare `s1` and `s2` using [self] as collator.
    fn cmp_osstr(&self, s1: &OsStr, s2: &OsStr) -> Ordering
    where
        Self: Collator<Char = u8>,
    {
        self.cmp_bytes(s1.as_encoded_bytes(), s2.as_encoded_bytes())
    }

    /// Compare `s1` and `s2` using [self] as collator.
    fn cmp_bytes(&self, s1: &[u8], s2: &[u8]) -> Ordering
    where
        Self: Collator<Char = u8>,
    {
        self.cmp(s1.iter().copied(), s2.iter().copied())
    }
}
impl<C: AsciiCollator> Collator for C {
    type Char = u8;

    fn weight(&self, c: &Self::Char) -> impl Ord {
        // SAFETY: safe indexing because [Self::WEIGHTS] has exactly `u8::MAX` items.
        unsafe { *Self::WEIGHTS.get_unchecked(*c as usize) }
    }

    fn as_digit(&self, c: &Self::Char) -> Option<impl Ord> {
        c.is_ascii_digit().then_some(*c)
    }
}

/// Unicode collation for ASCII extracted from the CLDR (Common Locale Data Repository) root table.
pub struct CldrAsciiCollator;
impl CldrAsciiCollator {
    /// Unicode collation for ASCII extracted from the CLDR root table:
    /// <https://raw.githubusercontent.com/unicode-org/cldr/refs/heads/main/common/uca/allkeys_CLDR.txt>.
    /// See also <https://www.unicode.org/reports/tr35/tr35-collation.html#CLDR_Collation_Algorithm>.
    const COLLATION: [u8; 128] = [
        b'\0', 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13,
        0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x7f, b'\t', b'\n',
        0x0b, 0x0c, b'\r', b' ', b'_', b'-', b',', b';', b':', b'!', b'?', b'.', b'\'', b'"', b'(',
        b')', b'[', b']', b'{', b'}', b'@', b'*', b'/', b'\\', b'&', b'#', b'%', b'`', b'^', b'+',
        b'<', b'=', b'>', b'|', b'~', b'$', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8',
        b'9', b'A', b'a', b'B', b'b', b'C', b'c', b'D', b'd', b'E', b'e', b'F', b'f', b'G', b'g',
        b'H', b'h', b'I', b'i', b'J', b'j', b'K', b'k', b'L', b'l', b'M', b'm', b'N', b'n', b'O',
        b'o', b'P', b'p', b'Q', b'q', b'R', b'r', b'S', b's', b'T', b't', b'U', b'u', b'V', b'v',
        b'W', b'w', b'X', b'x', b'Y', b'y', b'Z', b'z',
    ];
}
impl AsciiCollator for CldrAsciiCollator {
    const WEIGHTS: [u8; 256] = ascii_collation_weight_from(&Self::COLLATION);
}

/// Generate the collation weight table from an ASCII collation table.
/// The last 128 bytes are mapped to themselves.
pub const fn ascii_collation_weight_from(collation_table: &[u8; 128]) -> [u8; 256] {
    let mut result = [0u8; 256];
    let mut i = 0;
    while i < collation_table.len() {
        debug_assert!(
            result[collation_table[i] as usize] == 0,
            "A character appears twice in the collation table."
        );
        result[collation_table[i] as usize] = i as u8;
        i += 1;
    }
    while i < result.len() {
        result[i] = i as u8;
        i += 1;
    }
    result
}

pub trait StrLikeExtension: ToOwned {
    /// Returns the same value as String::to_lowercase. The only difference
    /// is that this functions returns ```Cow``` and does not allocate
    /// if the string is already in lowercase.
    fn to_ascii_lowercase_cow(&self) -> Cow<Self>;

    /// Compare two strings using a natural ASCII order.
    ///
    /// Uppercase letters come first (e.g. `A` < `a` < `B` < `b`)
    /// and number are compared in a human way (e.g. `9` < `10`).
    fn ascii_nat_cmp(&self, other: &Self) -> Ordering;
}

pub trait StrOnlyExtension: ToOwned {
    /// Returns the same value as String::to_lowercase. The only difference
    /// is that this functions returns ```Cow``` and does not allocate
    /// if the string is already in lowercase.
    fn to_lowercase_cow(&self) -> Cow<Self>;
}

impl StrLikeExtension for str {
    fn to_ascii_lowercase_cow(&self) -> Cow<Self> {
        let has_ascii_uppercase = self.bytes().any(|b| b.is_ascii_uppercase());
        if has_ascii_uppercase {
            #[expect(clippy::disallowed_methods)]
            Cow::Owned(self.to_ascii_lowercase())
        } else {
            Cow::Borrowed(self)
        }
    }

    fn ascii_nat_cmp(&self, other: &Self) -> Ordering {
        self.as_bytes().ascii_nat_cmp(other.as_bytes())
    }
}

impl StrOnlyExtension for str {
    fn to_lowercase_cow(&self) -> Cow<Self> {
        let has_uppercase = self.chars().any(char::is_uppercase);
        if has_uppercase {
            #[expect(clippy::disallowed_methods)]
            Cow::Owned(self.to_lowercase())
        } else {
            Cow::Borrowed(self)
        }
    }
}

impl StrLikeExtension for std::ffi::OsStr {
    fn to_ascii_lowercase_cow(&self) -> Cow<Self> {
        let has_ascii_uppercase = self
            .as_encoded_bytes()
            .iter()
            .any(|b| b.is_ascii_uppercase());
        if has_ascii_uppercase {
            #[expect(clippy::disallowed_methods)]
            Cow::Owned(self.to_ascii_lowercase())
        } else {
            Cow::Borrowed(self)
        }
    }

    fn ascii_nat_cmp(&self, other: &Self) -> Ordering {
        self.as_encoded_bytes()
            .ascii_nat_cmp(other.as_encoded_bytes())
    }
}

impl StrLikeExtension for [u8] {
    fn to_ascii_lowercase_cow(&self) -> Cow<Self> {
        let has_ascii_uppercase = self.iter().any(|b| b.is_ascii_uppercase());
        if has_ascii_uppercase {
            Cow::Owned(self.to_ascii_lowercase())
        } else {
            Cow::Borrowed(self)
        }
    }

    fn ascii_nat_cmp(&self, other: &Self) -> Ordering {
        CldrAsciiCollator.cmp(self.iter().copied(), other.iter().copied())
    }
}

// TODO: Once trait-alias are stabilized it would be enough to `use` this trait instead of individual ones.
// https://doc.rust-lang.org/stable/unstable-book/language-features/trait-alias.html
pub trait StrExtension: StrOnlyExtension + StrLikeExtension {}
impl<T: StrOnlyExtension + StrLikeExtension> StrExtension for T {}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn test_case_identify() {
        let no_effect = true;

        assert_eq!(Case::identify("100", no_effect), Case::Number);

        assert_eq!(Case::identify("aHttpServer", no_effect), Case::Camel);
        assert_eq!(Case::identify("aHTTPServer", true), Case::Unknown);
        assert_eq!(Case::identify("aHTTPServer", false), Case::Camel);
        assert_eq!(Case::identify("v8Engine", no_effect), Case::Camel);
        assert_eq!(Case::identify("2024Edition", no_effect), Case::Camel);

        assert_eq!(Case::identify("HTTP_SERVER", no_effect), Case::Constant);
        assert_eq!(Case::identify("V8_ENGINE", no_effect), Case::Constant);
        assert_eq!(Case::identify("2024_EDITION", no_effect), Case::Unknown);

        assert_eq!(Case::identify("http-server", no_effect), Case::Kebab);
        assert_eq!(Case::identify("2024-edition", no_effect), Case::Kebab);

        assert_eq!(Case::identify("httpserver", no_effect), Case::Lower);

        assert_eq!(Case::identify("T", no_effect), Case::NumberableCapital);
        assert_eq!(Case::identify("T1", no_effect), Case::NumberableCapital);

        assert_eq!(Case::identify("HttpServer", no_effect), Case::Pascal);
        assert_eq!(Case::identify("HTTPServer", true), Case::Unknown);
        assert_eq!(Case::identify("HTTPServer", false), Case::Pascal);
        assert_eq!(Case::identify("V8Engine", true), Case::Pascal);

        assert_eq!(Case::identify("http_server", no_effect), Case::Snake);
        assert_eq!(Case::identify("2024_edition", no_effect), Case::Snake);

        assert_eq!(Case::identify("HTTPSERVER", no_effect), Case::Upper);
        assert_eq!(Case::identify("2024EDITION", no_effect), Case::Unknown);

        assert_eq!(Case::identify("100안녕하세요", no_effect), Case::Uni);
        assert_eq!(Case::identify("안녕하세요", no_effect), Case::Uni);

        // Don't allow identifiers that starts/ends with a delimiter
        assert_eq!(Case::identify("-a", no_effect), Case::Unknown);
        assert_eq!(Case::identify("_a", no_effect), Case::Unknown);
        assert_eq!(Case::identify("a-", no_effect), Case::Unknown);
        assert_eq!(Case::identify("a_", no_effect), Case::Unknown);

        // Don't allow identifiers that use consecutive delimiters
        assert_eq!(Case::identify("a--a", no_effect), Case::Unknown);
        assert_eq!(Case::identify("a__a", no_effect), Case::Unknown);

        assert_eq!(Case::identify("", no_effect), Case::Unknown);
        assert_eq!(Case::identify("-", no_effect), Case::Unknown);
        assert_eq!(Case::identify("_", no_effect), Case::Unknown);
        assert_eq!(Case::identify("안녕하세요ABC", no_effect), Case::Unknown);
        assert_eq!(Case::identify("안녕하세요abc", no_effect), Case::Unknown);
        assert_eq!(Case::identify("안녕하세요_ABC", no_effect), Case::Unknown);
        assert_eq!(Case::identify("안녕하세요_abc", no_effect), Case::Unknown);
        assert_eq!(Case::identify("안녕하세요-abc", no_effect), Case::Unknown);
    }

    #[test]
    fn test_cases_contains() {
        // Individual cases
        assert!(Cases::from(Case::Unknown).contains(Case::Unknown));
        assert!(!Cases::from(Case::Camel).contains(Case::Unknown));
        assert!(!Cases::from(Case::Constant).contains(Case::Unknown));
        assert!(!Cases::from(Case::Kebab).contains(Case::Unknown));
        assert!(!Cases::from(Case::Lower).contains(Case::Unknown));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Unknown));
        assert!(!Cases::from(Case::Pascal).contains(Case::Unknown));
        assert!(!Cases::from(Case::Snake).contains(Case::Unknown));
        assert!(!Cases::from(Case::Uni).contains(Case::Unknown));
        assert!(!Cases::from(Case::Upper).contains(Case::Unknown));

        assert!(Cases::from(Case::Unknown).contains(Case::Camel));
        assert!(Cases::from(Case::Camel).contains(Case::Camel));
        assert!(!Cases::from(Case::Constant).contains(Case::Camel));
        assert!(!Cases::from(Case::Kebab).contains(Case::Camel));
        assert!(!Cases::from(Case::Lower).contains(Case::Camel));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Camel));
        assert!(!Cases::from(Case::Pascal).contains(Case::Camel));
        assert!(!Cases::from(Case::Snake).contains(Case::Camel));
        assert!(!Cases::from(Case::Uni).contains(Case::Camel));
        assert!(!Cases::from(Case::Upper).contains(Case::Camel));

        assert!(Cases::from(Case::Unknown).contains(Case::Constant));
        assert!(!Cases::from(Case::Camel).contains(Case::Constant));
        assert!(Cases::from(Case::Constant).contains(Case::Constant));
        assert!(!Cases::from(Case::Kebab).contains(Case::Constant));
        assert!(!Cases::from(Case::Lower).contains(Case::Constant));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Constant));
        assert!(!Cases::from(Case::Pascal).contains(Case::Constant));
        assert!(!Cases::from(Case::Snake).contains(Case::Constant));
        assert!(!Cases::from(Case::Uni).contains(Case::Constant));
        assert!(!Cases::from(Case::Upper).contains(Case::Constant));

        assert!(Cases::from(Case::Unknown).contains(Case::Kebab));
        assert!(!Cases::from(Case::Camel).contains(Case::Kebab));
        assert!(!Cases::from(Case::Constant).contains(Case::Kebab));
        assert!(Cases::from(Case::Kebab).contains(Case::Kebab));
        assert!(!Cases::from(Case::Lower).contains(Case::Kebab));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Kebab));
        assert!(!Cases::from(Case::Pascal).contains(Case::Kebab));
        assert!(!Cases::from(Case::Snake).contains(Case::Kebab));
        assert!(!Cases::from(Case::Uni).contains(Case::Kebab));
        assert!(!Cases::from(Case::Upper).contains(Case::Kebab));

        assert!(Cases::from(Case::Unknown).contains(Case::Lower));
        assert!(Cases::from(Case::Camel).contains(Case::Lower));
        assert!(!Cases::from(Case::Constant).contains(Case::Lower));
        assert!(Cases::from(Case::Kebab).contains(Case::Lower));
        assert!(Cases::from(Case::Lower).contains(Case::Lower));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Lower));
        assert!(!Cases::from(Case::Pascal).contains(Case::Lower));
        assert!(Cases::from(Case::Snake).contains(Case::Lower));
        assert!(!Cases::from(Case::Uni).contains(Case::Lower));
        assert!(!Cases::from(Case::Upper).contains(Case::Lower));

        assert!(Cases::from(Case::Unknown).contains(Case::NumberableCapital));
        assert!(!Cases::from(Case::Camel).contains(Case::NumberableCapital));
        assert!(Cases::from(Case::Constant).contains(Case::NumberableCapital));
        assert!(!Cases::from(Case::Kebab).contains(Case::NumberableCapital));
        assert!(!Cases::from(Case::Lower).contains(Case::NumberableCapital));
        assert!(Cases::from(Case::NumberableCapital).contains(Case::NumberableCapital));
        assert!(Cases::from(Case::Pascal).contains(Case::NumberableCapital));
        assert!(!Cases::from(Case::Snake).contains(Case::NumberableCapital));
        assert!(!Cases::from(Case::Uni).contains(Case::NumberableCapital));
        assert!(Cases::from(Case::Upper).contains(Case::NumberableCapital));

        assert!(Cases::from(Case::Unknown).contains(Case::Pascal));
        assert!(!Cases::from(Case::Camel).contains(Case::Pascal));
        assert!(!Cases::from(Case::Constant).contains(Case::Pascal));
        assert!(!Cases::from(Case::Kebab).contains(Case::Pascal));
        assert!(!Cases::from(Case::Lower).contains(Case::Pascal));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Pascal));
        assert!(Cases::from(Case::Pascal).contains(Case::Pascal));
        assert!(!Cases::from(Case::Snake).contains(Case::Pascal));
        assert!(!Cases::from(Case::Uni).contains(Case::Pascal));
        assert!(!Cases::from(Case::Upper).contains(Case::Pascal));

        assert!(Cases::from(Case::Unknown).contains(Case::Snake));
        assert!(!Cases::from(Case::Camel).contains(Case::Snake));
        assert!(!Cases::from(Case::Constant).contains(Case::Snake));
        assert!(!Cases::from(Case::Kebab).contains(Case::Snake));
        assert!(!Cases::from(Case::Lower).contains(Case::Snake));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Snake));
        assert!(!Cases::from(Case::Pascal).contains(Case::Snake));
        assert!(Cases::from(Case::Snake).contains(Case::Snake));
        assert!(!Cases::from(Case::Uni).contains(Case::Snake));
        assert!(!Cases::from(Case::Upper).contains(Case::Snake));

        assert!(Cases::from(Case::Unknown).contains(Case::Uni));
        assert!(!Cases::from(Case::Camel).contains(Case::Uni));
        assert!(!Cases::from(Case::Constant).contains(Case::Uni));
        assert!(!Cases::from(Case::Kebab).contains(Case::Uni));
        assert!(!Cases::from(Case::Lower).contains(Case::Uni));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Uni));
        assert!(!Cases::from(Case::Pascal).contains(Case::Uni));
        assert!(!Cases::from(Case::Snake).contains(Case::Uni));
        assert!(Cases::from(Case::Uni).contains(Case::Uni));
        assert!(!Cases::from(Case::Upper).contains(Case::Uni));

        assert!(Cases::from(Case::Unknown).contains(Case::Upper));
        assert!(!Cases::from(Case::Camel).contains(Case::Upper));
        assert!(Cases::from(Case::Constant).contains(Case::Upper));
        assert!(!Cases::from(Case::Kebab).contains(Case::Upper));
        assert!(!Cases::from(Case::Lower).contains(Case::Upper));
        assert!(!Cases::from(Case::NumberableCapital).contains(Case::Upper));
        assert!(!Cases::from(Case::Pascal).contains(Case::Upper));
        assert!(!Cases::from(Case::Snake).contains(Case::Upper));
        assert!(!Cases::from(Case::Uni).contains(Case::Upper));
        assert!(Cases::from(Case::Upper).contains(Case::Upper));

        // Set of cases
        assert!((Case::Camel | Case::Kebab | Case::Snake).contains(Case::Camel));
        assert!((Case::Camel | Case::Kebab | Case::Snake).contains(Case::Kebab));
        assert!((Case::Camel | Case::Kebab | Case::Snake).contains(Case::Snake));
        assert!((Case::Camel | Case::Kebab | Case::Snake).contains(Case::Lower));
        assert!(!(Case::Camel | Case::Kebab | Case::Snake).contains(Case::Unknown));
        assert!(!(Case::Camel | Case::Kebab | Case::Snake).contains(Case::Constant));
        assert!(!(Case::Camel | Case::Kebab | Case::Snake).contains(Case::Upper));
        assert!(!(Case::Camel | Case::Kebab | Case::Snake).contains(Case::NumberableCapital));
        assert!(!(Case::Camel | Case::Kebab | Case::Snake).contains(Case::Uni));

        assert!((Case::Constant | Case::Upper).contains(Case::Constant));
        assert!((Case::Constant | Case::Upper).contains(Case::Upper));
        assert!((Case::Constant | Case::Upper).contains(Case::NumberableCapital));
        assert!(!(Case::Constant | Case::Upper).contains(Case::Unknown));
        assert!(!(Case::Constant | Case::Upper).contains(Case::Camel));
        assert!(!(Case::Constant | Case::Upper).contains(Case::Kebab));
        assert!(!(Case::Constant | Case::Upper).contains(Case::Snake));
        assert!(!(Case::Constant | Case::Upper).contains(Case::Lower));
        assert!(!(Case::Constant | Case::Upper).contains(Case::Uni));
    }

    #[test]
    fn test_case_convert() {
        assert_eq!(Case::Camel.convert("camelCase"), "camelCase");
        assert_eq!(Case::Camel.convert("CONSTANT_CASE"), "constantCase");
        assert_eq!(Case::Camel.convert("kebab-case"), "kebabCase");
        assert_eq!(Case::Camel.convert("PascalCase"), "pascalCase");
        assert_eq!(Case::Camel.convert("snake_case"), "snakeCase");
        assert_eq!(Case::Camel.convert("Unknown_Style"), "unknownStyle");

        assert_eq!(Case::Constant.convert("camelCase"), "CAMEL_CASE");
        assert_eq!(Case::Constant.convert("CONSTANT_CASE"), "CONSTANT_CASE");
        assert_eq!(Case::Constant.convert("kebab-case"), "KEBAB_CASE");
        assert_eq!(Case::Constant.convert("PascalCase"), "PASCAL_CASE");
        assert_eq!(Case::Constant.convert("snake_case"), "SNAKE_CASE");
        assert_eq!(Case::Constant.convert("Unknown_Style"), "UNKNOWN_STYLE");

        assert_eq!(Case::Kebab.convert("camelCase"), "camel-case");
        assert_eq!(Case::Kebab.convert("CONSTANT_CASE"), "constant-case");
        assert_eq!(Case::Kebab.convert("kebab-case"), "kebab-case");
        assert_eq!(Case::Kebab.convert("PascalCase"), "pascal-case");
        assert_eq!(Case::Kebab.convert("snake_case"), "snake-case");
        assert_eq!(Case::Kebab.convert("Unknown_Style"), "unknown-style");

        assert_eq!(Case::Lower.convert("camelCase"), "camelcase");
        assert_eq!(Case::Lower.convert("CONSTANT_CASE"), "constantcase");
        assert_eq!(Case::Lower.convert("kebab-case"), "kebabcase");
        assert_eq!(Case::Lower.convert("PascalCase"), "pascalcase");
        assert_eq!(Case::Lower.convert("snake_case"), "snakecase");
        assert_eq!(Case::Lower.convert("Unknown_Style"), "unknownstyle");

        assert_eq!(Case::NumberableCapital.convert("LONG"), "L");

        assert_eq!(Case::Pascal.convert("camelCase"), "CamelCase");
        assert_eq!(Case::Pascal.convert("CONSTANT_CASE"), "ConstantCase");
        assert_eq!(Case::Pascal.convert("kebab-case"), "KebabCase");
        assert_eq!(Case::Pascal.convert("PascalCase"), "PascalCase");
        assert_eq!(Case::Pascal.convert("V8Engine"), "V8Engine");
        assert_eq!(Case::Pascal.convert("snake_case"), "SnakeCase");
        assert_eq!(Case::Pascal.convert("Unknown_Style"), "UnknownStyle");

        assert_eq!(Case::Snake.convert("camelCase"), "camel_case");
        assert_eq!(Case::Snake.convert("CONSTANT_CASE"), "constant_case");
        assert_eq!(Case::Snake.convert("kebab-case"), "kebab_case");
        assert_eq!(Case::Snake.convert("PascalCase"), "pascal_case");
        assert_eq!(Case::Snake.convert("snake_case"), "snake_case");
        assert_eq!(Case::Snake.convert("Unknown_Style"), "unknown_style");

        assert_eq!(Case::Upper.convert("camelCase"), "CAMELCASE");
        assert_eq!(Case::Upper.convert("CONSTANT_CASE"), "CONSTANTCASE");
        assert_eq!(Case::Upper.convert("kebab-case"), "KEBABCASE");
        assert_eq!(Case::Upper.convert("PascalCase"), "PASCALCASE");
        assert_eq!(Case::Upper.convert("snake_case"), "SNAKECASE");
        assert_eq!(Case::Upper.convert("Unknown_Style"), "UNKNOWNSTYLE");

        assert_eq!(Case::Uni.convert("안녕하세요"), "안녕하세요");
        assert_eq!(Case::Uni.convert("a안b녕c하_세D요E"), "안녕하세요");

        assert_eq!(Case::Unknown.convert("Unknown_Style"), "Unknown_Style");
    }

    #[test]
    fn test_cases_iter() {
        fn vec(value: impl Into<Cases>) -> Vec<Case> {
            value.into().into_iter().collect::<Vec<_>>()
        }

        assert_eq!(vec(Cases::empty()).as_slice(), &[]);
        assert_eq!(vec(Case::Unknown).as_slice(), &[Case::Unknown]);
        assert_eq!(vec(Case::Camel).as_slice(), &[Case::Camel]);
        assert_eq!(vec(Case::Kebab).as_slice(), &[Case::Kebab]);
        assert_eq!(vec(Case::Snake).as_slice(), &[Case::Snake]);
        assert_eq!(vec(Case::Lower).as_slice(), &[Case::Lower]);
        assert_eq!(vec(Case::Pascal).as_slice(), &[Case::Pascal]);
        assert_eq!(vec(Case::Constant).as_slice(), &[Case::Constant]);
        assert_eq!(vec(Case::Upper).as_slice(), &[Case::Upper]);
        assert_eq!(vec(Case::Uni).as_slice(), &[Case::Uni]);
        assert_eq!(vec(Case::Number).as_slice(), &[Case::Number]);
        assert_eq!(
            vec(Case::NumberableCapital).as_slice(),
            &[Case::NumberableCapital]
        );

        assert_eq!(
            vec(Case::Unknown | Case::Camel).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(
            vec(Case::Unknown | Case::Kebab).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(
            vec(Case::Unknown | Case::Snake).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(
            vec(Case::Unknown | Case::Lower).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(
            vec(Case::Unknown | Case::Pascal).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(
            vec(Case::Unknown | Case::Constant).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(
            vec(Case::Unknown | Case::Upper).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(
            vec(Case::Unknown | Case::NumberableCapital).as_slice(),
            &[Case::Unknown]
        );
        assert_eq!(vec(Case::Unknown | Case::Uni).as_slice(), &[Case::Unknown]);
        assert_eq!(
            vec(Case::Unknown | Case::Pascal | Case::Camel).as_slice(),
            &[Case::Unknown]
        );

        assert_eq!(vec(Case::Camel | Case::Lower).as_slice(), &[Case::Camel]);
        assert_eq!(vec(Case::Kebab | Case::Lower).as_slice(), &[Case::Kebab]);
        assert_eq!(vec(Case::Snake | Case::Lower).as_slice(), &[Case::Snake]);

        assert_eq!(vec(Case::Lower | Case::Number).as_slice(), &[Case::Lower]);
        assert_eq!(vec(Case::Camel | Case::Number).as_slice(), &[Case::Camel]);
        assert_eq!(vec(Case::Kebab | Case::Number).as_slice(), &[Case::Kebab]);
        assert_eq!(vec(Case::Snake | Case::Number).as_slice(), &[Case::Snake]);

        assert_eq!(
            vec(Case::Constant | Case::Upper).as_slice(),
            &[Case::Constant]
        );

        assert_eq!(
            vec(Case::Pascal | Case::NumberableCapital).as_slice(),
            &[Case::Pascal]
        );
        assert_eq!(
            vec(Case::Constant | Case::NumberableCapital).as_slice(),
            &[Case::Constant]
        );
        assert_eq!(
            vec(Case::Upper | Case::NumberableCapital).as_slice(),
            &[Case::Upper]
        );

        assert_eq!(
            vec(Case::Pascal | Case::Camel).as_slice(),
            &[Case::Camel, Case::Pascal]
        );
        assert_eq!(
            vec(Case::NumberableCapital | Case::Uni).as_slice(),
            &[Case::NumberableCapital, Case::Uni]
        );

        assert_eq!(
            vec(Case::Pascal
                | Case::Constant
                | Case::Camel
                | Case::Kebab
                | Case::Snake
                | Case::Uni)
            .as_slice(),
            &[
                Case::Camel,
                Case::Kebab,
                Case::Snake,
                Case::Pascal,
                Case::Constant,
                Case::Uni
            ]
        );
    }

    #[test]
    fn test_leading_bit_to_case() {
        for (i, case) in LEADING_BIT_INDEX_TO_CASE.iter().enumerate() {
            assert_eq!(i as u16, 15u16 - (*case as u16).leading_zeros() as u16)
        }
    }

    #[test]
    fn test_size_hint_upper_limit() {
        let mut cases = Cases::empty();
        let mut max_count = 0;
        for case in LEADING_BIT_INDEX_TO_CASE {
            let count = (cases | case).into_iter().count();
            if count >= max_count {
                cases |= case;
                max_count = count;
            }
        }
        assert_eq!(cases.into_iter().size_hint().1, Some(max_count));
    }

    #[test]
    fn to_ascii_lowercase_cow() {
        assert_eq!("test", "Test".to_ascii_lowercase_cow());
        assert_eq!(
            OsStr::new("test"),
            OsStr::new("Test").to_ascii_lowercase_cow()
        );
        assert_eq!(b"test", b"Test".to_ascii_lowercase_cow().as_ref());

        assert_eq!("test", "teSt".to_ascii_lowercase_cow());
        assert_eq!("te😀st", "te😀St".to_ascii_lowercase_cow());
        assert_eq!(
            OsStr::new("test"),
            OsStr::new("teSt").to_ascii_lowercase_cow()
        );
        assert_eq!(b"test", b"teSt".to_ascii_lowercase_cow().as_ref());

        assert!(matches!("test".to_ascii_lowercase_cow(), Cow::Borrowed(_)));
        assert!(matches!(
            OsStr::new("test").to_ascii_lowercase_cow(),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn to_lowercase_cow() {
        assert_eq!("test", "Test".to_lowercase_cow());

        assert_eq!("test", "teSt".to_lowercase_cow());
        assert_eq!("te😀st", "te😀St".to_lowercase_cow());

        assert!(matches!("test".to_lowercase_cow(), Cow::Borrowed(_)));

        assert_eq!("ėest", "Ėest".to_lowercase_cow());

        assert_eq!("tešt", "teŠt".to_lowercase_cow());
        assert_eq!("te😀st", "te😀St".to_lowercase_cow());

        assert!(matches!("tešt".to_lowercase_cow(), Cow::Borrowed(_)));
    }

    #[test]
    fn collation_weight_unique() {
        for weight in 0..=255 {
            assert!(CldrAsciiCollator::WEIGHTS.contains(&weight));
        }
    }

    #[test]
    fn ascii_nat_ord() {
        assert_eq!("".ascii_nat_cmp(""), Ordering::Equal);
        assert_eq!("a".ascii_nat_cmp(""), Ordering::Greater);
        assert_eq!("".ascii_nat_cmp("a"), Ordering::Less);

        assert_eq!("ab".ascii_nat_cmp("ab"), Ordering::Equal);
        assert_eq!("abc".ascii_nat_cmp("ab"), Ordering::Greater);
        assert_eq!("ab".ascii_nat_cmp("abc"), Ordering::Less);

        assert_eq!("A".ascii_nat_cmp("a"), Ordering::Less);
        assert_eq!("a".ascii_nat_cmp("B"), Ordering::Less);

        assert_eq!("9".ascii_nat_cmp("10"), Ordering::Less);
        assert_eq!("10".ascii_nat_cmp("10"), Ordering::Equal);
        assert_eq!("10".ascii_nat_cmp("9"), Ordering::Greater);
        assert_eq!("09".ascii_nat_cmp("10"), Ordering::Less);
        assert_eq!("a00".ascii_nat_cmp("a01"), Ordering::Less);
        assert_eq!("a00b".ascii_nat_cmp("a01b"), Ordering::Less);

        assert_eq!("a10".ascii_nat_cmp("a009"), Ordering::Less);

        assert_eq!("1a".ascii_nat_cmp("9"), Ordering::Less);
        assert_eq!("9".ascii_nat_cmp("1a"), Ordering::Greater);

        assert_eq!("0".ascii_nat_cmp("a"), Ordering::Less);
    }
}
