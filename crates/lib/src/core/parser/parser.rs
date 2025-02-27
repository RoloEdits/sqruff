use super::context::ParseContext;
use super::helpers::check_still_complete;
use super::segments::base::{pos_marker, ErasedSegment};
use crate::core::config::FluffConfig;
use crate::core::errors::SQLParseError;
use crate::dialects::ansi::FileSegment;

/// Instantiates parsed queries from a sequence of lexed raw segments.
pub struct Parser<'a> {
    config: &'a FluffConfig,
    root_segment: FileSegment,
}

impl<'a> Parser<'a> {
    pub fn new(config: &'a FluffConfig, _dialect: Option<String>) -> Self {
        Self { config, root_segment: FileSegment }
    }

    pub fn config(&self) -> &FluffConfig {
        self.config
    }

    pub fn parse(
        &self,
        segments: &[ErasedSegment],
        f_name: Option<String>,
        parse_statistics: bool,
    ) -> Result<Option<ErasedSegment>, SQLParseError> {
        if segments.is_empty() {
            // This should normally never happen because there will usually
            // be an end_of_file segment. It would probably only happen in
            // api use cases.
            return Ok(None);
        }

        // NOTE: This is the only time we use the parse context not in the
        // context of a context manager. That's because it's the initial
        // instantiation.
        let mut parse_cx = ParseContext::from_config(self.config);
        // Kick off parsing with the root segment. The BaseFileSegment has
        // a unique entry point to facilitate exaclty this. All other segments
        // will use the standard .match()/.parse() route.
        let mut root = self.root_segment.root_parse(
            parse_cx.dialect().name,
            segments,
            &mut parse_cx,
            f_name,
        )?;

        // FIXME: remove hack
        let pos = pos_marker(root.segments());
        root.get_mut().set_position_marker(pos.into());

        // Basic Validation, that we haven't dropped anything.
        check_still_complete(segments, &[root.clone()], &[]);

        if parse_statistics {
            unimplemented!();
        }

        Ok(root.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::linter::Linter;

    #[test]
    #[ignore]
    fn test_parser_parse_error() {
        let in_str = "SELECT ;".to_string();
        let config = FluffConfig::new(<_>::default(), None, None);
        let linter = Linter::new(config, None, None);

        let _ = linter.parse_string(&in_str, None, None, None);
    }
}
