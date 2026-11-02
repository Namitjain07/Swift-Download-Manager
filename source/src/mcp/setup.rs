//! Ready-to-paste setup text for MCP clients.

use serde_json::json;

/// The command that adds this program to Claude Code.
pub fn claude_code_command(exe: &str) -> String {
    format!("claude mcp add swift-download-manager -- \"{exe}\" --mcp")
}

/// The block for Claude Desktop's `claude_desktop_config.json` (and most other MCP clients).
pub fn desktop_config(exe: &str) -> String {
    let config = json!({ "mcpServers": { "swift-download-manager": { "command": exe, "args": ["--mcp"] } } });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn claude_code_command_quotes_the_path_and_adds_the_flag() {
        let c = claude_code_command("/opt/Swift Download Manager/swift_download_manager");
        assert!(c.starts_with("claude mcp add swift-download-manager -- "));
        assert!(c.contains("\"/opt/Swift Download Manager/swift_download_manager\" --mcp"), "{c}");
    }

    #[test]
    fn desktop_config_is_valid_json_with_the_path_and_flag() {
        let exe = r"C:\Program Files\Swift\SwiftDownloadManager.exe";
        let v: Value = serde_json::from_str(&desktop_config(exe)).expect("valid JSON even with backslashes");
        let server = &v["mcpServers"]["swift-download-manager"];
        assert_eq!(server["command"], exe);
        assert_eq!(server["args"], serde_json::json!(["--mcp"]));
    }
}
