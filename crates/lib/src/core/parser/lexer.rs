use std::borrow::Cow;
use std::fmt::Debug;
use std::ops::Range;

use fancy_regex::Regex;

use super::markers::PositionMarker;
use super::segments::base::ErasedSegment;
use super::segments::meta::EndOfFile;
use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::errors::{SQLLexError, ValueError};
use crate::core::parser::segments::base::Segment;
use crate::core::slice_helpers::{is_zero_slice, offset_slice};
use crate::core::templaters::base::TemplatedFile;
use crate::dialects::SyntaxKind;

/// An element matched during lexing.
#[derive(Debug, Clone)]
pub struct Element<'a> {
    name: &'static str,
    text: Cow<'a, str>,
    value: fn(&str, PositionMarker) -> ErasedSegment,
}

impl<'a> Element<'a> {
    fn new(
        name: &'static str,
        value: fn(&str, PositionMarker) -> ErasedSegment,
        text: impl Into<Cow<'a, str>>,
    ) -> Self {
        Self { name, value, text: text.into() }
    }
}

/// A LexedElement, bundled with it's position in the templated file.
#[derive(Debug)]
pub struct TemplateElement<'a> {
    raw: Cow<'a, str>,
    template_slice: Range<usize>,
    matcher: Info,
}

#[derive(Debug)]
struct Info {
    name: &'static str,
    constructor: fn(&str, PositionMarker) -> ErasedSegment,
}

impl<'a> TemplateElement<'a> {
    /// Make a TemplateElement from a LexedElement.
    pub fn from_element(element: Element<'a>, template_slice: Range<usize>) -> Self {
        TemplateElement {
            raw: element.text,
            template_slice,
            matcher: Info { name: element.name, constructor: element.value },
        }
    }

    pub fn to_segment(
        &self,
        pos_marker: PositionMarker,
        subslice: Option<Range<usize>>,
    ) -> ErasedSegment {
        let slice = subslice.map_or_else(|| self.raw.as_ref(), |slice| &self.raw[slice]);
        (self.matcher.constructor)(slice, pos_marker)
    }
}

/// A class to hold matches from the lexer.
#[derive(Debug)]
pub struct Match<'a> {
    pub forward_string: &'a str,
    pub elements: Vec<Element<'a>>,
}

impl Match<'_> {
    /// A LexMatch is truthy if it contains a non-zero number of matched
    /// elements.
    pub fn is_non_empty(&self) -> bool {
        !self.elements.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Matcher {
    pattern: Pattern,
    subdivider: Option<Pattern>,
    trim_post_subdivide: Option<Pattern>,
}

impl Matcher {
    pub const fn new(pattern: Pattern) -> Self {
        Self { pattern, subdivider: None, trim_post_subdivide: None }
    }

    pub const fn string(
        name: &'static str,
        pattern: &'static str,
        constructor: fn(&str, PositionMarker) -> ErasedSegment,
    ) -> Self {
        Self::new(Pattern::string(name, pattern, constructor))
    }

    pub fn regex(
        name: &'static str,
        pattern: &'static str,
        constructor: fn(&str, PositionMarker) -> ErasedSegment,
    ) -> Self {
        Self::new(Pattern::regex(name, pattern, constructor))
    }

    pub fn subdivider(mut self, subdivider: Pattern) -> Self {
        self.subdivider = Some(subdivider);
        self
    }

    pub fn post_subdivide(mut self, trim_post_subdivide: Pattern) -> Self {
        self.trim_post_subdivide = Some(trim_post_subdivide);
        self
    }

    pub fn name(&self) -> &'static str {
        self.pattern.name
    }

    pub fn matches<'a>(&self, forward_string: &'a str) -> Match<'a> {
        match self.pattern.matches(forward_string) {
            Some(matched) => {
                let new_elements = self.subdivide(matched, self.pattern.value);

                Match { forward_string: &forward_string[matched.len()..], elements: new_elements }
            }
            None => Match { forward_string, elements: Vec::new() },
        }
    }

    fn subdivide<'a>(
        &self,
        matched: &'a str,
        matched_kind: fn(&str, PositionMarker) -> ErasedSegment,
    ) -> Vec<Element<'a>> {
        match &self.subdivider {
            Some(subdivider) => {
                let mut elem_buff = Vec::new();
                let mut str_buff = matched;

                while !str_buff.is_empty() {
                    let Some(div_pos) = subdivider.search(str_buff) else {
                        let mut trimmed_elems = self.trim_match(str_buff);
                        elem_buff.append(&mut trimmed_elems);
                        break;
                    };

                    let mut trimmed_elems = self.trim_match(&str_buff[..div_pos.start]);
                    let div_elem = Element::new(
                        subdivider.name,
                        subdivider.value,
                        &str_buff[div_pos.start..div_pos.end],
                    );

                    elem_buff.append(&mut trimmed_elems);
                    elem_buff.push(div_elem);

                    str_buff = &str_buff[div_pos.end..];
                }

                elem_buff
            }
            None => {
                vec![Element::new(self.name(), matched_kind, matched)]
            }
        }
    }

    fn trim_match<'a>(&self, matched_str: &'a str) -> Vec<Element<'a>> {
        let Some(trim_post_subdivide) = &self.trim_post_subdivide else {
            return Vec::new();
        };

        let mk_element =
            |text| Element::new(trim_post_subdivide.name, trim_post_subdivide.value, text);

        let mut elem_buff = Vec::new();
        let mut content_buff = String::new();
        let mut str_buff = matched_str;

        while !str_buff.is_empty() {
            let Some(trim_pos) = trim_post_subdivide.search(str_buff) else {
                break;
            };

            let start = trim_pos.start;
            let end = trim_pos.end;

            if start == 0 {
                elem_buff.push(mk_element(&str_buff[..end]));
                str_buff = str_buff[end..].into();
            } else if end == str_buff.len() {
                let raw = format!("{}{}", content_buff, &str_buff[..start]);

                elem_buff.push(Element::new(
                    trim_post_subdivide.name,
                    trim_post_subdivide.value,
                    raw,
                ));
                elem_buff.push(mk_element(&str_buff[start..end]));

                content_buff.clear();
                str_buff = "";
            } else {
                content_buff.push_str(&str_buff[..end]);
                str_buff = &str_buff[end..];
            }
        }

        if !content_buff.is_empty() || !str_buff.is_empty() {
            let raw = format!("{}{}", content_buff, str_buff);
            elem_buff.push(Element::new(self.pattern.name, self.pattern.value, raw));
        }

        elem_buff
    }
}

#[derive(Debug, Clone)]
pub struct Pattern {
    name: &'static str,
    value: fn(&str, PositionMarker) -> ErasedSegment,
    kind: SearchPatternKind,
}

#[derive(Debug, Clone)]
pub enum SearchPatternKind {
    String(&'static str),
    Regex(Regex),
}

impl Pattern {
    pub const fn string(
        name: &'static str,
        template: &'static str,
        constructor: fn(&str, PositionMarker) -> ErasedSegment,
    ) -> Self {
        Self { name, value: constructor, kind: SearchPatternKind::String(template) }
    }

    pub fn regex(
        name: &'static str,
        regex: &'static str,
        constructor: fn(&str, PositionMarker) -> ErasedSegment,
    ) -> Self {
        Self {
            name,
            value: constructor,
            kind: SearchPatternKind::Regex(Regex::new(regex).unwrap()),
        }
    }

    fn matches<'a>(&self, forward_string: &'a str) -> Option<&'a str> {
        match self.kind {
            SearchPatternKind::String(template) => {
                if forward_string.starts_with(template) {
                    return Some(template);
                }
            }
            SearchPatternKind::Regex(ref template) => {
                if let Ok(Some(matched)) = template.find(forward_string) {
                    if matched.start() == 0 {
                        return Some(matched.as_str());
                    }
                }
            }
        };

        None
    }

    fn search(&self, forward_string: &str) -> Option<Range<usize>> {
        match &self.kind {
            SearchPatternKind::String(template) => {
                forward_string.find(template).map(|start| start..start + template.len())
            }
            SearchPatternKind::Regex(template) => {
                if let Ok(Some(matched)) = template.find(forward_string) {
                    return Some(matched.range());
                }
                None
            }
        }
    }
}

/// The Lexer class actually does the lexing step.
pub struct Lexer<'a> {
    config: &'a FluffConfig,
    last_resort_lexer: Matcher,
}

pub enum StringOrTemplate<'a> {
    String(&'a str),
    Template(TemplatedFile),
}

impl<'a> Lexer<'a> {
    /// Create a new lexer.
    pub fn new(config: &'a FluffConfig, _dialect: Option<Dialect>) -> Self {
        Lexer {
            config,
            last_resort_lexer: Matcher::regex("<unlexable>", r"[^\t\n.]*", |_, _| unimplemented!()),
        }
    }

    pub fn lex(
        &self,
        raw: StringOrTemplate,
    ) -> Result<(Vec<ErasedSegment>, Vec<SQLLexError>), ValueError> {
        // Make sure we've got a string buffer and a template regardless of what was
        // passed in.

        let template;
        let mut str_buff = match raw {
            StringOrTemplate::String(s) => {
                template = TemplatedFile::from_string(s.into());
                s
            }
            StringOrTemplate::Template(slot) => {
                template = slot;
                template.templated_str.as_ref().unwrap()
            }
        };

        // Lex the string to get a tuple of LexedElement
        let mut element_buffer: Vec<Element> = Vec::new();
        let lexer_matchers = self.config.get_dialect().lexer_matchers();

        loop {
            let mut res = Lexer::lex_match(str_buff, lexer_matchers);
            element_buffer.append(&mut res.elements);

            if res.forward_string.is_empty() {
                break;
            }

            // If we STILL can't match, then just panic out.
            let mut resort_res = self.last_resort_lexer.matches(str_buff);
            if !resort_res.elements.is_empty() {
                break;
            }

            str_buff = resort_res.forward_string;
            element_buffer.append(&mut resort_res.elements);
        }

        // Map tuple LexedElement to list of TemplateElement.
        // This adds the template_slice to the object.
        let templated_buffer = Lexer::map_template_slices(element_buffer, &template);
        // Turn lexed elements into segments.
        let segments = self.elements_to_segments(templated_buffer, &template);

        Ok((segments, Vec::new()))
    }

    /// Generate any lexing errors for any un-lex-ables.
    ///
    /// TODO: Taking in an iterator, also can make the typing better than use
    /// unwrap.
    #[allow(dead_code)]
    fn violations_from_segments(segments: Vec<impl Segment>) -> Vec<SQLLexError> {
        segments
            .into_iter()
            .filter(|s| s.is_type(SyntaxKind::Unlexable))
            .map(|s| {
                SQLLexError::new(
                    format!(
                        "Unable to lex characters: {}",
                        s.raw().chars().take(10).collect::<String>()
                    ),
                    s.get_position_marker().unwrap(),
                )
            })
            .collect()
    }

    /// Iteratively match strings using the selection of sub-matchers.
    fn lex_match<'b>(mut forward_string: &'b str, lexer_matchers: &[Matcher]) -> Match<'b> {
        let mut elem_buff = Vec::new();
        'main: loop {
            if forward_string.is_empty() {
                return Match { forward_string, elements: elem_buff };
            }

            for matcher in lexer_matchers {
                let mut match_result = matcher.matches(forward_string);

                if !match_result.elements.is_empty() {
                    elem_buff.append(&mut match_result.elements);
                    forward_string = match_result.forward_string;
                    continue 'main;
                }
            }

            return Match { forward_string, elements: elem_buff };
        }
    }

    /// Create a tuple of TemplateElement from a tuple of LexedElement.
    ///
    /// This adds slices in the templated file to the original lexed
    /// elements. We'll need this to work out the position in the source
    /// file.
    /// TODO Can this vec be turned into an iterator and return iterator to make
    /// lazy?
    fn map_template_slices<'b>(
        elements: Vec<Element<'b>>,
        template: &TemplatedFile,
    ) -> Vec<TemplateElement<'b>> {
        let mut idx = 0;
        let mut templated_buff: Vec<TemplateElement> = Vec::with_capacity(elements.len());

        for element in elements {
            let template_slice = offset_slice(idx, element.text.len());
            idx += element.text.len();

            let templated_string = template.get_templated_string().unwrap();
            if templated_string[template_slice.clone()] != element.text {
                panic!(
                    "Template and lexed elements do not match. This should never happen {:?} != \
                     {:?}",
                    element.text, &templated_string[template_slice]
                );
            }

            templated_buff.push(TemplateElement::from_element(element, template_slice));
        }

        templated_buff
    }

    /// Convert a tuple of lexed elements into a tuple of segments.

    fn elements_to_segments(
        &self,
        elements: Vec<TemplateElement>,
        templated_file: &TemplatedFile,
    ) -> Vec<ErasedSegment> {
        let mut segments = iter_segments(elements, templated_file);

        // Add an end of file marker
        let position_maker = segments
            .last()
            .map(|segment| segment.get_position_marker().unwrap().end_point_marker())
            .unwrap_or_else(|| {
                PositionMarker::from_point(0, 0, templated_file.clone(), None, None)
            });
        segments.push(EndOfFile::create(position_maker));

        segments
    }
}

fn iter_segments(
    lexed_elements: Vec<TemplateElement>,
    templated_file: &TemplatedFile,
) -> Vec<ErasedSegment> {
    let mut result = Vec::with_capacity(lexed_elements.len());
    // An index to track where we've got to in the templated file.
    let tfs_idx = 0;
    // We keep a map of previous block locations in case they re-occur.
    // let block_stack = BlockTracker()
    let templated_file_slices = &templated_file.sliced_file;

    // Now work out source slices, and add in template placeholders.
    for element in lexed_elements.into_iter() {
        let consumed_element_length = 0;
        let mut stashed_source_idx = None;

        for (mut tfs_idx, tfs) in templated_file_slices
            .iter()
            .skip(tfs_idx)
            .enumerate()
            .map(|(i, tfs)| (i + tfs_idx, tfs))
        {
            // Is it a zero slice?
            if is_zero_slice(tfs.templated_slice.clone()) {
                let _slice = if tfs_idx + 1 < templated_file_slices.len() {
                    templated_file_slices[tfs_idx + 1].clone().into()
                } else {
                    None
                };

                _handle_zero_length_slice();

                continue;
            }

            if tfs.slice_type == "literal" {
                let tfs_offset = tfs.source_slice.start - tfs.templated_slice.start;

                // NOTE: Greater than OR EQUAL, to include the case of it matching
                // length exactly.
                if element.template_slice.end <= tfs.templated_slice.end {
                    let slice_start = stashed_source_idx.unwrap_or_else(|| {
                        element.template_slice.start + consumed_element_length + tfs_offset
                    });

                    result.push(element.to_segment(
                        PositionMarker::new(
                            slice_start..element.template_slice.end + tfs_offset,
                            element.template_slice.clone(),
                            templated_file.clone(),
                            None,
                            None,
                        ),
                        Some(consumed_element_length..element.raw.len()),
                    ));

                    // If it was an exact match, consume the templated element too.
                    #[allow(unused_assignments)]
                    if element.template_slice.end == tfs.templated_slice.end {
                        tfs_idx += 1
                    }
                    // In any case, we're done with this element. Move on
                    break;
                } else if element.template_slice.start == tfs.templated_slice.end {
                    // Did we forget to move on from the last tfs and there's
                    // overlap?
                    // NOTE: If the rest of the logic works, this should never
                    // happen.
                    // lexer_logger.debug("     NOTE: Missed Skip")  # pragma: no cover
                    continue;
                } else {
                    // This means that the current lexed element spans across
                    // multiple templated file slices.
                    // lexer_logger.debug("     Consuming whole spanning literal")
                    // This almost certainly means there's a templated element
                    // in the middle of a whole lexed element.

                    // What we do here depends on whether we're allowed to split
                    // lexed elements. This is basically only true if it's whitespace.
                    // NOTE: We should probably make this configurable on the
                    // matcher object, but for now we're going to look for the
                    // name of the lexer.
                    if element.matcher.name == "whitespace" {
                        if stashed_source_idx.is_some() {
                            panic!("Found literal whitespace with stashed idx!")
                        }

                        let incremental_length =
                            tfs.templated_slice.end - element.template_slice.start;
                        result.push(element.to_segment(
                            PositionMarker::new(
                                element.template_slice.start + consumed_element_length + tfs_offset
                                    ..tfs.templated_slice.end + tfs_offset,
                                element.template_slice.clone(),
                                templated_file.clone(),
                                None,
                                None,
                            ),
                            offset_slice(consumed_element_length, incremental_length).into(),
                        ));
                    } else {
                        // We can't split it. We're going to end up yielding a segment
                        // which spans multiple slices. Stash the type, and if we haven't
                        // set the start yet, stash it too.
                        // lexer_logger.debug("     Spilling over literal slice.")
                        if stashed_source_idx.is_none() {
                            stashed_source_idx = (element.template_slice.start + tfs_idx).into();
                            // lexer_logger.debug(
                            //     "     Stashing a source start. %s", stashed_source_idx
                            // )
                            continue;
                        }
                    }
                }
            } else if matches!(tfs.slice_type.as_str(), "templated" | "block_start") {
                unimplemented!();
            }
        }
    }

    result
}

fn _handle_zero_length_slice() {
    // impl me
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert that a matcher does or doesn't work on a string.
    ///
    /// The optional `matchstring` argument, which can optionally
    /// be None, allows to either test positive matching of a
    /// particular string or negative matching (that it explicitly)
    /// doesn't match.
    fn assert_matches(in_string: &str, matcher: &Matcher, match_string: Option<&str>) {
        let res = matcher.matches(in_string);
        if let Some(match_string) = match_string {
            assert_eq!(res.forward_string, &in_string[match_string.len()..]);
            assert_eq!(res.elements.len(), 1);
            assert_eq!(res.elements[0].text, match_string);
        } else {
            assert_eq!(res.forward_string, in_string);
            assert_eq!(res.elements.len(), 0);
        }
    }

    #[test]
    fn test_parser_lexer_trim_post_subdivide() {
        let matcher: Vec<Matcher> = vec![
            Matcher::regex(
                "function_script_terminator",
                r";\s+(?!\*)\/(?!\*)|\s+(?!\*)\/(?!\*)",
                |_, _| unimplemented!(),
            )
            .subdivider(Pattern::string("semicolon", ";", |_, _| unimplemented!()))
            .post_subdivide(Pattern::regex(
                "newline",
                r"(\n|\r\n)+",
                |_, _| unimplemented!(),
            )),
        ];

        let res = Lexer::lex_match(";\n/\n", &matcher);
        assert_eq!(res.elements[0].text, ";");
        assert_eq!(res.elements[1].text, "\n");
        assert_eq!(res.elements[2].text, "/");
        assert_eq!(res.elements.len(), 3);
    }

    /// Test the RegexLexer.
    #[test]
    fn test_parser_lexer_regex() {
        let tests = &[
            ("fsaljk", "f", "f"),
            ("fsaljk", r"f", "f"),
            ("fsaljk", r"[fas]*", "fsa"),
            // Matching whitespace segments
            ("   \t   fsaljk", r"[^\S\r\n]*", "   \t   "),
            // Matching whitespace segments (with a newline)
            ("   \t \n  fsaljk", r"[^\S\r\n]*", "   \t "),
            // Matching quotes containing stuff
            ("'something boring'   \t \n  fsaljk", r"'[^']*'", "'something boring'"),
            (
                "' something exciting \t\n '   \t \n  fsaljk",
                r"'[^']*'",
                "' something exciting \t\n '",
            ),
        ];

        for (raw, reg, res) in tests {
            let matcher = Matcher::regex("test", reg, |_, _| unimplemented!());

            assert_matches(raw, &matcher, Some(res));
        }
    }

    /// Test the lexer string
    #[test]
    fn test_parser_lexer_string() {
        let matcher = Matcher::string("dot", ".", |_, _| unimplemented!());

        assert_matches(".fsaljk", &matcher, Some("."));
        assert_matches("fsaljk", &matcher, None);
    }

    /// Test the RepeatedMultiMatcher
    #[test]
    fn test_parser_lexer_lex_match() {
        let matchers: Vec<Matcher> = vec![
            Matcher::string("dot", ".", |_, _| unimplemented!()),
            Matcher::regex("test", "#[^#]*#", |_, _| unimplemented!()),
        ];

        let res = Lexer::lex_match("..#..#..#", &matchers);

        assert_eq!(res.forward_string, "#");
        assert_eq!(res.elements.len(), 5);
        assert_eq!(res.elements[2].text, "#..#");
    }
}
