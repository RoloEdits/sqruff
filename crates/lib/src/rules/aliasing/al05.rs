use std::cell::RefCell;

use ahash::AHashMap;
use smol_str::SmolStr;

use crate::core::config::Value;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::AliasInfo;
use crate::core::dialects::init::DialectKind;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::ObjectReferenceLevel;
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::utils::analysis::query::Query;
use crate::utils::analysis::select::get_select_statement_info;
use crate::utils::functional::segments::Segments;

#[derive(Default, Clone)]
struct AL05Query {
    aliases: Vec<AliasInfo>,
    tbl_refs: Vec<SmolStr>,
}

#[derive(Debug, Default, Clone)]
pub struct RuleAL05;

impl Rule for RuleAL05 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL05.erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.unused"
    }

    fn description(&self) -> &'static str {
        "Tables should not be aliased if that alias is not used."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, alias `zoo` is not used.

```sql
SELECT
    a
FROM foo AS zoo
```

**Best practice**

Use the alias or remove it. An unused alias makes code harder to read without changing any functionality.

```sql
SELECT
    zoo.a
FROM foo AS zoo

-- Alternatively...

SELECT
    a
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut violations = Vec::new();
        let select_info = get_select_statement_info(&context.segment, context.dialect.into(), true);

        let Some(select_info) = select_info else {
            return Vec::new();
        };

        if select_info.table_aliases.is_empty() {
            return Vec::new();
        }

        let query = Query::from_segment(&context.segment, context.dialect, None);
        self.analyze_table_aliases(query.clone(), context.dialect);

        for alias in &RefCell::borrow(&query.inner).payload.aliases {
            if Self::is_alias_required(&alias.from_expression_element, context.dialect.name) {
                continue;
            }

            if alias.aliased
                && !RefCell::borrow(&query.inner).payload.tbl_refs.contains(&alias.ref_str)
            {
                let violation = self.report_unused_alias(alias.clone());
                violations.push(violation);
            }
        }

        violations
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}

impl RuleAL05 {
    #[allow(clippy::only_used_in_recursion)]
    fn analyze_table_aliases(&self, query: Query<AL05Query>, dialect: &Dialect) {
        let selectables = std::mem::take(&mut RefCell::borrow_mut(&query.inner).selectables);

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                RefCell::borrow_mut(&query.inner).payload.aliases.extend(select_info.table_aliases);

                for r in select_info.reference_buffer {
                    for tr in
                        r.extract_possible_references(ObjectReferenceLevel::Table, dialect.name)
                    {
                        Self::resolve_and_mark_reference(query.clone(), tr.part);
                    }
                }
            }
        }

        RefCell::borrow_mut(&query.inner).selectables = selectables;

        for child in query.children() {
            self.analyze_table_aliases(child, dialect);
        }
    }

    fn resolve_and_mark_reference(query: Query<AL05Query>, r#ref: String) {
        if RefCell::borrow(&query.inner).payload.aliases.iter().any(|it| it.ref_str == r#ref) {
            RefCell::borrow_mut(&query.inner).payload.tbl_refs.push(r#ref.into());
        } else if let Some(parent) = RefCell::borrow(&query.inner).parent.clone() {
            Self::resolve_and_mark_reference(parent, r#ref);
        }
    }

    fn is_alias_required(
        from_expression_element: &ErasedSegment,
        dialect_name: DialectKind,
    ) -> bool {
        for segment in from_expression_element
            .iter_segments(Some(const { SyntaxSet::new(&[SyntaxKind::Bracketed]) }), false)
        {
            if segment.is_type(SyntaxKind::TableExpression) {
                return if segment
                    .child(const { SyntaxSet::new(&[SyntaxKind::ValuesClause]) })
                    .is_some()
                {
                    matches!(dialect_name, DialectKind::Snowflake)
                } else {
                    segment
                        .iter_segments(
                            Some(const { SyntaxSet::new(&[SyntaxKind::Bracketed]) }),
                            false,
                        )
                        .iter()
                        .any(|seg| {
                            const {
                                SyntaxSet::new(&[
                                    SyntaxKind::SelectStatement,
                                    SyntaxKind::SetExpression,
                                    SyntaxKind::WithCompoundStatement,
                                ])
                            }
                            .contains(seg.get_type())
                        })
                };
            }
        }
        false
    }

    fn report_unused_alias(&self, alias: AliasInfo) -> LintResult {
        let mut fixes = vec![LintFix::delete(alias.alias_expression.clone().unwrap())];
        let to_delete = Segments::from_vec(alias.from_expression_element.segments().to_vec(), None)
            .reversed()
            .select::<fn(&ErasedSegment) -> bool>(
                None,
                Some(|it| it.is_whitespace() || it.is_meta()),
                alias.alias_expression.as_ref().unwrap().into(),
                None,
            );

        fixes.extend(to_delete.into_iter().map(LintFix::delete));

        LintResult::new(
            alias.segment,
            fixes,
            None,
            format!("Alias '{}' is never used in SELECT statement.", alias.ref_str).into(),
            None,
        )
    }
}
