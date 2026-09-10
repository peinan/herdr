//! Custom pane-title formats (`[ui] pane_title_format`).
//!
//! A starship-style subset: literal text, `$var` / `${var}` variables, and
//! conditional groups `( … )` that drop entirely when every variable inside
//! them resolves empty. The format is parsed once into a typed [`Segment`] AST
//! and later rendered by the pure [`expand`] against per-pane [`TitleFacts`].
//!
//! Parsing and expansion are both pure: no I/O, no state mutation.

use std::fmt;

/// A resolvable field of a pane's title.
///
/// Each variant maps to one `$name` in the format and to one resolved string
/// in [`TitleFacts`]. An empty resolved string means "not applicable" and
/// drives conditional-group elision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleField {
    /// Basename of the pane cwd (`$HOME` shown as `~`).
    Dir,
    /// Full pane cwd, with `$HOME` abbreviated to `~`.
    Cwd,
    /// Foreground process name running in the pane.
    Process,
    /// Detected/reported agent label.
    Agent,
    /// Git branch for the pane's repository.
    Branch,
    /// Ahead/behind marker relative to upstream (e.g. `↑2↓1`).
    AheadBehind,
    /// Working-tree status marks (e.g. dirty/staged).
    GitStatus,
    /// Zoom indicator marker, shown only on the zoomed pane.
    Zoom,
    /// Manual pane label set by the user.
    Label,
}

impl TitleField {
    /// Resolve a `$name` (without the `$`) to its field.
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "dir" => Self::Dir,
            "cwd" => Self::Cwd,
            "process" => Self::Process,
            "agent" => Self::Agent,
            "branch" => Self::Branch,
            "ahead_behind" => Self::AheadBehind,
            "git_status" => Self::GitStatus,
            "zoom" => Self::Zoom,
            "label" => Self::Label,
            _ => return None,
        })
    }
}

/// One node of a parsed pane-title format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Verbatim text (escapes already resolved).
    Literal(String),
    /// A `$var` reference.
    Var(TitleField),
    /// A conditional group `( … )`. Renders nothing when every [`Var`]
    /// reachable inside resolves to an empty string.
    ///
    /// [`Var`]: Segment::Var
    Group(Vec<Segment>),
}

/// A pane-title format that failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

/// Resolved per-pane values for every [`TitleField`].
///
/// Each field holds its already-formatted display string; an empty string
/// means the field does not apply to this pane and any conditional group that
/// depends only on empty fields is dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleFacts {
    pub dir: String,
    pub cwd: String,
    pub process: String,
    pub agent: String,
    pub branch: String,
    pub ahead_behind: String,
    pub git_status: String,
    pub zoom: String,
    pub label: String,
}

impl TitleFacts {
    /// The resolved string for `field`.
    pub fn field(&self, field: TitleField) -> &str {
        match field {
            TitleField::Dir => &self.dir,
            TitleField::Cwd => &self.cwd,
            TitleField::Process => &self.process,
            TitleField::Agent => &self.agent,
            TitleField::Branch => &self.branch,
            TitleField::AheadBehind => &self.ahead_behind,
            TitleField::GitStatus => &self.git_status,
            TitleField::Zoom => &self.zoom,
            TitleField::Label => &self.label,
        }
    }
}

/// Whether `segments` reference `field` anywhere, including inside groups.
///
/// Lets consumers such as the git refresh demand ask which facts a format
/// actually needs without expanding it.
pub fn references(segments: &[Segment], field: TitleField) -> bool {
    segments.iter().any(|segment| match segment {
        Segment::Literal(_) => false,
        Segment::Var(var) => *var == field,
        Segment::Group(inner) => references(inner, field),
    })
}

/// Parse a `[ui] pane_title_format` string into a [`Segment`] sequence.
///
/// Grammar (v1):
/// - `$name` / `${name}` — a variable; unknown names are an error.
/// - `( … )` — a conditional group; groups nest. Unbalanced parens are an error.
/// - `\$ \( \) \\` — escapes for the four metacharacters.
/// - anything else is literal text.
pub fn parse(input: &str) -> Result<Vec<Segment>, ParseError> {
    let mut parser = Parser {
        chars: input.chars().peekable(),
    };
    let segments = parser.parse_segments(true)?;
    Ok(segments)
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Parser<'_> {
    /// Parse segments until end of input (`top_level`) or a closing `)`.
    ///
    /// On `)` the closing paren is consumed and parsing returns; at top level a
    /// stray `)` is an error and EOF ends parsing. Adjacent literal characters
    /// are coalesced into one [`Segment::Literal`].
    fn parse_segments(&mut self, top_level: bool) -> Result<Vec<Segment>, ParseError> {
        let mut segments = Vec::new();
        let mut literal = String::new();

        macro_rules! flush_literal {
            () => {
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
            };
        }

        while let Some(&ch) = self.chars.peek() {
            match ch {
                '\\' => {
                    self.chars.next();
                    match self.chars.next() {
                        Some(escaped @ ('$' | '(' | ')' | '\\')) => literal.push(escaped),
                        Some(other) => {
                            return Err(ParseError::new(format!(
                                "invalid escape '\\{other}'; only \\$ \\( \\) \\\\ are supported"
                            )));
                        }
                        None => {
                            return Err(ParseError::new("trailing '\\' with nothing to escape"));
                        }
                    }
                }
                '$' => {
                    self.chars.next();
                    let field = self.parse_variable()?;
                    flush_literal!();
                    segments.push(Segment::Var(field));
                }
                '(' => {
                    self.chars.next();
                    flush_literal!();
                    let inner = self.parse_segments(false)?;
                    segments.push(Segment::Group(inner));
                }
                ')' => {
                    if top_level {
                        return Err(ParseError::new("unmatched ')' in pane title format"));
                    }
                    self.chars.next();
                    flush_literal!();
                    return Ok(segments);
                }
                _ => {
                    self.chars.next();
                    literal.push(ch);
                }
            }
        }

        if !top_level {
            return Err(ParseError::new("unmatched '(' in pane title format"));
        }
        flush_literal!();
        Ok(segments)
    }

    /// Parse the name after a `$`, in either `$name` or `${name}` form.
    fn parse_variable(&mut self) -> Result<TitleField, ParseError> {
        let braced = self.chars.peek() == Some(&'{');
        if braced {
            self.chars.next();
        }

        let mut name = String::new();
        if braced {
            while let Some(&ch) = self.chars.peek() {
                if ch == '}' {
                    break;
                }
                name.push(ch);
                self.chars.next();
            }
            if self.chars.next() != Some('}') {
                return Err(ParseError::new("unterminated '${' in pane title format"));
            }
        } else {
            while let Some(&ch) = self.chars.peek() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    name.push(ch);
                    self.chars.next();
                } else {
                    break;
                }
            }
        }

        if name.is_empty() {
            return Err(ParseError::new("empty variable name after '$'"));
        }
        TitleField::from_name(&name)
            .ok_or_else(|| ParseError::new(format!("unknown pane title variable '${name}'")))
    }
}

/// Render parsed `segments` against `facts`.
///
/// Pure. A [`Segment::Group`] contributes nothing when every [`Segment::Var`]
/// reachable inside it (through nested groups) resolves to an empty string;
/// otherwise the group's literals and non-empty variables render in place.
pub fn expand(segments: &[Segment], facts: &TitleFacts) -> String {
    let mut out = String::new();
    expand_into(segments, facts, &mut out);
    out
}

fn expand_into(segments: &[Segment], facts: &TitleFacts, out: &mut String) {
    for segment in segments {
        match segment {
            Segment::Literal(text) => out.push_str(text),
            Segment::Var(field) => out.push_str(facts.field(*field)),
            Segment::Group(inner) => {
                if group_has_value(inner, facts) {
                    expand_into(inner, facts, out);
                }
            }
        }
    }
}

/// Whether a group should render: true when at least one variable inside it
/// (including nested groups) resolves non-empty. A group with no variables at
/// all never renders, matching starship's treatment of variable-free groups.
fn group_has_value(segments: &[Segment], facts: &TitleFacts) -> bool {
    segments.iter().any(|segment| match segment {
        Segment::Literal(_) => false,
        Segment::Var(field) => !facts.field(*field).is_empty(),
        Segment::Group(inner) => group_has_value(inner, facts),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_finds_fields_inside_groups_only_when_present() {
        let segments = parse("$dir( $branch$git_status)").expect("parse");
        assert!(references(&segments, TitleField::Dir));
        assert!(references(&segments, TitleField::Branch));
        assert!(references(&segments, TitleField::GitStatus));
        assert!(!references(&segments, TitleField::AheadBehind));
        assert!(!references(&[], TitleField::Dir));
    }

    fn facts() -> TitleFacts {
        TitleFacts {
            dir: "herdr".into(),
            cwd: "~/code/herdr".into(),
            process: "nvim".into(),
            agent: "claude".into(),
            branch: "main".into(),
            ahead_behind: "↑2".into(),
            git_status: "*".into(),
            zoom: "Z".into(),
            label: "build".into(),
        }
    }

    fn expand_str(input: &str, facts: &TitleFacts) -> String {
        expand(&parse(input).expect("parse"), facts)
    }

    #[test]
    fn each_variable_expands() {
        let f = facts();
        assert_eq!(expand_str("$dir", &f), "herdr");
        assert_eq!(expand_str("$cwd", &f), "~/code/herdr");
        assert_eq!(expand_str("$process", &f), "nvim");
        assert_eq!(expand_str("$agent", &f), "claude");
        assert_eq!(expand_str("$branch", &f), "main");
        assert_eq!(expand_str("$ahead_behind", &f), "↑2");
        assert_eq!(expand_str("$git_status", &f), "*");
        assert_eq!(expand_str("$zoom", &f), "Z");
        assert_eq!(expand_str("$label", &f), "build");
    }

    #[test]
    fn braced_variable_expands() {
        let f = facts();
        assert_eq!(expand_str("${dir}", &f), "herdr");
        // Braces delimit the name so adjacent text is not swallowed.
        assert_eq!(expand_str("${dir}x", &f), "herdrx");
        assert_eq!(expand_str("${branch}_${dir}", &f), "main_herdr");
    }

    #[test]
    fn literals_and_variables_interleave() {
        let f = facts();
        assert_eq!(expand_str("$dir $process", &f), "herdr nvim");
        assert_eq!(expand_str("[$agent]", &f), "[claude]");
    }

    #[test]
    fn empty_field_expands_empty() {
        let mut f = facts();
        f.process = String::new();
        assert_eq!(expand_str("$dir$process", &f), "herdr");
    }

    #[test]
    fn group_drops_when_all_vars_empty() {
        let mut f = facts();
        f.branch = String::new();
        f.ahead_behind = String::new();
        f.git_status = String::new();
        assert_eq!(
            expand_str("$dir( ⋅ $branch$ahead_behind$git_status)", &f),
            "herdr"
        );
    }

    #[test]
    fn group_renders_when_any_var_present() {
        let mut f = facts();
        f.ahead_behind = String::new();
        f.git_status = String::new();
        // branch is still set, so the whole group (separator included) stays.
        assert_eq!(
            expand_str("$dir( ⋅ $branch$ahead_behind$git_status)", &f),
            "herdr ⋅ main"
        );
    }

    #[test]
    fn nested_groups() {
        let mut f = facts();
        f.branch = "main".into();
        f.ahead_behind = String::new();
        f.git_status = String::new();
        // Outer group renders (branch set); inner group drops (counts empty).
        assert_eq!(
            expand_str("$dir( on $branch( $ahead_behind$git_status))", &f),
            "herdr on main"
        );

        // Inner non-empty keeps both groups.
        f.ahead_behind = "↑1".into();
        assert_eq!(
            expand_str("$dir( on $branch( $ahead_behind$git_status))", &f),
            "herdr on main ↑1"
        );
    }

    #[test]
    fn group_without_variables_never_renders() {
        let f = facts();
        // A group of pure literals has no variable to make it non-empty.
        assert_eq!(expand_str("$dir(literal)", &f), "herdr");
    }

    #[test]
    fn escapes_resolve_to_literals() {
        let f = facts();
        assert_eq!(expand_str("\\$dir", &f), "$dir");
        assert_eq!(expand_str("\\(\\)", &f), "()");
        assert_eq!(expand_str("a\\\\b", &f), "a\\b");
        // Escaped paren is literal, not a group boundary.
        assert_eq!(expand_str("$dir \\( $branch \\)", &f), "herdr ( main )");
    }

    #[test]
    fn user_example_outside_repo_drops_group() {
        // The user's real title; outside a repo branch/ahead/status are empty.
        let mut f = TitleFacts {
            dir: "scratch".into(),
            process: "zsh".into(),
            ..TitleFacts::default()
        };
        assert_eq!(
            expand_str("$dir $process( ⋅ $branch$ahead_behind$git_status)", &f),
            "scratch zsh"
        );
        // Inside a repo the group appears.
        f.branch = "main".into();
        assert_eq!(
            expand_str("$dir $process( ⋅ $branch$ahead_behind$git_status)", &f),
            "scratch zsh ⋅ main"
        );
    }

    #[test]
    fn empty_format_parses_to_no_segments() {
        assert_eq!(parse("").expect("parse"), Vec::new());
    }

    #[test]
    fn unknown_variable_is_error() {
        let err = parse("$bogus").expect_err("should fail");
        assert!(err.to_string().contains("bogus"), "{err}");
    }

    #[test]
    fn unknown_braced_variable_is_error() {
        assert!(parse("${nope}").is_err());
    }

    #[test]
    fn unbalanced_open_paren_is_error() {
        assert!(parse("$dir (oops").is_err());
    }

    #[test]
    fn unbalanced_close_paren_is_error() {
        assert!(parse("oops)").is_err());
    }

    #[test]
    fn unterminated_brace_is_error() {
        assert!(parse("${dir").is_err());
    }

    #[test]
    fn empty_variable_name_is_error() {
        assert!(parse("$ ").is_err());
        assert!(parse("${}").is_err());
    }

    #[test]
    fn invalid_escape_is_error() {
        assert!(parse("\\n").is_err());
        assert!(parse("trailing\\").is_err());
    }
}
