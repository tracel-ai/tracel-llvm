/// Returns the provided path surrounded with double quotes
pub fn quote_path(path: &std::path::Path) -> String {
    format!(r#""{}""#, path.to_str().unwrap())
}
