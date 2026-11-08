//! What a tool call can end in. An assistant is told the kind (so it can decide what to do next: fix an
//! argument, wait and retry, or give up and ask the person), the message and, where one helps, a hint.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// A missing or wrong argument, or a name the app does not know (a queue, a category, a status).
    InvalidArgument,
    /// No download with that id.
    NotFound,
    /// Not allowed: the access level, the list of sites, private addresses.
    Forbidden,
    /// Switched off in the settings.
    Disabled,
    /// Too many requests; try again later.
    RateLimited,
    /// The app is busy, starting or not answering; try again in a moment.
    Busy,
    /// Cannot be done in the state things are in (already complete, not finished yet).
    Conflict,
    /// The remote server or the network failed.
    Unreachable,
    /// A failure inside the app.
    Internal,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::InvalidArgument => "invalid_argument",
            ErrorKind::NotFound => "not_found",
            ErrorKind::Forbidden => "forbidden",
            ErrorKind::Disabled => "disabled",
            ErrorKind::RateLimited => "rate_limited",
            ErrorKind::Busy => "busy",
            ErrorKind::Conflict => "conflict",
            ErrorKind::Unreachable => "unreachable",
            ErrorKind::Internal => "internal",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            ErrorKind::InvalidArgument,
            ErrorKind::NotFound,
            ErrorKind::Forbidden,
            ErrorKind::Disabled,
            ErrorKind::RateLimited,
            ErrorKind::Busy,
            ErrorKind::Conflict,
            ErrorKind::Unreachable,
            ErrorKind::Internal,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolError {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ToolError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        ToolError { kind, message: message.into(), hint: None }
    }

    /// What to do about it.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidArgument, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Forbidden, message)
    }

    /// For tests: does the message (or the hint) say this?
    #[cfg(test)]
    pub fn contains(&self, needle: &str) -> bool {
        self.message.contains(needle) || self.hint.as_deref().is_some_and(|h| h.contains(needle))
    }

    /// The text an assistant reads: kind, message and hint in one piece.
    pub fn text(&self) -> String {
        match &self.hint {
            Some(h) => format!("Error ({}): {} {}", self.kind.as_str(), self.message, h),
            None => format!("Error ({}): {}", self.kind.as_str(), self.message),
        }
    }

    /// The same as data, for `structuredContent`.
    pub fn to_json(&self) -> Value {
        json!({ "error": self })
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ToolError {}

/// Messages made elsewhere in the app as plain text are sorted into kinds here, by what they say, so the
/// many places that report a problem need not all know about kinds. The tests below pin the sentences down.
fn classify(message: &str) -> ErrorKind {
    let starts = |p: &str| message.starts_with(p);
    let has = |p: &str| message.contains(p);
    if starts("No download with id") {
        ErrorKind::NotFound
    } else if has("is turned off") {
        ErrorKind::Disabled
    } else if starts("Unknown ") || starts("Give at least") || starts("At most ") || starts("Only http") || starts("A SHA-256") || starts("Network names") || has("doesn't look like a web address") || has("is not a ") {
        ErrorKind::InvalidArgument
    } else if starts("Only an administrator") || starts("That can only be done") {
        ErrorKind::Forbidden
    } else if has("is already complete") || has("is not finished yet") || has("is not complete") {
        ErrorKind::Conflict
    } else if has("still opening") || has("in use by another") || has("stayed busy") || has("already running the downloads") || has("not answering") || has("did not answer") || has("Try again") {
        ErrorKind::Busy
    } else {
        ErrorKind::Internal
    }
}

impl From<String> for ToolError {
    fn from(message: String) -> Self {
        ToolError { kind: classify(&message), message, hint: None }
    }
}

impl From<&str> for ToolError {
    fn from(message: &str) -> Self {
        ToolError::from(message.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_from_the_app_get_the_right_kind() {
        let kind = |m: &str| ToolError::from(m).kind;
        assert_eq!(kind("No download with id 7. Use list_downloads to see the ids."), ErrorKind::NotFound);
        assert_eq!(kind("MCP is turned off in Swift Download Manager (Options > AI assistants)."), ErrorKind::Disabled);
        assert_eq!(kind("Unknown queue \"x\". Available: Main Queue"), ErrorKind::InvalidArgument);
        assert_eq!(kind("Unknown category \"x\". Available: Video"), ErrorKind::InvalidArgument);
        assert_eq!(kind("Only http:// and https:// links are supported."), ErrorKind::InvalidArgument);
        assert_eq!(kind("\"nope\" doesn't look like a web address."), ErrorKind::InvalidArgument);
        assert_eq!(kind("Download 3 is already complete."), ErrorKind::Conflict);
        assert_eq!(kind("Download 3 is not finished yet."), ErrorKind::Conflict);
        assert_eq!(kind("The app window is still opening. Try again in a moment."), ErrorKind::Busy);
        assert_eq!(kind("Only an administrator can do that."), ErrorKind::Forbidden);
        assert_eq!(kind("something nobody foresaw"), ErrorKind::Internal);
    }

    #[test]
    fn text_and_json_carry_kind_message_and_hint() {
        let e = ToolError::new(ErrorKind::RateLimited, "Too many downloads were added this hour.").hint("Try again in 12 minutes.");
        assert_eq!(e.text(), "Error (rate_limited): Too many downloads were added this hour. Try again in 12 minutes.");
        let j = e.to_json();
        assert_eq!(j["error"]["kind"], "rate_limited");
        assert_eq!(j["error"]["hint"], "Try again in 12 minutes.");
        assert!(ToolError::invalid("x").to_json()["error"].get("hint").is_none(), "no hint, no field");
        assert_eq!(ErrorKind::parse("not_found"), Some(ErrorKind::NotFound));
        assert_eq!(ErrorKind::parse("nope"), None);
    }
}
