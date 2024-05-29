use std::mem::take;

use itertools::Itertools;

use super::config::ReflowConfig;
use super::depth_map::DepthMap;
use super::elements::{ReflowBlock, ReflowElement, ReflowPoint, ReflowSequenceType};
use super::rebreak::rebreak_sequence;
use super::reindent::{construct_single_indent, lint_indent_points, lint_line_length};
use crate::core::config::FluffConfig;
use crate::core::parser::segments::base::{ErasedSegment, SegmentExt};
use crate::core::rules::base::{LintFix, LintResult};

pub struct ReflowSequence {
    root_segment: ErasedSegment,
    elements: ReflowSequenceType,
    lint_results: Vec<LintResult>,
    reflow_config: ReflowConfig,
    depth_map: DepthMap,
}

impl ReflowSequence {
    pub fn raw(&self) -> String {
        self.elements.iter().map(|it| it.raw()).join("")
    }

    pub fn results(self) -> Vec<LintResult> {
        self.lint_results
    }

    pub fn fixes(self) -> Vec<LintFix> {
        self.results().into_iter().flat_map(|result| result.fixes).collect()
    }

    pub fn from_root(root_segment: ErasedSegment, config: &FluffConfig) -> Self {
        let depth_map = DepthMap::from_parent(&root_segment).into();

        Self::from_raw_segments(root_segment.get_raw_segments(), root_segment, config, depth_map)
    }

    pub fn from_raw_segments(
        segments: Vec<ErasedSegment>,
        root_segment: ErasedSegment,
        config: &FluffConfig,
        depth_map: Option<DepthMap>,
    ) -> Self {
        let reflow_config = ReflowConfig::from_fluff_config(config);
        let depth_map = depth_map.unwrap_or_else(|| {
            DepthMap::from_raws_and_root(segments.clone(), root_segment.clone())
        });
        let elements = Self::elements_from_raw_segments(segments, &depth_map, &reflow_config);

        Self { root_segment, elements, lint_results: Vec::new(), reflow_config, depth_map }
    }

    fn elements_from_raw_segments(
        segments: Vec<ErasedSegment>,
        depth_map: &DepthMap,
        reflow_config: &ReflowConfig,
    ) -> Vec<ReflowElement> {
        let mut elem_buff = Vec::new();
        let mut seg_buff = Vec::new();

        for seg in segments {
            // NOTE: end_of_file is block-like rather than point-like.
            // This is to facilitate better evaluation of the ends of files.
            // NOTE: This also allows us to include literal placeholders for
            // whitespace only strings.
            if matches!(seg.get_type(), "whitespace" | "newline" | "indent" | "dedent") {
                // Add to the buffer and move on.
                seg_buff.push(seg);
                continue;
            } else if !elem_buff.is_empty() || !seg_buff.is_empty() {
                // There are elements. The last will have been a block.
                // Add a point before we add the block. NOTE: It may be empty.
                elem_buff.push(ReflowElement::Point(ReflowPoint::new(seg_buff.clone())));
            }

            // Add the block, with config info.
            let depth_info = depth_map.get_depth_info(&seg);
            elem_buff.push(ReflowElement::Block(ReflowBlock::from_config(
                vec![seg],
                reflow_config,
                depth_info,
            )));

            // Empty the buffer
            seg_buff.clear();
        }

        if !seg_buff.is_empty() {
            elem_buff.push(ReflowPoint::new(seg_buff).into());
        }

        elem_buff
    }

    pub fn from_around_target(
        target_segment: &ErasedSegment,
        root_segment: ErasedSegment,
        sides: &str,
        config: &FluffConfig,
    ) -> ReflowSequence {
        let all_raws = root_segment.get_raw_segments();
        let target_raws = target_segment.get_raw_segments();

        assert!(!target_raws.is_empty());

        let pre_idx = all_raws.iter().position(|x| x == &target_raws[0]).unwrap();
        let post_idx =
            all_raws.iter().position(|x| x == &target_raws[target_raws.len() - 1]).unwrap() + 1;

        let mut pre_idx = pre_idx;
        let mut post_idx = post_idx;

        if sides == "both" || sides == "before" {
            pre_idx -= 1;
            for i in (0..=pre_idx).rev() {
                if all_raws[i].is_code() {
                    pre_idx = i;
                    break;
                }
            }
        }

        if sides == "both" || sides == "after" {
            for (i, it) in all_raws.iter().enumerate().skip(post_idx) {
                if it.is_code() {
                    post_idx = i;
                    break;
                }
            }
            post_idx += 1;
        }

        let segments = &all_raws[pre_idx..post_idx];
        ReflowSequence::from_raw_segments(segments.to_vec(), root_segment, config, None)
    }

    pub fn insert(
        self,
        insertion: ErasedSegment,
        target: ErasedSegment,
        pos: &'static str,
    ) -> Self {
        let target_idx = self.find_element_idx_with(&target);

        let new_block = ReflowBlock::from_config(
            vec![insertion.clone()],
            &self.reflow_config,
            self.depth_map.get_depth_info(&target),
        );

        if pos == "before" {
            let mut new_elements = self.elements[..target_idx].to_vec();
            new_elements.push(new_block.into());
            new_elements.push(ReflowPoint::default().into());
            new_elements.extend_from_slice(&self.elements[target_idx..]);

            let new_lint_result = LintResult::new(
                target.clone().into(),
                vec![LintFix::create_before(target, vec![insertion])],
                None,
                None,
                None,
            );

            return ReflowSequence {
                root_segment: self.root_segment,
                elements: new_elements,
                lint_results: vec![new_lint_result],
                reflow_config: self.reflow_config,
                depth_map: self.depth_map,
            };
        }

        self
    }

    fn find_element_idx_with(&self, target: &ErasedSegment) -> usize {
        self.elements
            .iter()
            .position(|elem| elem.segments().contains(target))
            .unwrap_or_else(|| panic!("Target [{:?}] not found in ReflowSequence.", target))
    }

    pub fn without(self, target: &ErasedSegment) -> ReflowSequence {
        let removal_idx = self.find_element_idx_with(target);
        if removal_idx == 0 || removal_idx == self.elements.len() - 1 {
            panic!("Unexpected removal at one end of a ReflowSequence.");
        }
        if let ReflowElement::Point(_) = &self.elements[removal_idx] {
            panic!("Not expected removal of whitespace in ReflowSequence.");
        }
        let merged_point = ReflowPoint::new(
            [self.elements[removal_idx - 1].segments(), self.elements[removal_idx + 1].segments()]
                .concat(),
        );
        let mut new_elements = self.elements[..removal_idx - 1].to_vec();
        new_elements.push(ReflowElement::Point(merged_point));
        new_elements.extend_from_slice(&self.elements[removal_idx + 2..]);

        ReflowSequence {
            elements: new_elements,
            root_segment: self.root_segment.clone(),
            lint_results: vec![LintResult::new(
                target.clone().into(),
                vec![LintFix::delete(target.clone())],
                None,
                None,
                None,
            )],
            reflow_config: self.reflow_config,
            depth_map: self.depth_map,
        }
    }

    pub fn respace(mut self, strip_newlines: bool, filter: Filter) -> Self {
        let mut lint_results = take(&mut self.lint_results);
        let mut new_elements = Vec::new();

        for (point, pre, post) in self.iter_points_with_constraints() {
            let (new_lint_results, mut new_point) =
                point.respace_point(pre, post, lint_results.clone(), strip_newlines);

            let ignore = if new_point.segments.iter().any(|seg| seg.is_type("newline"))
                || post.as_ref().map_or(false, |p| p.class_types().contains("end_of_file"))
            {
                filter == Filter::Inline
            } else {
                filter == Filter::Newline
            };

            if ignore {
                new_point = point.clone();
            } else {
                lint_results = new_lint_results;
            }

            if let Some(pre_value) = pre {
                if new_elements.is_empty() || new_elements.last().unwrap() != pre_value {
                    new_elements.push(pre_value.clone().into());
                }
            }

            new_elements.push(new_point.into());

            if let Some(post) = post {
                new_elements.push(post.clone().into());
            }
        }

        self.elements = new_elements;
        self.lint_results = lint_results;

        self
    }

    pub fn rebreak(self) -> Self {
        if !self.lint_results.is_empty() {
            panic!("rebreak cannot currently handle pre-existing embodied fixes");
        }

        // Delegate to the rebreak algorithm
        let (elem_buff, lint_results) = rebreak_sequence(self.elements, self.root_segment.clone());

        ReflowSequence {
            root_segment: self.root_segment,
            elements: elem_buff,
            lint_results,
            reflow_config: self.reflow_config,
            depth_map: self.depth_map,
        }
    }

    // https://github.com/sqlfluff/sqlfluff/blob/baceed9907908e055b79ca50ce6203bcd7949f39/src/sqlfluff/utils/reflow/sequence.py#L397
    pub fn replace(&mut self, target: ErasedSegment, edit: &[ErasedSegment]) -> Self {
        let target_raws = target.get_raw_segments();

        let mut edit_raws: Vec<ErasedSegment> = Vec::new();

        for seg in edit {
            edit_raws.extend_from_slice(&seg.get_raw_segments());
        }

        let trim_amount = target.path_to(&target_raws[0]).len();

        for edit_raw in &edit_raws {
            self.depth_map.copy_depth_info(
                target_raws[0].clone(),
                edit_raw.clone(),
                trim_amount,
            );
        }

        let current_raws: Vec<ErasedSegment> =
            self.elements.iter().flat_map(|elem| elem.segments().iter().cloned()).collect();

        let start_idx = current_raws.iter().position(|s| *s == target_raws[0]).unwrap();
        let last_idx =
            current_raws.iter().position(|s| *s == *target_raws.last().unwrap()).unwrap();

        let new_elements = Self::elements_from_raw_segments(
            current_raws[..start_idx]
                .iter()
                .chain(edit_raws.iter())
                .chain(current_raws[last_idx + 1..].iter())
                .cloned()
                .collect(),
            &self.depth_map,
            &self.reflow_config,
        );

        ReflowSequence {
            elements: new_elements,
            root_segment: self.root_segment.clone(),
            reflow_config: self.reflow_config.clone(),
            depth_map: self.depth_map.clone(),
            lint_results: vec![LintResult::new(
                target.clone().into(),
                vec![LintFix::replace(target.clone(), edit.to_vec(), None)],
                None,
                None,
                None,
            )],
        }
    }

    pub fn reindent(self) -> Self {
        if !self.lint_results.is_empty() {
            panic!("reindent cannot currently handle pre-existing embodied fixes");
        }

        let single_indent = construct_single_indent("space", 4);

        let (elements, indent_results) =
            lint_indent_points(self.elements, &single_indent, <_>::default(), <_>::default());

        Self {
            root_segment: self.root_segment,
            elements,
            lint_results: indent_results,
            reflow_config: self.reflow_config,
            depth_map: self.depth_map,
        }
    }

    pub fn break_long_lines(self) -> Self {
        if !self.lint_results.is_empty() {
            panic!("break_long_lines cannot currently handle pre-existing embodied fixes");
        }

        let single_indent = construct_single_indent(
            &self.reflow_config.indent_unit,
            self.reflow_config.tab_space_size,
        );

        let (elements, length_results) = lint_line_length(
            &self.elements,
            self.root_segment.clone(),
            &single_indent,
            self.reflow_config.max_line_length,
            self.reflow_config.allow_implicit_indents,
            &self.reflow_config.trailing_comments,
        );

        ReflowSequence {
            root_segment: self.root_segment,
            elements,
            lint_results: length_results,
            reflow_config: self.reflow_config,
            depth_map: self.depth_map,
        }
    }

    fn iter_points_with_constraints(
        &self,
    ) -> impl Iterator<Item = (&ReflowPoint, Option<&ReflowBlock>, Option<&ReflowBlock>)> + '_ {
        self.elements.iter().enumerate().flat_map(|(idx, elem)| {
            if let ReflowElement::Point(elem) = elem {
                {
                    let mut pre = None;
                    let mut post = None;

                    if idx > 0 {
                        if let ReflowElement::Block(ref block) = self.elements[idx - 1] {
                            pre = Some(block);
                        }
                    }

                    if idx < self.elements.len() - 1 {
                        if let ReflowElement::Block(ref block) = self.elements[idx + 1] {
                            post = Some(block);
                        }
                    }

                    (elem, pre, post).into()
                }
            } else {
                None
            }
        })
    }

    pub fn elements(&self) -> &[ReflowElement] {
        &self.elements
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Inline,
    Newline,
}
