use std::sync::Arc;

use super::sparksql_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};
use crate::core::dialects::base::Dialect;
use crate::core::dialects::init::DialectKind;
use crate::core::parser::grammar::anyof::{any_set_of, one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::{Anything, Ref};
use crate::core::parser::grammar::conditional::Conditional;
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::Matcher;
use crate::core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, Segment, SymbolSegment,
    SymbolSegmentNewArgs,
};
use crate::core::parser::segments::bracketed::BracketedSegmentMatcher;
use crate::core::parser::segments::meta::MetaSegment;
use crate::core::parser::types::ParseMode;
use crate::dialects::ansi::NodeMatcher;
use crate::dialects::{ansi, SyntaxKind};
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

pub fn sparksql_dialect() -> Dialect {
    let ansi_dialect = ansi::raw_dialect();
    let hive_dialect = super::hive::raw_dialect();
    let mut sparksql_dialect = ansi_dialect;
    sparksql_dialect.name = DialectKind::Sparksql;

    sparksql_dialect.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"(--)[^\n]*", |slice, marker| {
            CommentSegment::create(
                slice,
                marker.into(),
                CommentSegmentNewArgs { r#type: SyntaxKind::InlineComment, trim_start: Some(vec!["--"]) },
            )
        }),
        Matcher::regex("equals", r"==|<=>|=", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::RawComparisonOperator, ..<_>::default() },
            )
        }),
        Matcher::regex("back_quote", r"`([^`]|``)*`", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::BackQuote, ..<_>::default() },
            )
        }),
        Matcher::regex("numeric_literal", r#"(?>(?>\d+\.\d+|\d+\.|\.\d+)([eE][+-]?\d+)?([dDfF]|BD|bd)?|\d+[eE][+-]?\d+([dDfF]|BD|bd)?|\d+([dDfFlLsSyY]|BD|bd)?)((?<=\.)|(?=\b))"#, |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::NumericLiteral, ..<_>::default() },
            )
        }),
    ]);

    sparksql_dialect.insert_lexer_matchers(
        vec![
            Matcher::regex("bytes_single_quote", r"X'([^'\\]|\\.)*'", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs {
                        code_type: SyntaxKind::BytesSingleQuote,
                        ..Default::default()
                    },
                )
            }),
            Matcher::regex("bytes_double_quote", r#"X"([^"\\]|\\.)*""#, |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs {
                        code_type: SyntaxKind::BytesDoubleQuote,
                        ..Default::default()
                    },
                )
            }),
        ],
        "single_quote",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![
            Matcher::regex("bytes_single_quote", r"X'([^'\\]|\\.)*'", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs {
                        code_type: SyntaxKind::BytesSingleQuote,
                        ..Default::default()
                    },
                )
            }),
            Matcher::regex("bytes_double_quote", r#"X"([^"\\]|\\.)*""#, |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs {
                        code_type: SyntaxKind::BytesDoubleQuote,
                        ..Default::default()
                    },
                )
            }),
        ],
        "single_quote",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::regex("at_sign_literal", r"@\w*", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::AtSignLiteral, ..Default::default() },
            )
        })],
        "word",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![
            Matcher::regex("file_literal", r#"[a-zA-Z0-9]*:?([a-zA-Z0-9\-_\.]*(/|\\)){2,}((([a-zA-Z0-9\-_\.]*(:|\?|=|&)[a-zA-Z0-9\-_\.]*)+)|([a-zA-Z0-9\-_\.]*\.[a-z]+))"#, |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: SyntaxKind::FileLiteral, ..Default::default() },
                )
            }),
        ],
        "newline",
    );

    sparksql_dialect.sets_mut("bare_functions").clear();
    sparksql_dialect.sets_mut("bare_functions").extend([
        "CURRENT_DATE",
        "CURRENT_TIMESTAMP",
        "CURRENT_USER",
    ]);

    sparksql_dialect.sets_mut("datetime_units").clear();
    sparksql_dialect.sets_mut("datetime_units").extend([
        "YEAR",
        "YEARS",
        "YYYY",
        "YY",
        "QUARTER",
        "QUARTERS",
        "MONTH",
        "MONTHS",
        "MON",
        "MM",
        "WEEK",
        "WEEKS",
        "DAY",
        "DAYS",
        "DD",
        "HOUR",
        "HOURS",
        "MINUTE",
        "MINUTES",
        "SECOND",
        "SECONDS",
        "MILLISECOND",
        "MILLISECONDS",
        "MICROSECOND",
        "MICROSECONDS",
    ]);

    sparksql_dialect.sets_mut("unreserved_keywords").extend(UNRESERVED_KEYWORDS);
    sparksql_dialect.sets_mut("reserved_keywords").extend(RESERVED_KEYWORDS);

    sparksql_dialect.update_bracket_sets(
        "angle_bracket_pairs",
        vec![("angle", "StartAngleBracketSegment", "EndAngleBracketSegment", false)],
    );

    sparksql_dialect.add([
        (
            "ComparisonOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("EqualsSegment"),
                Ref::new("EqualsSegment_a"),
                Ref::new("EqualsSegment_b"),
                Ref::new("GreaterThanSegment"),
                Ref::new("LessThanSegment"),
                Ref::new("GreaterThanOrEqualToSegment"),
                Ref::new("LessThanOrEqualToSegment"),
                Ref::new("NotEqualToSegment"),
                Ref::new("LikeOperatorSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("NOT"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FromClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("WHERE"),
                Ref::keyword("LIMIT"),
                Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("CLUSTER"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("DISTRIBUTE"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("SORT"), Ref::keyword("BY")]),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::new("SetOperatorSegment"),
                Ref::new("WithNoSchemaBindingClauseSegment"),
                Ref::new("WithDataClauseSegment"),
                Ref::keyword("KEYS")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TemporaryGrammar".into(),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("GLOBAL")]).config(|config| {
                    config.optional();
                }),
                one_of(vec_of_erased![Ref::keyword("TEMP"), Ref::keyword("TEMPORARY")])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(
                    SyntaxKind::SingleQuote,
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: SyntaxKind::QuotedLiteral },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                TypedParser::new(
                    SyntaxKind::DoubleQuote,
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: SyntaxKind::QuotedLiteral },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LiteralGrammar".into(),
            sparksql_dialect
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("BytesQuotedLiteralSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "NaturalJoinKeywordsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("NATURAL"),
                Ref::new("JoinTypeKeywords").optional()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LikeGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("LIKE"), Ref::keyword("ILIKE")]),
                    one_of(vec_of_erased![
                        Ref::keyword("ALL"),
                        Ref::keyword("ANY"),
                        Ref::keyword("SOME")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ]),
                Ref::keyword("RLIKE"),
                Ref::keyword("REGEXP")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                Ref::new("SingleQuotedIdentifierSegment"),
                Ref::new("BackQuotedIdentifierSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("CLUSTER"),
                        Ref::keyword("DISTRIBUTE"),
                        Ref::keyword("GROUP"),
                        Ref::keyword("ORDER"),
                        Ref::keyword("SORT")
                    ]),
                    Ref::keyword("BY")
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("DISTRIBUTE"), Ref::keyword("BY")]),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::keyword("OVERLAPS"),
                Ref::keyword("APPLY")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ORDER"),
                        Ref::keyword("DISTRIBUTE"),
                        Ref::keyword("CLUSTER"),
                        Ref::keyword("SORT")
                    ]),
                    Ref::keyword("BY")
                ]),
                Ref::keyword("LIMIT"),
                Ref::keyword("HAVING"),
                Ref::keyword("WINDOW")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ORDER"),
                        Ref::keyword("CLUSTER"),
                        Ref::keyword("DISTRIBUTE"),
                        Ref::keyword("SORT")
                    ]),
                    Ref::keyword("BY")
                ]),
                Ref::keyword("LIMIT"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ArithmeticBinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("PlusSegment"),
                Ref::new("MinusSegment"),
                Ref::new("DivideSegment"),
                Ref::new("MultiplySegment"),
                Ref::new("ModuloSegment"),
                Ref::new("BitwiseAndSegment"),
                Ref::new("BitwiseOrSegment"),
                Ref::new("BitwiseXorSegment"),
                Ref::new("BitwiseLShiftSegment"),
                Ref::new("BitwiseRShiftSegment"),
                Ref::new("DivBinaryOperatorSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("ArithmeticBinaryOperatorGrammar"),
                Ref::new("StringBinaryOperatorGrammar"),
                Ref::new("BooleanBinaryOperatorGrammar"),
                Ref::new("ComparisonOperatorGrammar"),
                Ref::new("RightArrowOperator")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AccessorGrammar".into(),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("ArrayAccessorSegment"),
                Ref::new("SemiStructuredAccessorSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ObjectReferenceTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("AS"),
                Ref::keyword("USING"),
                Ref::new("CommaSegment"),
                Ref::new("CastOperatorSegment"),
                Ref::new("StartSquareBracketSegment"),
                Ref::new("StartBracketSegment"),
                Ref::new("BinaryOperatorGrammar"),
                Ref::new("DelimiterGrammar"),
                Ref::new("JoinLikeClauseGrammar"),
                BracketedSegmentMatcher::new()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionContentsExpressionGrammar".into(),
            one_of(vec_of_erased![Ref::new("ExpressionSegment"), Ref::new("StarSegment")])
                .to_matchable()
                .into(),
        ),
    ]);
    sparksql_dialect.add([
        (
            "FileLiteralSegment".into(),
            TypedParser::new(
                SyntaxKind::FileLiteral,
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::FileLiteral },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "BackQuotedIdentifierSegment".into(),
            TypedParser::new(
                SyntaxKind::BackQuote,
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::QuotedIdentifier },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "NakedSemiStructuredElementSegment".into(),
            RegexParser::new(
                "[A-Z0-9_]*",
                |segment: &dyn Segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: SyntaxKind::SemiStructuredElement,
                            ..CodeSegmentNewArgs::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "QuotedSemiStructuredElementSegment".into(),
            TypedParser::new(
                SyntaxKind::SingleQuote,
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::SemiStructuredElement },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "RightArrowOperator".into(),
            StringParser::new(
                "->",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::BinaryOperator },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "BinaryfileKeywordSegment".into(),
            StringParser::new(
                "BINARYFILE",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::FileFormat },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "JsonfileKeywordSegment".into(),
            StringParser::new(
                "JSONFILE",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::FileFormat },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "RcfileKeywordSegment".into(),
            StringParser::new(
                "RCFILE",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::FileFormat },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SequencefileKeywordSegment".into(),
            StringParser::new(
                "SEQUENCEFILE",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::FileFormat },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "TextfileKeywordSegment".into(),
            StringParser::new(
                "TEXTFILE",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::FileFormat },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "StartAngleBracketSegment".into(),
            StringParser::new(
                "<",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::StartAngleBracket },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndAngleBracketSegment".into(),
            StringParser::new(
                ">",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::EndAngleBracket },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EqualsSegment_a".into(),
            StringParser::new(
                "==",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::ComparisonOperator },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EqualsSegment_b".into(),
            StringParser::new(
                "<=>",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::ComparisonOperator },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "FileKeywordSegment".into(),
            MultiStringParser::new(
                vec!["FILE".into(), "FILES".into()],
                |segment: &dyn Segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: SyntaxKind::FileKeyword,
                            ..CodeSegmentNewArgs::default()
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "JarKeywordSegment".into(),
            MultiStringParser::new(
                vec!["JAR".into(), "JARS".into()],
                |segment: &dyn Segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: SyntaxKind::FileKeyword,
                            ..CodeSegmentNewArgs::default()
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "NoscanKeywordSegment".into(),
            StringParser::new(
                "NOSCAN",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::Keyword },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "WhlKeywordSegment".into(),
            StringParser::new(
                "WHL",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::FileKeyword },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        ("CommentGrammar".into(), hive_dialect.grammar("CommentGrammar").into()),
        ("LocationGrammar".into(), hive_dialect.grammar("LocationGrammar").into()),
        ("SerdePropertiesGrammar".into(), hive_dialect.grammar("SerdePropertiesGrammar").into()),
        ("StoredAsGrammar".into(), hive_dialect.grammar("StoredAsGrammar").into()),
        ("StoredByGrammar".into(), hive_dialect.grammar("StoredByGrammar").into()),
        ("StorageFormatGrammar".into(), hive_dialect.grammar("StorageFormatGrammar").into()),
        ("TerminatedByGrammar".into(), hive_dialect.grammar("TerminatedByGrammar").into()),
        (
            "PropertyGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("PropertyNameSegment"),
                Ref::new("EqualsSegment").optional(),
                one_of(vec_of_erased![
                    Ref::new("LiteralGrammar"),
                    Ref::new("SingleIdentifierGrammar")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PropertyNameListGrammar".into(),
            Delimited::new(vec_of_erased![Ref::new("PropertyNameSegment")]).to_matchable().into(),
        ),
        (
            "BracketedPropertyNameListGrammar".into(),
            Bracketed::new(vec_of_erased![Ref::new("PropertyNameListGrammar")])
                .to_matchable()
                .into(),
        ),
        (
            "PropertyListGrammar".into(),
            Delimited::new(vec_of_erased![Ref::new("PropertyGrammar")]).to_matchable().into(),
        ),
        (
            "BracketedPropertyListGrammar".into(),
            Bracketed::new(vec_of_erased![Ref::new("PropertyListGrammar")]).to_matchable().into(),
        ),
        (
            "OptionsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("OPTIONS"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BucketSpecGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("ClusteredBySpecGrammar"),
                Ref::new("SortedBySpecGrammar").optional(),
                Ref::keyword("INTO"),
                Ref::new("NumericLiteralSegment"),
                Ref::keyword("BUCKETS")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ClusteredBySpecGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CLUSTERED"),
                Ref::keyword("BY"),
                Ref::new("BracketedColumnReferenceListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DatabasePropertiesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DBPROPERTIES"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DataSourcesV2FileTypeGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("AVRO"),
                Ref::keyword("CSV"),
                Ref::keyword("JSON"),
                Ref::keyword("PARQUET"),
                Ref::keyword("ORC"),
                Ref::keyword("DELTA"),
                Ref::keyword("CSV"),
                Ref::keyword("ICEBERG"),
                Ref::keyword("TEXT"),
                Ref::keyword("BINARYFILE")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FileFormatGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("DataSourcesV2FileTypeGrammar"),
                Ref::keyword("SEQUENCEFILE"),
                Ref::keyword("TEXTFILE"),
                Ref::keyword("RCFILE"),
                Ref::keyword("JSONFILE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INPUTFORMAT"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::keyword("OUTPUTFORMAT"),
                    Ref::new("QuotedLiteralSegment")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TimestampAsOfGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TIMESTAMP"),
                Ref::keyword("AS"),
                Ref::keyword("OF"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "VersionAsOfGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("VERSION"),
                Ref::keyword("AS"),
                Ref::keyword("OF"),
                Ref::new("NumericLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StartHintSegment".into(),
            StringParser::new(
                "/*+",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::StartHint },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndHintSegment".into(),
            StringParser::new(
                "*/",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::EndHint },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "PartitionSpecGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Sequence::new(vec_of_erased![Ref::keyword("PARTITIONED"), Ref::keyword("BY")])
                ]),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![
                        Ref::new("ColumnDefinitionSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("EqualsSegment").optional(),
                            Ref::new("LiteralGrammar").optional(),
                            Ref::new("CommentGrammar").optional()
                        ]),
                        Ref::new("IcebergTransformationSegment").optional()
                    ]
                )])])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PartitionFieldGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("PARTITION"),
                Ref::keyword("FIELD"),
                Delimited::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("ColumnDefinitionSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("EqualsSegment").optional(),
                        Ref::new("LiteralGrammar").optional(),
                        Ref::new("CommentGrammar").optional()
                    ]),
                    Ref::new("IcebergTransformationSegment").optional()
                ])]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH").optional(),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("ColumnDefinitionSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("EqualsSegment").optional(),
                            Ref::new("LiteralGrammar").optional(),
                            Ref::new("CommentGrammar").optional()
                        ]),
                        Ref::new("IcebergTransformationSegment").optional()
                    ])])
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::new("NakedIdentifierSegment")
                ])
                .config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PropertiesNakedIdentifierSegment".into(),
            RegexParser::new(
                "[A-Z0-9]*[A-Z][A-Z0-9]*",
                |segment: &dyn Segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: SyntaxKind::PropertiesNakedIdentifier,
                            ..CodeSegmentNewArgs::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ResourceFileGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("JarKeywordSegment"),
                Ref::new("WhlKeywordSegment"),
                Ref::new("FileKeywordSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ResourceLocationGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Ref::new("ResourceFileGrammar"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SortedBySpecGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SORTED"),
                Ref::keyword("BY"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")]).config(
                            |config| {
                                config.optional();
                            }
                        )
                    ]
                )])])
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable()
            .into(),
        ),
        (
            "UnsetTablePropertiesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("UNSET"),
                Ref::keyword("TBLPROPERTIES"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("BracketedPropertyNameListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TablePropertiesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TBLPROPERTIES"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BytesQuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(
                    SyntaxKind::BytesSingleQuote,
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: SyntaxKind::BytesQuotedLiteral },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                TypedParser::new(
                    SyntaxKind::BytesDoubleQuote,
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: SyntaxKind::BytesQuotedLiteral },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "JoinTypeKeywords".into(),
            one_of(vec_of_erased![
                Ref::keyword("CROSS"),
                Ref::keyword("INNER"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("FULL"),
                        Ref::keyword("LEFT"),
                        Ref::keyword("RIGHT")
                    ]),
                    Ref::keyword("OUTER").optional()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LEFT").optional(),
                    Ref::keyword("SEMI")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LEFT").optional(),
                    Ref::keyword("ANTI")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AtSignLiteralSegment".into(),
            TypedParser::new(
                SyntaxKind::AtSignLiteral,
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: SyntaxKind::AtSignLiteral },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SignedQuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(
                    SyntaxKind::SingleQuote,
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: SyntaxKind::SignedQuotedLiteral },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                TypedParser::new(
                    SyntaxKind::DoubleQuote,
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: SyntaxKind::SignedQuotedLiteral },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrRefreshGrammar".into(),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REFRESH")])
                .to_matchable()
                .into(),
        ),
        (
            "WidgetNameIdentifierSegment".into(),
            RegexParser::new(
                "[A-Z][A-Z0-9_]*",
                |segment: &dyn Segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: SyntaxKind::WidgetNameIdentifier,
                            ..CodeSegmentNewArgs::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "WidgetDefaultGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TableDefinitionSegment".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::new("OrReplaceGrammar"), Ref::new("OrRefreshGrammar")])
                    .config(|config| {
                        config.optional();
                    }),
                Ref::new("TemporaryGrammar").optional(),
                Ref::keyword("EXTERNAL").optional(),
                Ref::keyword("STREAMING").optional(),
                Ref::keyword("LIVE").optional(),
                Ref::keyword("TABLE"),
                Ref::new("IfNotExistsGrammar").optional(),
                one_of(vec_of_erased![
                    Ref::new("FileReferenceSegment"),
                    Ref::new("TableReferenceSegment")
                ]),
                one_of(vec_of_erased![
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("ColumnDefinitionSegment"),
                                Ref::new("GeneratedColumnDefinitionSegment")
                            ]),
                            Ref::new("CommentGrammar").optional()
                        ]
                    )])]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LIKE"),
                        one_of(vec_of_erased![
                            Ref::new("FileReferenceSegment"),
                            Ref::new("TableReferenceSegment")
                        ])
                    ])
                ])
                .config(|config| {
                    config.optional();
                }),
                Ref::new("UsingClauseSegment").optional(),
                any_set_of(vec_of_erased![
                    Ref::new("RowFormatClauseSegment"),
                    Ref::new("StoredAsGrammar"),
                    Ref::new("CommentGrammar"),
                    Ref::new("OptionsGrammar"),
                    Ref::new("PartitionSpecGrammar"),
                    Ref::new("BucketSpecGrammar")
                ])
                .config(|config| {
                    config.optional();
                }),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("LocationGrammar").optional(),
                    Ref::new("CommentGrammar").optional(),
                    Ref::new("TablePropertiesGrammar").optional()
                ]),
                MetaSegment::dedent(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS").optional(),
                    optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
                ])
                .config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::regex("start_hint", r"\/\*\+", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::BlockComment, ..Default::default() },
            )
        })],
        "block_comment",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::regex("end_hint", r"\*\/", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::EndHint, ..Default::default() },
            )
        })],
        "single_quote",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::string("end_hint", r"->", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: SyntaxKind::RightArrow, ..Default::default() },
            )
        })],
        "like_operator",
    );

    sparksql_dialect.add([
        (
            "SQLConfPropertiesSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SqlConfOption,
                Sequence::new(vec_of_erased![
                    StringParser::new(
                        "-",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: SyntaxKind::Dash },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    StringParser::new(
                        "v",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: SyntaxKind::SqlConfOption },
                            )
                        },
                        None,
                        false,
                        None,
                    )
                ])
                .config(|config| {
                    config.disallow_gaps();
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DivBinaryOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, Ref::keyword("DIV").to_matchable())
                .to_matchable()
                .into(),
        ),
        (
            "QualifyClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::QualifyClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("QUALIFY"),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.add([
        (
            "PrimitiveTypeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PrimitiveType,
                one_of(vec_of_erased![
                    Ref::keyword("BOOLEAN"),
                    Ref::keyword("TINYINT"),
                    Ref::keyword("LONG"),
                    Ref::keyword("SMALLINT"),
                    Ref::keyword("INT"),
                    Ref::keyword("INTEGER"),
                    Ref::keyword("BIGINT"),
                    Ref::keyword("FLOAT"),
                    Ref::keyword("REAL"),
                    Ref::keyword("DOUBLE"),
                    Ref::keyword("DATE"),
                    Ref::keyword("TIMESTAMP"),
                    Ref::keyword("STRING"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("CHAR"),
                            Ref::keyword("CHARACTER"),
                            Ref::keyword("VARCHAR"),
                            Ref::keyword("DECIMAL"),
                            Ref::keyword("DEC"),
                            Ref::keyword("NUMERIC")
                        ]),
                        Ref::new("BracketedArguments").optional()
                    ]),
                    Ref::keyword("BINARY"),
                    Ref::keyword("INTERVAL")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        ("ArrayTypeSegment".into(), hive_dialect.grammar("ArrayTypeSegment").into()),
        ("StructTypeSegment".into(), hive_dialect.grammar("StructTypeSegment").into()),
        ("StructTypeSchemaSegment".into(), hive_dialect.grammar("StructTypeSchemaSegment").into()),
    ]);

    sparksql_dialect.add([
        (
            "SemiStructuredAccessorSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SemiStructuredExpression,
                Sequence::new(vec_of_erased![
                    Ref::new("ColonSegment"),
                    one_of(vec_of_erased![
                        Ref::new("NakedSemiStructuredElementSegment"),
                        Bracketed::new(vec_of_erased![Ref::new(
                            "QuotedSemiStructuredElementSegment"
                        )])
                        .config(|config| {
                            config.bracket_type = "square";
                        })
                    ]),
                    Ref::new("ArrayAccessorSegment").optional(),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("DotSegment"),
                                Ref::new("ColonSegment")
                            ]),
                            one_of(vec_of_erased![
                                Ref::new("NakedSemiStructuredElementSegment"),
                                Bracketed::new(vec_of_erased![Ref::new(
                                    "QuotedSemiStructuredElementSegment"
                                )])
                                .config(|config| {
                                    config.bracket_type = "square";
                                })
                            ])
                        ]),
                        Ref::new("ArrayAccessorSegment").optional()
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DataType,
                one_of(vec_of_erased![
                    Ref::new("PrimitiveTypeSegment"),
                    Ref::new("ArrayTypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MAP"),
                        Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment"),
                            Ref::new("CommaSegment"),
                            Ref::new("DatatypeSegment")
                        ])])
                        .config(|config| {
                            config.bracket_pairs_set = "angle_bracket_pairs";
                            config.bracket_type = "angle";
                        })
                    ]),
                    Ref::new("StructTypeSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AlterDatabaseStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AlterDatabaseStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    one_of(vec_of_erased![Ref::keyword("DATABASE"), Ref::keyword("SCHEMA")]),
                    Ref::new("DatabaseReferenceSegment"),
                    Ref::keyword("SET"),
                    Ref::new("DatabasePropertiesGrammar")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "AlterTableStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TABLE"),
            Ref::new("TableReferenceSegment"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar"),
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("PartitionSpecGrammar")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("COLUMN"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("TO"),
                    Ref::new("ColumnReferenceSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    one_of(vec_of_erased![Ref::keyword("COLUMNS"), Ref::keyword("COLUMN")]),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnFieldDefinitionSegment"),
                            one_of(vec_of_erased![
                                Ref::keyword("FIRST"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("AFTER"),
                                    Ref::new("ColumnReferenceSegment")
                                ])
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ])
                    ])]),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("ALTER"), Ref::keyword("CHANGE")]),
                    Ref::keyword("COLUMN").optional(),
                    MetaSegment::indent(),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment")
                            .exclude(one_of(vec_of_erased![
                                Ref::keyword("COMMENT"),
                                Ref::keyword("TYPE"),
                                Ref::new("DatatypeSegment"),
                                Ref::keyword("FIRST"),
                                Ref::keyword("AFTER"),
                                Ref::keyword("SET"),
                                Ref::keyword("DROP")
                            ]))
                            .config(|config| {
                                config.exclude = one_of(vec_of_erased![
                                    Ref::keyword("COMMENT"),
                                    Ref::keyword("TYPE"),
                                    Ref::new("DatatypeSegment"),
                                    Ref::keyword("FIRST"),
                                    Ref::keyword("AFTER"),
                                    Ref::keyword("SET"),
                                    Ref::keyword("DROP")
                                ])
                                .to_matchable()
                                .into();
                            })
                    ])
                    .config(|config| {
                        config.max_times = Some(2);
                    }),
                    Ref::keyword("TYPE").optional(),
                    Ref::new("DatatypeSegment").optional(),
                    Ref::new("CommentGrammar").optional(),
                    one_of(vec_of_erased![
                        Ref::keyword("FIRST"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AFTER"),
                            Ref::new("ColumnReferenceSegment")
                        ])
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DROP")]),
                        Ref::keyword("NOT"),
                        Ref::keyword("NULL")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REPLACE"),
                    Ref::keyword("COLUMNS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            Ref::new("ColumnDefinitionSegment"),
                            Ref::new("CommentGrammar").optional()
                        ]
                    )])])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLUMN"),
                            Ref::new("ColumnReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLUMNS"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                AnyNumberOf::new(vec_of_erased![Ref::new(
                                    "ColumnReferenceSegment"
                                )])
                            ])])
                        ])
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("PartitionSpecGrammar"),
                        Ref::new("PartitionFieldGrammar")
                    ])
                    .config(|config| {
                        config.min_times = 1;
                    })
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::new("IfExistsGrammar").optional(),
                    one_of(vec_of_erased![
                        Ref::new("PartitionSpecGrammar"),
                        Ref::new("PartitionFieldGrammar")
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("PURGE")]).config(|config| {
                        config.optional();
                    })
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("Replace"),
                    Ref::new("PartitionFieldGrammar")
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("RECOVER"), Ref::keyword("PARTITIONS")]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("TablePropertiesGrammar")
                ]),
                Ref::new("UnsetTablePropertiesGrammar"),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SERDEPROPERTIES"),
                            Ref::new("BracketedPropertyListGrammar")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SERDE"),
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("SerdePropertiesGrammar").optional()
                        ])
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::keyword("SET"),
                    Ref::keyword("FILEFORMAT"),
                    Ref::new("DataSourceFormatSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::keyword("SET"),
                    Ref::new("LocationGrammar")
                ]),
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    one_of(vec_of_erased![Ref::keyword("ADD"), Ref::keyword("DROP")]),
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ColumnReferenceSegment").exclude(Ref::keyword("CHECK")).config(
                        |config| {
                            config.exclude = Ref::keyword("CHECK").to_matchable().into();
                        }
                    ),
                    Ref::keyword("CHECK").optional(),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]).config(
                        |config| {
                            config.optional();
                        }
                    ),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WRITE"),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DISTRIBUTED"),
                            Ref::keyword("BY"),
                            Ref::keyword("PARTITION")
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LOCALLY").optional(),
                            Ref::keyword("ORDERED"),
                            Ref::keyword("BY"),
                            MetaSegment::indent(),
                            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                    .config(|config| {
                                        config.optional();
                                    }),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("NULLS"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("FIRST"),
                                        Ref::keyword("LAST")
                                    ])
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                            ])])
                            .config(|config| {
                                config.optional();
                            }),
                            MetaSegment::dedent()
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ])
                    .config(|config| {
                        config.min_times = 1;
                        config.max_times_per_element = Some(1);
                    })
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("IDENTIFIER"),
                    Ref::keyword("FIELDS"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])]),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("IDENTIFIER"),
                    Ref::keyword("FIELDS"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])]),
                    MetaSegment::dedent()
                ])
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    sparksql_dialect.add([(
        "ColumnFieldDefinitionSegment".into(),
        NodeMatcher::new(
            SyntaxKind::ColumnDefinition,
            Sequence::new(vec_of_erased![
                Ref::new("ColumnReferenceSegment"),
                Ref::new("DatatypeSegment"),
                Bracketed::new(vec_of_erased![Anything::new()]).config(|config| {
                    config.optional();
                }),
                AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment").optional()])
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.add([(
        "AlterViewStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::AlterViewStatement,
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("VIEW"),
                Ref::new("TableReferenceSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RENAME"),
                        Ref::keyword("TO"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::new("TablePropertiesGrammar")
                    ]),
                    Ref::new("UnsetTablePropertiesGrammar"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        optionally_bracketed(vec_of_erased![Ref::new("SelectStatementSegment")])
                    ])
                ])
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.replace_grammar(
        "CreateDatabaseStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![Ref::keyword("DATABASE"), Ref::keyword("SCHEMA")]),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("DatabaseReferenceSegment"),
            Ref::new("CommentGrammar").optional(),
            Ref::new("LocationGrammar").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("DBPROPERTIES"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .config(|config| {
                config.optional();
            })
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "CreateFunctionStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE")]).config(
                |config| {
                    config.optional();
                }
            ),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("FUNCTION"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("FunctionNameIdentifierSegment"),
            Ref::keyword("AS"),
            Ref::new("QuotedLiteralSegment"),
            Ref::new("ResourceLocationGrammar").optional()
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "CreateTableStatementSegment",
        Sequence::new(vec_of_erased![Ref::keyword("CREATE"), Ref::new("TableDefinitionSegment")])
            .to_matchable(),
    );

    sparksql_dialect.add([(
        "CreateHiveFormatTableStatementSegment".into(),
        hive_dialect.grammar("CreateTableStatementSegment").into(),
    )]);

    sparksql_dialect.replace_grammar(
        "CreateViewStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![Ref::new("OrReplaceGrammar"), Ref::new("OrRefreshGrammar")])
                .config(|config| {
                    config.optional();
                }),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("STREAMING").optional(),
            Ref::keyword("LIVE").optional(),
            Ref::keyword("VIEW"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![Delimited::new(
                vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("CommentGrammar").optional()
                ])]
            )])])
            .config(|config| {
                config.optional();
            }),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Ref::new("DataSourceFormatSegment")
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::new("OptionsGrammar").optional(),
            Ref::new("CommentGrammar").optional(),
            Ref::new("TablePropertiesGrammar").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::new("WithNoSchemaBindingClauseSegment").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "CreateWidgetStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateWidgetStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("WIDGET"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DROPDOWN"),
                            Ref::new("WidgetNameIdentifierSegment"),
                            Ref::new("WidgetDefaultGrammar"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CHOICES"),
                                Ref::new("SelectStatementSegment")
                            ])
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TEXT"),
                            Ref::new("WidgetNameIdentifierSegment"),
                            Ref::new("WidgetDefaultGrammar")
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ReplaceTableStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ReplaceTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("REPLACE"),
                    Ref::new("TableDefinitionSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RemoveWidgetStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RemoveWidgetStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("REMOVE"),
                    Ref::keyword("WIDGET"),
                    Ref::new("WidgetNameIdentifierSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "DropDatabaseStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            one_of(vec_of_erased![Ref::keyword("DATABASE"), Ref::keyword("SCHEMA")]),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("DatabaseReferenceSegment"),
            Ref::new("DropBehaviorGrammar").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([(
        "DropFunctionStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::DropFunctionStatement,
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::new("TemporaryGrammar").optional(),
                Ref::keyword("FUNCTION"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("FunctionNameSegment")
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.add([(
        "MsckRepairTableStatementSegment".into(),
        hive_dialect.grammar("MsckRepairTableStatementSegment").into(),
    )]);

    sparksql_dialect.replace_grammar(
        "TruncateStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("TRUNCATE"),
            Ref::keyword("TABLE"),
            Ref::new("TableReferenceSegment"),
            Ref::new("PartitionSpecGrammar").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "UseDatabaseStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::UseDatabaseStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("USE"),
                    Ref::new("DatabaseReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::InsertStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    one_of(vec_of_erased![Ref::keyword("INTO"), Ref::keyword("OVERWRITE")]),
                    Ref::keyword("TABLE").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    one_of(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("ValuesClauseSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        ),
                        Ref::new("SelectableGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE").optional(),
                            Ref::new("TableReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::new("TableReferenceSegment"),
                            Ref::keyword("SELECT"),
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                            Ref::new("WhereClauseSegment").optional(),
                            Ref::new("GroupByClauseSegment").optional(),
                            Ref::new("OrderByClauseSegment").optional(),
                            Ref::new("LimitClauseSegment").optional()
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "InsertOverwriteDirectorySegment".into(),
            NodeMatcher::new(
                SyntaxKind::InsertOverwriteDirectoryStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("OVERWRITE"),
                    Ref::keyword("LOCAL").optional(),
                    Ref::keyword("DIRECTORY"),
                    Ref::new("QuotedLiteralSegment").optional(),
                    Ref::keyword("USING"),
                    Ref::new("DataSourceFormatSegment"),
                    Ref::new("OptionsGrammar").optional(),
                    one_of(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("ValuesClauseSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        ),
                        Ref::new("SelectableGrammar")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "InsertOverwriteDirectoryHiveFmtSegment".into(),
            NodeMatcher::new(
                SyntaxKind::InsertOverwriteDirectoryHiveFmtStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("OVERWRITE"),
                    Ref::keyword("LOCAL").optional(),
                    Ref::keyword("DIRECTORY"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("RowFormatClauseSegment").optional(),
                    Ref::new("StoredAsGrammar").optional(),
                    one_of(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("ValuesClauseSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        ),
                        Ref::new("SelectableGrammar")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "LoadDataSegment".into(),
            NodeMatcher::new(
                SyntaxKind::LoadDataStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("LOAD"),
                    Ref::keyword("DATA"),
                    Ref::keyword("LOCAL").optional(),
                    Ref::keyword("INPATH"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::keyword("OVERWRITE").optional(),
                    Ref::keyword("INTO"),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("PartitionSpecGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ClusterByClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ClusterByClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTER"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment")
                        ]
                    )])])
                    .config(|config| {
                        config.terminators = vec_of_erased![
                            Ref::keyword("LIMIT"),
                            Ref::keyword("HAVING"),
                            Ref::keyword("WINDOW"),
                            Ref::new("FrameClauseUnitGrammar"),
                            Ref::keyword("SEPARATOR")
                        ];
                    }),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DistributeByClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DistributeByClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DISTRIBUTE"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment")
                        ]
                    )])])
                    .config(|config| {
                        config.terminators = vec_of_erased![
                            Ref::keyword("SORT"),
                            Ref::keyword("LIMIT"),
                            Ref::keyword("HAVING"),
                            Ref::keyword("WINDOW"),
                            Ref::new("FrameClauseUnitGrammar"),
                            Ref::keyword("SEPARATOR")
                        ];
                    }),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "HintFunctionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::HintFunction,
                Sequence::new(vec_of_erased![
                    Ref::new("FunctionNameSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("ColumnReferenceSegment")
                        ])
                        .config(|config| {
                            config.min_times = 1;
                        })
                    ])])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SelectHintSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SelectHint,
                Sequence::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("StartHintSegment"),
                    Delimited::new(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("HintFunctionSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        )
                    ])
                    .config(|config| {
                        config.terminators = vec_of_erased![Ref::new("EndHintSegment")];
                    }),
                    Ref::new("EndHintSegment")
                ])])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "LimitClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("LIMIT"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Ref::new("NumericLiteralSegment"),
                Ref::keyword("ALL"),
                Ref::new("FunctionSegment")
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "SetOperatorSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("EXCEPT"), Ref::keyword("MINUS")]),
                Ref::keyword("ALL").optional()
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("UNION"), Ref::keyword("INTERSECT")]),
                one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")]).config(
                    |config| {
                        config.optional();
                    }
                )
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "SelectClauseModifierSegment",
        Sequence::new(vec_of_erased![
            Ref::new("SelectHintSegment").optional(),
            one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")]).config(
                |config| {
                    config.optional();
                }
            )
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        ansi::get_unordered_select_statement_segment_grammar().copy(
            Some(vec_of_erased![Ref::new("QualifyClauseSegment").optional()]),
            None,
            None,
            Some(vec_of_erased![Ref::new("OverlapsClauseSegment").optional()]),
            Vec::new(),
            false,
        ),
    );

    sparksql_dialect.replace_grammar(
        "SelectStatementSegment",
        ansi::select_statement()
            .copy(
                Some(vec_of_erased![
                    Ref::new("ClusterByClauseSegment",).optional(),
                    Ref::new("DistributeByClauseSegment").optional(),
                    Ref::new("SortByClauseSegment").optional(),
                ]),
                None,
                Some(Ref::new("LimitClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            )
            .copy(
                Some(vec_of_erased![Ref::new("QualifyClauseSegment").optional()]),
                None,
                Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            ),
    );

    sparksql_dialect.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("GROUP"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Delimited::new(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("CubeRollupClauseSegment"),
                    Ref::new("GroupingSetsClauseSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("ExpressionSegment")
                    ]),
                    one_of(vec_of_erased![
                        Ref::new("WithCubeRollupClauseSegment"),
                        Ref::new("GroupingSetsClauseSegment")
                    ])
                ])
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "WithCubeRollupClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WithCubeRollupClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    one_of(vec_of_erased![Ref::keyword("CUBE"), Ref::keyword("ROLLUP")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SortByClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SortByClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SORT"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment")
                        ]),
                        one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")]).config(
                            |config| {
                                config.optional();
                            }
                        ),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NULLS"),
                            one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST")])
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ])])
                    .config(|config| {
                        config.terminators = vec_of_erased![
                            Ref::keyword("LIMIT"),
                            Ref::keyword("HAVING"),
                            Ref::keyword("QUALIFY"),
                            Ref::keyword("WINDOW"),
                            Ref::new("FrameClauseUnitGrammar"),
                            Ref::keyword("SEPARATOR")
                        ];
                    }),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "SamplingExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("TABLESAMPLE"),
            one_of(vec_of_erased![
                Bracketed::new(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    one_of(vec_of_erased![Ref::keyword("PERCENT"), Ref::keyword("ROWS")])
                ]),
                Bracketed::new(vec_of_erased![
                    Ref::keyword("BUCKET"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::keyword("OUT"),
                    Ref::keyword("OF"),
                    Ref::new("NumericLiteralSegment")
                ])
            ])
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "LateralViewClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::LateralViewClause,
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::keyword("LATERAL"),
                    Ref::keyword("VIEW"),
                    Ref::keyword("OUTER").optional(),
                    Ref::new("FunctionSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AS").optional(),
                                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AS").optional(),
                            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                        ])
                    ]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "PivotClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PivotClause,
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::keyword("PIVOT"),
                    Bracketed::new(vec_of_erased![
                        MetaSegment::indent(),
                        Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("BaseExpressionElementGrammar"),
                            Ref::new("AliasExpressionSegment").optional()
                        ])]),
                        Ref::keyword("FOR"),
                        optionally_bracketed(vec_of_erased![one_of(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                        ])]),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                        Ref::new("ExpressionSegment")
                                    ])])
                                    .config(|config| {
                                        config.parse_mode(ParseMode::Greedy);
                                    }),
                                    Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")])
                                ]),
                                Ref::new("AliasExpressionSegment").optional()
                            ])
                        ])]),
                        MetaSegment::dedent()
                    ]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TransformClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TransformClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("TRANSFORM"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SingleIdentifierGrammar"
                    )])])
                    .config(|config| {
                        config.parse_mode(ParseMode::Greedy);
                    }),
                    MetaSegment::indent(),
                    Ref::new("RowFormatClauseSegment").optional(),
                    Ref::keyword("USING"),
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            AnyNumberOf::new(vec_of_erased![
                                Ref::new("SingleIdentifierGrammar"),
                                Ref::new("DatatypeSegment")
                            ])
                        ])])
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Ref::new("RowFormatClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        ("RowFormatClauseSegment".into(), hive_dialect.grammar("RowFormatClauseSegment").into()),
        ("SkewedByClauseSegment".into(), hive_dialect.grammar("SkewedByClauseSegment").into()),
    ]);

    sparksql_dialect.replace_grammar(
        "ExplainStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("EXPLAIN"),
            one_of(vec_of_erased![
                Ref::keyword("EXTENDED"),
                Ref::keyword("CODEGEN"),
                Ref::keyword("COST"),
                Ref::keyword("FORMATTED")
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::new("StatementSegment")
        ])
        .to_matchable(),
    );

    sparksql_dialect.add([
        (
            "AddFileSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AddFileStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::new("FileKeywordSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AddJarSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AddJarStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::new("JarKeywordSegment"),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileLiteralSegment")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AnalyzeTableSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AnalyzeTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ANALYZE"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE"),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("PartitionSpecGrammar").optional(),
                            Ref::keyword("COMPUTE"),
                            Ref::keyword("STATISTICS"),
                            one_of(vec_of_erased![
                                Ref::keyword("NOSCAN"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FOR"),
                                    Ref::keyword("COLUMNS"),
                                    optionally_bracketed(vec_of_erased![Delimited::new(
                                        vec_of_erased![Ref::new("ColumnReferenceSegment")]
                                    )])
                                ])
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLES"),
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                                Ref::new("DatabaseReferenceSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Ref::keyword("COMPUTE"),
                            Ref::keyword("STATISTICS"),
                            Ref::keyword("NOSCAN").optional()
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CacheTableSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CacheTable,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CACHE"),
                    Ref::keyword("LAZY").optional(),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("OptionsGrammar").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS").optional(),
                        Ref::new("SelectableGrammar")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ClearCacheSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ClearCache,
                Sequence::new(vec_of_erased![Ref::keyword("CLEAR"), Ref::keyword("CACHE")])
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DescribeStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DescribeStatement,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("DESCRIBE"), Ref::keyword("DESC")]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("DATABASE"),
                                Ref::keyword("SCHEMA")
                            ]),
                            Ref::keyword("EXTENDED").optional(),
                            Ref::new("DatabaseReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FUNCTION"),
                            Ref::keyword("EXTENDED").optional(),
                            Ref::new("FunctionNameSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE").optional(),
                            Ref::keyword("EXTENDED").optional(),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("PartitionSpecGrammar").optional(),
                            Sequence::new(vec_of_erased![
                                Ref::new("SingleIdentifierGrammar"),
                                AnyNumberOf::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::new("DotSegment"),
                                        Ref::new("SingleIdentifierGrammar")
                                    ])
                                    .config(|config| {
                                        config.disallow_gaps();
                                    })
                                ])
                                .config(|config| {
                                    config.max_times = Some(2);
                                    config.disallow_gaps();
                                })
                            ])
                            .config(|config| {
                                config.optional();
                                config.disallow_gaps();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("QUERY").optional(),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("TABLE"),
                                    Ref::new("TableReferenceSegment")
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FROM"),
                                    Ref::new("TableReferenceSegment"),
                                    Ref::keyword("SELECT"),
                                    Delimited::new(vec_of_erased![Ref::new(
                                        "ColumnReferenceSegment"
                                    )]),
                                    Ref::new("WhereClauseSegment").optional(),
                                    Ref::new("GroupByClauseSegment").optional(),
                                    Ref::new("OrderByClauseSegment").optional(),
                                    Ref::new("LimitClauseSegment").optional()
                                ]),
                                Ref::new("StatementSegment")
                            ])
                        ])
                    ])
                    .config(|config| {
                        config.exclude =
                            one_of(vec_of_erased![Ref::keyword("HISTORY"), Ref::keyword("DETAIL")])
                                .to_matchable()
                                .into();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ListFileSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ListFileStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIST"),
                    Ref::new("FileKeywordSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ListJarSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ListJarStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIST"),
                    Ref::new("JarKeywordSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RefreshStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RefreshStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFRESH"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE").optional(),
                            Ref::new("TableReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FUNCTION"),
                            Ref::new("FunctionNameSegment")
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ResetStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ResetStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESET"),
                    Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]).config(
                        |config| {
                            config.delimiter(Ref::new("DotSegment"));
                            config.optional();
                        }
                    )
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SetStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SetStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("SQLConfPropertiesSegment").optional(),
                    one_of(vec_of_erased![
                        Ref::new("PropertyListGrammar"),
                        Ref::new("PropertyNameSegment")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ShowStatement".into(),
            NodeMatcher::new(
                SyntaxKind::ShowStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SHOW"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CREATE"),
                            Ref::keyword("TABLE"),
                            Ref::new("TableExpressionSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AS"),
                                Ref::keyword("SERDE")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLUMNS"),
                            Ref::keyword("IN"),
                            Ref::new("TableExpressionSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("IN"),
                                Ref::new("DatabaseReferenceSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("DATABASES"),
                                Ref::keyword("SCHEMAS")
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("LIKE"),
                                Ref::new("QuotedLiteralSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("USER"),
                                Ref::keyword("SYSTEM"),
                                Ref::keyword("ALL")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Ref::keyword("FUNCTIONS"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::new("DatabaseReferenceSegment"),
                                    Ref::new("DotSegment"),
                                    Ref::new("FunctionNameSegment")
                                ])
                                .config(|config| {
                                    config.disallow_gaps();
                                    config.optional();
                                }),
                                Ref::new("FunctionNameSegment").optional(),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("LIKE"),
                                    Ref::new("QuotedLiteralSegment")
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                            ])
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PARTITIONS"),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("PartitionSpecGrammar").optional()
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE"),
                            Ref::keyword("EXTENDED"),
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![Ref::keyword("IN"), Ref::keyword("FROM")]),
                                Ref::new("DatabaseReferenceSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Ref::keyword("LIKE"),
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("PartitionSpecGrammar").optional()
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLES"),
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                                Ref::new("DatabaseReferenceSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("LIKE"),
                                Ref::new("QuotedLiteralSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TBLPROPERTIES"),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("BracketedPropertyNameListGrammar").optional()
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("VIEWS"),
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                                Ref::new("DatabaseReferenceSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("LIKE"),
                                Ref::new("QuotedLiteralSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "UncacheTableSegment".into(),
            NodeMatcher::new(
                SyntaxKind::UncacheTable,
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNCACHE"),
                    Ref::keyword("TABLE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec_of_erased![
                Ref::new("AlterDatabaseStatementSegment"),
                Ref::new("AlterTableStatementSegment"),
                Ref::new("AlterViewStatementSegment"),
                Ref::new("CreateHiveFormatTableStatementSegment"),
                Ref::new("MsckRepairTableStatementSegment"),
                Ref::new("UseDatabaseStatementSegment"),
                Ref::new("AddFileSegment"),
                Ref::new("AddJarSegment"),
                Ref::new("AnalyzeTableSegment"),
                Ref::new("CacheTableSegment"),
                Ref::new("ClearCacheSegment"),
                Ref::new("ListFileSegment"),
                Ref::new("ListJarSegment"),
                Ref::new("RefreshStatementSegment"),
                Ref::new("ResetStatementSegment"),
                Ref::new("SetStatementSegment"),
                Ref::new("ShowStatement"),
                Ref::new("UncacheTableSegment"),
                Ref::new("InsertOverwriteDirectorySegment"),
                Ref::new("InsertOverwriteDirectoryHiveFmtSegment"),
                Ref::new("LoadDataSegment"),
                Ref::new("ClusterByClauseSegment"),
                Ref::new("DistributeByClauseSegment"),
                Ref::new("VacuumStatementSegment"),
                Ref::new("DescribeHistoryStatementSegment"),
                Ref::new("DescribeDetailStatementSegment"),
                Ref::new("GenerateManifestFileStatementSegment"),
                Ref::new("ConvertToDeltaStatementSegment"),
                Ref::new("RestoreTableStatementSegment"),
                Ref::new("ConstraintStatementSegment"),
                Ref::new("ApplyChangesIntoStatementSegment"),
                Ref::new("CreateWidgetStatementSegment"),
                Ref::new("RemoveWidgetStatementSegment"),
                Ref::new("ReplaceTableStatementSegment"),
            ]),
            None,
            None,
            Some(vec_of_erased![
                Ref::new("TransactionStatementSegment"),
                Ref::new("CreateSchemaStatementSegment"),
                Ref::new("SetSchemaStatementSegment"),
                Ref::new("CreateModelStatementSegment"),
                Ref::new("DropModelStatementSegment"),
            ]),
            Vec::new(),
            false,
        ),
    );

    sparksql_dialect.replace_grammar(
        "JoinClauseSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("JoinTypeKeywords").optional(),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                MetaSegment::dedent(),
                Conditional::new(MetaSegment::indent()).indented_using_on(),
                one_of(vec_of_erased![
                    Ref::new("JoinOnConditionSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Conditional::new(MetaSegment::indent()),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "SingleIdentifierGrammar"
                        )])])
                        .config(|config| {
                            config.parse_mode(ParseMode::Greedy);
                        }),
                        Conditional::new(MetaSegment::dedent())
                    ])
                ])
                .config(|config| {
                    config.optional();
                }),
                Conditional::new(MetaSegment::dedent()).indented_using_on()
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("NaturalJoinKeywordsGrammar"),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                MetaSegment::dedent()
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "AliasExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("AS").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar").optional(),
                    Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                ]),
                Ref::new("SingleIdentifierGrammar")
            ])
            .config(|config| {
                config.exclude = one_of(vec_of_erased![
                    Ref::keyword("LATERAL"),
                    Ref::new("JoinTypeKeywords"),
                    Ref::keyword("WINDOW"),
                    Ref::keyword("PIVOT"),
                    Ref::keyword("KEYS"),
                    Ref::keyword("FROM")
                ])
                .to_matchable()
                .into();
            })
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "ValuesClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("VALUES"),
            Delimited::new(vec_of_erased![
                one_of(vec_of_erased![
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Ref::keyword("NULL"),
                        Ref::new("ExpressionSegment")
                    ])])
                    .config(|config| {
                        config.parse_mode(ParseMode::Greedy);
                    }),
                    Ref::keyword("NULL"),
                    Ref::new("ExpressionSegment")
                ])
                .config(|config| {
                    config.exclude =
                        one_of(vec_of_erased![Ref::keyword("VALUES")]).to_matchable().into();
                })
            ]),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![Ref::keyword("LIMIT"), Ref::keyword("ORDER")]))
                .optional()
                .config(|config| {
                    config.exclude =
                        one_of(vec_of_erased![Ref::keyword("LIMIT"), Ref::keyword("ORDER")])
                            .to_matchable()
                            .into();
                }),
            Ref::new("OrderByClauseSegment").optional(),
            Ref::new("LimitClauseSegment").optional()
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec_of_erased![
            Ref::new("ValuesClauseSegment"),
            Ref::new("BareFunctionSegment"),
            Ref::new("FunctionSegment"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("FileReferenceSegment"),
                    Ref::new("TableReferenceSegment")
                ]),
                one_of(vec_of_erased![
                    Ref::new("AtSignLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        MetaSegment::indent(),
                        one_of(vec_of_erased![
                            Ref::new("TimestampAsOfGrammar"),
                            Ref::new("VersionAsOfGrammar")
                        ]),
                        MetaSegment::dedent()
                    ])
                ])
                .config(|config| {
                    config.optional();
                })
            ]),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([(
        "FileReferenceSegment".into(),
        NodeMatcher::new(
            SyntaxKind::FileReference,
            Sequence::new(vec_of_erased![
                Ref::new("DataSourcesV2FileTypeGrammar"),
                Ref::new("DotSegment"),
                Ref::new("BackQuotedIdentifierSegment")
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.replace_grammar(
        "FromExpressionElementSegment",
        Sequence::new(vec_of_erased![
            Ref::new("PreTableFunctionKeywordsGrammar").optional(),
            optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![
                    Ref::new("FromClauseTerminatorGrammar"),
                    Ref::new("SamplingExpressionSegment")
                ]))
                .optional()
                .config(|config| {
                    config.exclude = one_of(vec_of_erased![
                        Ref::new("FromClauseTerminatorGrammar"),
                        Ref::new("SamplingExpressionSegment")
                    ])
                    .to_matchable()
                    .into();
                }),
            Ref::new("SamplingExpressionSegment").optional(),
            AnyNumberOf::new(vec_of_erased![Ref::new("LateralViewClauseSegment")]),
            Ref::new("NamedWindowSegment").optional(),
            Ref::new("PivotClauseSegment").optional(),
            Ref::new("PostTableExpressionGrammar").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "PropertyNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PropertyNameIdentifier,
                Sequence::new(vec_of_erased![one_of(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("PropertiesNakedIdentifierSegment")])
                        .config(|config| {
                            config.delimiter(Ref::new("DotSegment"));
                            config.disallow_gaps();
                        }),
                    Ref::new("SingleIdentifierGrammar")
                ])])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "GeneratedColumnDefinitionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::GeneratedColumnDefinition,
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("DatatypeSegment"),
                    Bracketed::new(vec_of_erased![Anything::new()]).config(|config| {
                        config.optional();
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("GENERATED"),
                        Ref::keyword("ALWAYS"),
                        Ref::keyword("AS"),
                        Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::new("FunctionSegment"),
                            Ref::new("BareFunctionSegment")
                        ])])
                    ]),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("ColumnConstraintSegment").optional()
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "MergeUpdateClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("UPDATE"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("WildcardIdentifierSegment")
                ]),
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::new("SetClauseListSegment"),
                    MetaSegment::dedent()
                ])
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "MergeInsertClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("INSERT"),
            one_of(vec_of_erased![
                Ref::new("WildcardIdentifierSegment"),
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    MetaSegment::dedent(),
                    Ref::new("ValuesClauseSegment")
                ])
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "UpdateStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("UPDATE"),
            one_of(vec_of_erased![
                Ref::new("FileReferenceSegment"),
                Ref::new("TableReferenceSegment")
            ]),
            Ref::new("AliasExpressionSegment").exclude(Ref::keyword("SET")).optional().config(
                |config| {
                    config.exclude = Ref::keyword("SET").to_matchable().into();
                }
            ),
            Ref::new("SetClauseListSegment"),
            Ref::new("WhereClauseSegment")
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([(
        "IntervalLiteralSegment".into(),
        NodeMatcher::new(
            SyntaxKind::IntervalLiteral,
            Sequence::new(vec_of_erased![
                Ref::new("SignedSegmentGrammar").optional(),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("SignedQuotedLiteralSegment")
                ]),
                Ref::new("DatetimeUnitSegment"),
                Ref::keyword("TO").optional(),
                Ref::new("DatetimeUnitSegment").optional()
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.replace_grammar(
        "IntervalExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("INTERVAL"),
            one_of(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![Ref::new("IntervalLiteralSegment")]),
                Ref::new("QuotedLiteralSegment")
            ])
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "VacuumStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::VacuumStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("VACUUM"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RETAIN"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("DatetimeUnitSegment")
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("DRY"), Ref::keyword("RUN")])
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DescribeHistoryStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DescribeHistoryStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DESCRIBE"),
                    Ref::keyword("HISTORY"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Ref::new("LimitClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DescribeDetailStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DescribeDetailStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DESCRIBE"),
                    Ref::keyword("DETAIL"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "GenerateManifestFileStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::GenerateManifestFileStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("GENERATE"),
                    StringParser::new(
                        "symlink_format_manifest",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: SyntaxKind::SymlinkFormatManifest },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    Ref::keyword("FOR"),
                    Ref::keyword("TABLE"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ConvertToDeltaStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ConvertToDeltaStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONVERT"),
                    Ref::keyword("TO"),
                    Ref::keyword("DELTA"),
                    Ref::new("FileReferenceSegment"),
                    Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("STATISTICS")])
                        .config(|config| {
                            config.optional();
                        }),
                    Ref::new("PartitionSpecGrammar").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RestoreTableStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RestoreTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESTORE"),
                    Ref::keyword("TABLE"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::new("TimestampAsOfGrammar"),
                        Ref::new("VersionAsOfGrammar")
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ConstraintStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ConstraintStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("EXPECT"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                    Sequence::new(vec_of_erased![Ref::keyword("ON"), Ref::keyword("VIOLATION")])
                        .config(|config| {
                            config.optional();
                        }),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("FAIL"), Ref::keyword("UPDATE")]),
                        Sequence::new(vec_of_erased![Ref::keyword("DROP"), Ref::keyword("ROW")])
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ApplyChangesIntoStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ApplyChangesIntoStatement,
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("APPLY"),
                        Ref::keyword("CHANGES"),
                        Ref::keyword("INTO")
                    ]),
                    MetaSegment::indent(),
                    Ref::new("TableExpressionSegment"),
                    MetaSegment::dedent(),
                    Ref::new("FromClauseSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("KEYS"),
                        MetaSegment::indent(),
                        Ref::new("BracketedColumnReferenceListGrammar"),
                        MetaSegment::dedent()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IGNORE"),
                        Ref::keyword("NULL"),
                        Ref::keyword("UPDATES")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Ref::new("WhereClauseSegment").optional(),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("APPLY"),
                        Ref::keyword("AS"),
                        one_of(vec_of_erased![Ref::keyword("DELETE"), Ref::keyword("TRUNCATE")]),
                        Ref::keyword("WHEN"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment")
                    ])])
                    .config(|config| {
                        config.max_times = Some(2);
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SEQUENCE"),
                        Ref::keyword("BY"),
                        Ref::new("ColumnReferenceSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMNS"),
                        one_of(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                            Sequence::new(vec_of_erased![
                                Ref::new("StarSegment"),
                                Ref::keyword("EXCEPT"),
                                Ref::new("BracketedColumnReferenceListGrammar")
                            ])
                        ])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("STORED"),
                        Ref::keyword("AS"),
                        Ref::keyword("SCD"),
                        Ref::keyword("TYPE"),
                        Ref::new("NumericLiteralSegment")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "WildcardExpressionSegment",
        ansi::wildcard_expression_segment().copy(
            Some(vec_of_erased![Ref::new("ExceptClauseSegment").optional()]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    sparksql_dialect.add([
        (
            "ExceptClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SelectExceptClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SingleIdentifierGrammar"
                    )])])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SelectClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SELECT"),
                    one_of(vec_of_erased![
                        Ref::new("TransformClauseSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("SelectClauseModifierSegment").optional(),
                            MetaSegment::indent(),
                            Delimited::new(vec_of_erased![Ref::new("SelectClauseElementSegment")])
                                .config(|config| {
                                    config.allow_trailing = true;
                                })
                        ])
                    ])
                ])
                .config(|config| {
                    config.terminators = vec_of_erased![
                        Ref::keyword("FROM"),
                        Ref::keyword("WHERE"),
                        Ref::keyword("UNION"),
                        Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                        Ref::keyword("LIMIT"),
                        Ref::keyword("OVERLAPS")
                    ];
                    config.parse_mode(ParseMode::GreedyOnceStarted);
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "UsingClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::UsingClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Ref::new("DataSourceFormatSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DataSourceFormatSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DataSourceFormat,
                one_of(vec_of_erased![
                    Ref::new("FileFormatGrammar"),
                    Ref::keyword("JDBC"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "IcebergTransformationSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IcebergTransformation,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("YEARS"),
                            Ref::keyword("MONTHS"),
                            Ref::keyword("DAYS"),
                            Ref::keyword("DATE"),
                            Ref::keyword("HOURS"),
                            Ref::keyword("DATE_HOUR")
                        ]),
                        Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment")])
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("BUCKET"), Ref::keyword("TRUNCATE")]),
                        Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("CommaSegment"),
                            Ref::new("ColumnReferenceSegment")
                        ])])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "FrameClauseSegment",
        {
            let frame_extent = one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("CURRENT"), Ref::keyword("ROW")]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        Ref::keyword("UNBOUNDED"),
                        Ref::new("IntervalExpressionSegment")
                    ]),
                    one_of(vec_of_erased![Ref::keyword("PRECEDING"), Ref::keyword("FOLLOWING")])
                ])
            ]);

            Sequence::new(vec_of_erased![
                Ref::new("FrameClauseUnitGrammar"),
                one_of(vec_of_erased![
                    frame_extent.clone(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BETWEEN"),
                        frame_extent.clone(),
                        Ref::keyword("AND"),
                        frame_extent
                    ])
                ])
            ])
        }
        .to_matchable(),
    );

    sparksql_dialect.expand();
    sparksql_dialect
}

#[cfg(test)]
mod tests {
    use expect_test::expect_file;
    use itertools::Itertools;
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

    use crate::core::config::{FluffConfig, Value};
    use crate::core::linter::linter::Linter;
    use crate::core::parser::segments::base::ErasedSegment;
    use crate::helpers;

    fn parse_sql(linter: &Linter, sql: &str) -> ErasedSegment {
        let parsed = linter.parse_string(sql, None, None, None).unwrap();
        parsed.tree.unwrap()
    }

    #[test]
    fn base_parse_struct() {
        let linter = Linter::new(
            FluffConfig::new(
                [(
                    "core".into(),
                    Value::Map([("dialect".into(), Value::String("sparksql".into()))].into()),
                )]
                .into(),
                None,
                None,
            ),
            None,
            None,
        );

        let files =
            glob::glob("test/fixtures/dialects/sparksql/*.sql").unwrap().flatten().collect_vec();

        files.par_iter().for_each(|file| {
            let _panic = helpers::enter_panic(file.display().to_string());

            let yaml = file.with_extension("yml");
            let yaml = std::path::absolute(yaml).unwrap();

            let actual = {
                let sql = std::fs::read_to_string(file).unwrap();
                let tree = parse_sql(&linter, &sql);
                let tree = tree.to_serialised(true, true, false);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml].assert_eq(&actual);
        });
    }
}
