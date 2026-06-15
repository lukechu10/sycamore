#[derive(Debug, Clone)]
pub enum SegmentAst {
    Param(String),
    DynParam(String),
    DynSegments(String),
}

#[derive(Debug)]
pub struct RoutePathAst {
    pub(crate) segments: Vec<SegmentAst>,
}

impl RoutePathAst {
    pub fn dyn_segments(&self) -> Vec<SegmentAst> {
        self.segments
            .iter()
            .filter(|x| matches!(x, SegmentAst::DynParam(_) | &SegmentAst::DynSegments(_)))
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub(crate) message: String,
}

type Result<T, E = ParseError> = std::result::Result<T, E>;

pub fn parse_route(i: &str) -> Result<RoutePathAst> {
    let i = i.trim_matches('/');
    let segments = i.split('/');
    let mut segments_ast = Vec::with_capacity(segments.size_hint().0);

    for segment in segments {
        if segment.starts_with('<') {
            if segment.ends_with("..>") {
                segments_ast.push(SegmentAst::DynSegments(
                    segment[1..segment.len() - 3].to_string(),
                ));
            } else if segment.ends_with('>') {
                segments_ast.push(SegmentAst::DynParam(
                    segment[1..segment.len() - 1].to_string(),
                ));
            } else {
                return Err(ParseError {
                    message: "missing `>` in dynamic segment".to_string(),
                });
            }
        } else if !segment.is_empty() {
            segments_ast.push(SegmentAst::Param(segment.to_string()));
        } else if !i.is_empty() {
            // Do not return this error if we are matching the index page ("/").
            return Err(ParseError {
                message: "segment cannot be empty".to_string(),
            });
        }
    }

    Ok(RoutePathAst {
        segments: segments_ast,
    })
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};

    use super::*;

    #[track_caller]
    fn check(input: &str, expect: Expect) {
        let actual = format!("{:#?}", parse_route(input).expect("could not parse route"));
        expect.assert_eq(&actual);
    }

    #[test]
    fn index_route() {
        check(
            "/",
            expect![[r#"
                RoutePathAst {
                    segments: [],
                }"#]],
        );
    }

    #[test]
    fn static_route() {
        check(
            "/my/static/path",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        Param(
                            "my",
                        ),
                        Param(
                            "static",
                        ),
                        Param(
                            "path",
                        ),
                    ],
                }"#]],
        );
    }

    #[test]
    fn route_with_trailing_slash() {
        check(
            "/path/",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        Param(
                            "path",
                        ),
                    ],
                }"#]],
        );
    }

    #[test]
    fn route_with_no_leading_slash() {
        check(
            "my/static/path",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        Param(
                            "my",
                        ),
                        Param(
                            "static",
                        ),
                        Param(
                            "path",
                        ),
                    ],
                }"#]],
        );
    }

    #[test]
    fn route_with_no_slash() {
        check(
            "path",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        Param(
                            "path",
                        ),
                    ],
                }"#]],
        );
    }

    #[test]
    fn dyn_param() {
        check(
            "/id/<id>",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        Param(
                            "id",
                        ),
                        DynParam(
                            "id",
                        ),
                    ],
                }"#]],
        );
    }

    #[test]
    fn unnamed_dyn_param() {
        check(
            "/id/<_>",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        Param(
                            "id",
                        ),
                        DynParam(
                            "_",
                        ),
                    ],
                }"#]],
        );
    }

    #[test]
    fn dyn_segments() {
        check(
            "/page/<path..>",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        Param(
                            "page",
                        ),
                        DynSegments(
                            "path",
                        ),
                    ],
                }"#]],
        );
    }

    #[test]
    fn dyn_param_before_dyn_segment() {
        check(
            "/<param>/<segments..>",
            expect![[r#"
                RoutePathAst {
                    segments: [
                        DynParam(
                            "param",
                        ),
                        DynSegments(
                            "segments",
                        ),
                    ],
                }"#]],
        );
    }
}
