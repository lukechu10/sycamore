//! The Sycamore Router.

#![warn(missing_docs)]
#![deny(missing_debug_implementations)]

// Alias self to sycamore_router for proc-macros.
extern crate self as sycamore_router;

mod router;

use std::str::FromStr;

pub use router::*;
pub use sycamore_router_macro::Route;

/// Trait that is implemented for `enum`s that can match routes.
///
/// This trait should not be implemented manually. Use the [`Route`](derive@Route) derive macro
/// instead.
pub trait Route: Sized + Default {
    /// Matches a route with the given path segments. Note that in general, empty segments should be
    /// filtered out before passed as an argument.
    ///
    /// It is likely that you are looking for the [`Route::match_path`] method instead.
    fn match_route(&self, segments: &[&str]) -> Self;

    /// Matches a route with the given path.
    fn match_path(&self, path: &str) -> Self {
        let segments = path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        self.match_route(&segments)
    }
}

/// Represents an URL segment or segments.
#[derive(Clone, Debug)]
pub enum Segment {
    /// Match a specific segment.
    Param(String),
    /// Match an arbitrary segment that is captured.
    DynParam,
    /// Match an arbitrary amount of segments that are captured.
    DynSegments,
}

/// Represents a capture of an URL segment or segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capture<'a> {
    /// A dynamic parameter in the URL (i.e. matches a single url segment).
    DynParam(&'a str),
    /// A dynamic segment in the URL (i.e. matches multiple url segments).
    DynSegments(Vec<&'a str>),
}

impl<'a> Capture<'a> {
    /// Attempts to cast the [`Capture`] to a [`Capture::DynParam`] with the matched url param.
    pub fn as_dyn_param(&self) -> Option<&'a str> {
        if let Self::DynParam(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// Attempts to cast the [`Capture`] to a [`Capture::DynSegments`] with the matched url params.
    pub fn as_dyn_segments(&self) -> Option<&[&'a str]> {
        if let Self::DynSegments(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

/// A list of [`Segment`]s.
#[derive(Clone, Debug)]
pub struct RoutePath {
    segments: Vec<Segment>,
}

impl RoutePath {
    /// Create a new [`RoutePath`] from a list of [`Segment`]s.
    pub fn new(segments: Vec<Segment>) -> Self {
        Self { segments }
    }

    /// Attempt to match the path (url) with the current [`RoutePath`]. The path should already be
    /// split around `/` characters.
    pub fn match_path<'a>(&self, path: &[&'a str]) -> Option<Vec<Capture<'a>>> {
        let mut paths = path.to_vec();
        if let Some(last) = paths.last_mut() {
            // Get rid of everything after '?' and '#' in the last segment.
            *last = last.split('?').next().unwrap().split('#').next().unwrap();
        }
        let mut paths = paths.iter();
        let mut segments = self.segments.iter();
        let mut captures = Vec::new();

        while let Some(segment) = segments.next() {
            match segment {
                Segment::Param(param) => {
                    if paths.next() != Some(&param.as_str()) {
                        return None;
                    }
                }
                Segment::DynParam => {
                    let p = paths.next()?;
                    captures.push(Capture::DynParam(p));
                }
                Segment::DynSegments => {
                    if let Some(next_segment) = segments.next() {
                        // Capture until match with Segment::Param.
                        match next_segment {
                            Segment::Param(next_param) => {
                                let mut capture = Vec::new();
                                for next_path in &mut paths {
                                    if next_path == next_param {
                                        captures.push(Capture::DynSegments(capture));
                                        break;
                                    } else {
                                        capture.push(next_path);
                                    }
                                }
                            }
                            _ => unreachable!("segment following DynSegments cannot be dynamic"),
                        }
                    } else {
                        // All remaining segments are captured.
                        let mut capture = Vec::new();
                        for next_path in &mut paths {
                            capture.push(*next_path);
                        }
                        captures.push(Capture::DynSegments(capture));
                    }
                }
            }
        }

        if paths.next().is_some() {
            return None; // Leftover segments in paths.
        }

        Some(captures)
    }
}

/// Fallible conversion between a param capture into a value.
///
/// Implemented for all types that implement [`FromStr`] by default.
pub trait TryFromParam: Sized {
    /// Creates a new value of this type from the given param. Returns `None` if the param cannot
    /// be converted into a value of this type.
    #[must_use]
    fn try_from_param(param: &str) -> Option<Self>;
}

impl<T> TryFromParam for T
where
    T: FromStr,
{
    fn try_from_param(param: &str) -> Option<Self> {
        param.parse().ok()
    }
}

/// Fallible conversion between a list of param captures into a value.
pub trait TryFromSegments: Sized {
    /// Sets the value of the capture variable with the value of `segments`. Returns `false` if
    /// unsuccessful (e.g. parsing error).
    #[must_use]
    fn try_from_segments(segments: &[&str]) -> Option<Self>;
}

impl<T> TryFromSegments for Vec<T>
where
    T: TryFromParam,
{
    fn try_from_segments(segments: &[&str]) -> Option<Self> {
        let mut tmp = Vec::with_capacity(segments.len());
        for segment in segments {
            let value = T::try_from_param(segment)?;
            tmp.push(value);
        }
        Some(tmp)
    }
}

impl<T: Route> TryFromSegments for T {
    fn try_from_segments(segments: &[&str]) -> Option<Self> {
        // It's fine to use `default()` here for the Perseus use-case (TODO is there any situation
        // where this wouldn't be fine?)
        Some(Self::match_route(&Self::default(), segments))
    }
}

#[cfg(test)]
mod tests {
    use Segment::*;

    use super::*;

    #[track_caller]
    fn check(path: &str, route: RoutePath, expected: Option<Vec<Capture>>) {
        let path = path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(route.match_path(&path), expected);
    }

    #[test]
    fn index_path() {
        check("/", RoutePath::new(Vec::new()), Some(Vec::new()));
    }

    #[test]
    fn static_path_single_segment() {
        check(
            "/path",
            RoutePath::new(vec![Param("path".to_string())]),
            Some(Vec::new()),
        );
    }

    #[test]
    fn static_path_multiple_segments() {
        check(
            "/my/static/path",
            RoutePath::new(vec![
                Param("my".to_string()),
                Param("static".to_string()),
                Param("path".to_string()),
            ]),
            Some(Vec::new()),
        );
    }

    #[test]
    fn do_not_match_if_leftover_segments() {
        check("/path", RoutePath::new(vec![]), None);
        check(
            "/my/static/path",
            RoutePath::new(vec![Param("my".to_string()), Param("static".to_string())]),
            None,
        );
    }

    #[test]
    fn dyn_param_single_segment() {
        check(
            "/abcdef",
            RoutePath::new(vec![DynParam]),
            Some(vec![Capture::DynParam("abcdef")]),
        );
    }

    #[test]
    fn dyn_param_with_leading_segment() {
        check(
            "/id/abcdef",
            RoutePath::new(vec![Param("id".to_string()), DynParam]),
            Some(vec![Capture::DynParam("abcdef")]),
        );
    }

    #[test]
    fn dyn_param_with_leading_and_trailing_segment() {
        check(
            "/id/abcdef/account",
            RoutePath::new(vec![
                Param("id".to_string()),
                DynParam,
                Param("account".to_string()),
            ]),
            Some(vec![Capture::DynParam("abcdef")]),
        );
    }

    #[test]
    fn dyn_param_final_missing_root() {
        check("/", RoutePath::new(vec![DynParam]), None);
    }

    #[test]
    fn dyn_param_final_missing() {
        check(
            "/id",
            RoutePath::new(vec![Param("id".to_string()), DynParam]),
            None,
        );
    }

    #[test]
    fn multiple_dyn_params() {
        check(
            "/a/b",
            RoutePath::new(vec![DynParam, DynParam]),
            Some(vec![Capture::DynParam("a"), Capture::DynParam("b")]),
        );
    }

    #[test]
    fn dyn_segments_at_root() {
        check(
            "/a/b/c",
            RoutePath::new(vec![DynSegments]),
            Some(vec![Capture::DynSegments(vec!["a", "b", "c"])]),
        );
    }

    #[test]
    fn dyn_segments_final() {
        check(
            "/id/a/b/c",
            RoutePath::new(vec![Param("id".to_string()), DynSegments]),
            Some(vec![Capture::DynSegments(vec!["a", "b", "c"])]),
        );
    }

    #[test]
    fn dyn_segments_capture_lazy() {
        check(
            "/id/a/b/c/end",
            RoutePath::new(vec![
                Param("id".to_string()),
                DynSegments,
                Param("end".to_string()),
            ]),
            Some(vec![Capture::DynSegments(vec!["a", "b", "c"])]),
        );
    }

    #[test]
    fn dyn_segments_can_capture_zero_segments() {
        check(
            "/",
            RoutePath::new(vec![DynSegments]),
            Some(vec![Capture::DynSegments(Vec::new())]),
        );
    }

    #[test]
    fn multiple_dyn_segments() {
        check(
            "/a/b/c/param/e/f/g",
            RoutePath::new(vec![DynSegments, Param("param".to_string()), DynSegments]),
            Some(vec![
                Capture::DynSegments(vec!["a", "b", "c"]),
                Capture::DynSegments(vec!["e", "f", "g"]),
            ]),
        );
    }

    #[test]
    fn ignore_query_params_static() {
        check(
            "/a/b?foo=bar",
            RoutePath::new(vec![Param("a".to_string()), Param("b".to_string())]),
            Some(Vec::new()),
        );
    }

    #[test]
    fn ignore_query_params_dyn() {
        check(
            "/a/b/c?foo=bar",
            RoutePath::new(vec![DynSegments]),
            Some(vec![Capture::DynSegments(vec!["a", "b", "c"])]),
        );
    }

    #[test]
    fn ignore_hash_static() {
        check(
            "/a/b#foo",
            RoutePath::new(vec![Param("a".to_string()), Param("b".to_string())]),
            Some(Vec::new()),
        );
    }

    #[test]
    fn ignore_hash_dyn() {
        check(
            "/a/b/c#foo",
            RoutePath::new(vec![DynSegments]),
            Some(vec![Capture::DynSegments(vec!["a", "b", "c"])]),
        );
    }

    mod integration {
        use crate::*;

        #[test]
        fn simple_router() {
            #[derive(Debug, PartialEq, Eq, Route)]
            enum Routes {
                #[to("/")]
                Home,
                #[to("/about")]
                About,
                #[not_found]
                NotFound,
            }

            assert_eq!(Routes::match_route(&Routes::default(), &[]), Routes::Home);
            assert_eq!(
                Routes::match_route(&Routes::default(), &["about"]),
                Routes::About
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["404"]),
                Routes::NotFound
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["about", "something"]),
                Routes::NotFound
            );
        }

        #[test]
        fn router_dyn_params() {
            #[derive(Debug, PartialEq, Eq, Route)]
            enum Routes {
                #[to("/account/<id>")]
                Account { id: u32 },
                #[not_found]
                NotFound,
            }

            assert_eq!(
                Routes::match_route(&Routes::default(), &["account", "123"]),
                Routes::Account { id: 123 }
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["account", "-1"]),
                Routes::NotFound
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["account", "abc"]),
                Routes::NotFound
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["account"]),
                Routes::NotFound
            );
        }

        #[test]
        fn router_multiple_dyn_params() {
            #[derive(Debug, PartialEq, Eq, Route)]
            enum Routes {
                #[to("/hello/<name>/<age>")]
                Hello { name: String, age: u32 },
                #[not_found]
                NotFound,
            }

            assert_eq!(
                Routes::match_route(&Routes::default(), &["hello", "Bob", "21"]),
                Routes::Hello {
                    name: "Bob".to_string(),
                    age: 21
                }
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["hello", "21", "Bob"]),
                Routes::NotFound
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["hello"]),
                Routes::NotFound
            );
        }

        #[test]
        fn router_multiple_dyn_segments() {
            #[derive(Debug, PartialEq, Eq, Route)]
            enum Routes {
                #[to("/path/<path..>")]
                Path { path: Vec<String> },
                #[to("/numbers/<numbers..>")]
                Numbers { numbers: Vec<u32> },
                #[not_found]
                NotFound,
            }

            assert_eq!(
                Routes::match_route(&Routes::default(), &["path", "a", "b", "c"]),
                Routes::Path {
                    path: vec!["a".to_string(), "b".to_string(), "c".to_string()]
                }
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["numbers", "1", "2", "3"]),
                Routes::Numbers {
                    numbers: vec![1, 2, 3]
                }
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["path"]),
                Routes::Path { path: Vec::new() }
            );
        }

        #[test]
        fn router_multiple_dyn_segments_match_lazy() {
            #[derive(Debug, PartialEq, Eq, Route)]
            enum Routes {
                #[to("/path/<path..>/end")]
                Path { path: Vec<u32> },
                #[not_found]
                NotFound,
            }

            assert_eq!(
                Routes::match_route(&Routes::default(), &["path", "1", "2", "end"]),
                Routes::Path { path: vec![1, 2] }
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["path", "end"]),
                Routes::Path { path: Vec::new() }
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["path", "1", "end", "2"]),
                Routes::NotFound
            );
        }

        #[test]
        fn router_dyn_param_before_dyn_segment() {
            #[derive(Debug, PartialEq, Eq, Route)]
            enum Routes {
                #[to("/<param>/<segments..>")]
                Path {
                    param: String,
                    segments: Vec<String>,
                },
                #[not_found]
                NotFound,
            }

            assert_eq!(
                Routes::match_route(&Routes::default(), &["path", "1", "2"]),
                Routes::Path {
                    param: "path".to_string(),
                    segments: vec!["1".to_string(), "2".to_string()]
                }
            );
        }

        #[test]
        fn nested_router() {
            #[derive(Debug, PartialEq, Eq, Route)]
            enum Nested {
                #[to("/nested")]
                Nested,
                #[not_found]
                NotFound,
            }

            #[derive(Debug, PartialEq, Eq, Route)]
            enum Routes {
                #[to("/")]
                Home,
                #[to("/route/<_..>")]
                Route(Nested),
                #[not_found]
                NotFound,
            }

            assert_eq!(Routes::match_route(&Routes::default(), &[]), Routes::Home);
            assert_eq!(
                Routes::match_route(&Routes::default(), &["route", "nested"]),
                Routes::Route(Nested::Nested)
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["route", "404"]),
                Routes::Route(Nested::NotFound)
            );
            assert_eq!(
                Routes::match_route(&Routes::default(), &["404"]),
                Routes::NotFound
            );
        }
    }
}
