use std::collections::HashMap;

pub fn fix(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    if lines.len() == 1 {
        return text.to_string();
    }

    let term_width = estimate_terminal_width(&lines);

    let mut groups: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut heredoc_delims: Vec<String> = Vec::new();

    for line in &lines {
        let trimmed = line.trim();

        if let Some(delim) = extract_heredoc_delim(line) {
            heredoc_delims.push(delim);
        }

        if current.is_empty() {
            current.push(line);
        } else if !heredoc_delims.is_empty() {
            groups.push(std::mem::take(&mut current));
            current.push(line);
        } else if should_join(current.last().unwrap(), line, term_width) {
            current.push(line);
        } else {
            groups.push(std::mem::take(&mut current));
            current.push(line);
        }

        if let Some(delim) = heredoc_delims.last() {
            if trimmed == delim.as_str() {
                heredoc_delims.pop();
            }
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }

    groups
        .iter()
        .map(|g| join_group(g))
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_heredoc_delim(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let mut chars = trimmed.char_indices();
    let mut prev_was_lt = false;

    while let Some((i, c)) = chars.next() {
        if c == '<' {
            if prev_was_lt {
                let rest = &trimmed[i + 1..];
                let rest = rest.trim_start();

                if rest.starts_with('-') {
                    let after = rest[1..].trim_start();
                    return extract_delim_text(after);
                }

                return extract_delim_text(rest);
            }
            prev_was_lt = true;
        } else if c == '>' {
            prev_was_lt = false;
        } else if !c.is_whitespace() {
            prev_was_lt = false;
        }
    }

    None
}

fn extract_delim_text(s: &str) -> Option<String> {
    let s = s.trim_start();

    if s.starts_with('\'') {
        let rest = &s[1..];
        let end = rest.find('\'')?;
        return Some(rest[..end].to_string());
    }
    if s.starts_with('"') {
        let rest = &s[1..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    if s.starts_with('\\') {
        let rest = &s[1..];
        let delim = rest.split_whitespace().next()?;
        return Some(delim.to_string());
    }

    let delim = s.split_whitespace().next()?;
    if !delim.is_empty() {
        return Some(delim.to_string());
    }
    None
}

fn estimate_terminal_width(lines: &[&str]) -> usize {
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for line in lines {
        let len = line.len();
        if len >= 70 && len <= 130 {
            *counts.entry(len).or_insert(0) += 1;
        }
    }

    let threshold = (lines.len() as f64 * 0.15).ceil() as usize;
    counts
        .into_iter()
        .filter(|&(_, count)| count >= threshold)
        .max_by_key(|&(len, count)| (count, len))
        .map(|(len, _)| len)
        .unwrap_or(100)
}

fn should_join(current: &str, next: &str, term_width: usize) -> bool {
    let c_trimmed = current.trim_end();
    let n_trimmed = next.trim_start();

    if c_trimmed.is_empty() || n_trimmed.is_empty() {
        return false;
    }

    let c_len = current.len();

    if c_trimmed.ends_with('\\') || c_trimmed.ends_with('^') || c_trimmed.ends_with('`') {
        return true;
    }

    if c_trimmed.ends_with('|')
        || c_trimmed.ends_with("&&")
        || c_trimmed.ends_with("||")
        || c_trimmed.ends_with("|&")
    {
        return true;
    }

    if n_trimmed.starts_with('-') || n_trimmed.starts_with("--") {
        return true;
    }

    if c_len >= term_width.saturating_sub(10) {
        if !looks_like_command(n_trimmed) {
            return true;
        }
    }

    if n_trimmed.starts_with('>')
        || n_trimmed.starts_with(">>")
        || n_trimmed.starts_with('|')
        || n_trimmed.starts_with("&&")
        || n_trimmed.starts_with("||")
    {
        return true;
    }

    false
}

fn looks_like_command(s: &str) -> bool {
    let first = s.split_whitespace().next().unwrap_or("");
    if first.is_empty() {
        return false;
    }

    let known: &[&str] = &[
        "git", "npm", "npx", "cargo", "rustc", "node", "python", "pip", "pip3", "docker",
        "docker-compose", "kubectl", "helm", "terraform", "aws", "gcloud", "az", "ssh", "scp",
        "curl", "wget", "cd", "ls", "dir", "mkdir", "md", "rmdir", "rm", "del", "cp", "copy",
        "mv", "move", "cat", "type", "echo", "set", "export", "code", "notepad", "start", "cls",
        "clear", "pwd", "sudo", "runas", "chmod", "chown", "grep", "find", "findstr", "sed", "awk",
        "sort", "uniq", "wc", "head", "tail", "tee", "xargs", "touch", "ni", "man", "help", "if",
        "for", "while", "switch", "try", "catch", "foreach", "dotnet", "go", "make", "cmake",
        "deno", "bun",
    ];

    let lower = first.to_lowercase();
    if known.contains(&lower.as_str()) {
        return true;
    }
    if lower.starts_with("get-")
        || lower.starts_with("set-")
        || lower.starts_with("new-")
        || lower.starts_with("start-")
        || lower.starts_with("stop-")
        || lower.starts_with("remove-")
        || lower.starts_with("write-")
        || lower.starts_with("read-")
        || lower.starts_with("invoke-")
        || lower.starts_with("add-")
        || lower.starts_with("clear-")
        || lower.starts_with("convert-")
        || lower.starts_with("enable-")
        || lower.starts_with("disable-")
        || lower.starts_with("enter-")
        || lower.starts_with("exit-")
        || lower.starts_with("export-")
        || lower.starts_with("format-")
        || lower.starts_with("group-")
        || lower.starts_with("import-")
        || lower.starts_with("join-")
        || lower.starts_with("measure-")
        || lower.starts_with("out-")
        || lower.starts_with("pop-")
        || lower.starts_with("push-")
        || lower.starts_with("resolve-")
        || lower.starts_with("restart-")
        || lower.starts_with("resume-")
        || lower.starts_with("select-")
        || lower.starts_with("sort-")
        || lower.starts_with("split-")
        || lower.starts_with("suspend-")
        || lower.starts_with("test-")
        || lower.starts_with("trace-")
        || lower.starts_with("update-")
        || lower.starts_with("wait-")
        || lower.starts_with("where-")
    {
        return true;
    }

    false
}

fn is_continuation_char(c: char) -> bool {
    c == '\\' || c == '^' || c == '`'
}

fn join_group(group: &[&str]) -> String {
    let mut result = String::new();
    for (i, line) in group.iter().enumerate() {
        if i == 0 {
            result.push_str(line.trim_end());
        } else {
            let prev = group[i - 1];
            let prev_trimmed = prev.trim_end();
            if !prev_trimmed.is_empty()
                && is_continuation_char(prev_trimmed.chars().last().unwrap())
            {
                result.pop();
                while result.ends_with(' ') || result.ends_with('\t') {
                    result.pop();
                }
            }

            result.push(' ');
            result.push_str(line.trim_start());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_change_for_short_lines() {
        let input = "git init\nnpm install";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_join_wrapped_line() {
        let input = "npm install --save-dev --save-exact lodash typescript react react-dom react-router-dom\nstyled-components";
        let expected = "npm install --save-dev --save-exact lodash typescript react react-dom react-router-dom styled-components";
        assert_eq!(fix(input), expected);
    }

    #[test]
    fn test_join_flag_continuation() {
        let input = "cargo build\n--release";
        let expected = "cargo build --release";
        assert_eq!(fix(input), expected);
    }

    #[test]
    fn test_join_pipe_continuation() {
        let input = "git log --oneline\n| grep fix";
        let expected = "git log --oneline | grep fix";
        assert_eq!(fix(input), expected);
    }

    #[test]
    fn test_join_backslash_continuation() {
        let input = "npm install \\\n--save-dev lodash";
        let expected = "npm install --save-dev lodash";
        assert_eq!(fix(input), expected);
    }

    #[test]
    fn test_heredoc_not_joined() {
        let input = "cat > file << 'EOF'\nline one\nline two\nEOF";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_heredoc_with_content() {
        let input = "cat > ~/script.sh << 'EOF'\n     #!/bin/bash\n     echo hello\n     export VAR=value\nEOF\nchmod +x ~/script.sh";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_nested_heredoc() {
        let input = "cat > outer << 'OUTER'\ncat > inner << 'INNER'\ncontent\nINNER\nOUTER";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_heredoc_double_quoted_delim() {
        let input = "cat > file << \"EOF\"\nline1\nline2\nEOF";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_heredoc_with_tab_prefix() {
        let input = "cat > file <<- 'EOF'\n\tindented line\nEOF";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_complex_heredoc_script() {
        let input = "cat > ~/use-redrevival.sh << 'EOF'\n     #!/bin/bash\n     cat > ~/.hermes/.env << 'INNER'\n     TERMINAL_SSH_HOST=redrevival.tailb6b666.ts.net\n     TERMINAL_SSH_USER=nothings\n     TERMINAL_SSH_KEY=/home/redsv/.ssh/hermes_redsv_key\n     TERMINAL_SSH_PORT=22\n     TERMINAL_SSH_PERSISTENT=true\n     INNER\n     export TERMINAL_SSH_HOST=redrevival.tailb6b666.ts.net\n     export TERMINAL_SSH_USER=nothings\n     export TERMINAL_SSH_KEY=/home/redsv/.ssh/hermes_redsv_key\n     export TERMINAL_SSH_PORT=22\n     export TERMINAL_SSH_PERSISTENT=true\n     pm2 restart hermes-agent --update-env\n     echo \"Switched to redrevival\"\n     EOF\nchmod +x ~/use-redrevival.sh";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_heredoc_flag_prevention() {
        let input = "cat > file << 'EOF'\n--option=value\n--another=val\nEOF";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_pipe_not_joined_inside_heredoc() {
        let input = "cat > file << 'EOF'\nline with | pipe\nanother | pipe\nEOF";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_anti_join_blank_lines() {
        let input = "git init\n\nnpm install";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_anti_join_two_commands() {
        let input = "git init\nnpm install";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_wrapped_heredoc_start() {
        let input =
            "cat ~/very-long-path-name/use-redrevival.sh << 'EOF'\nheredoc content\nEOF";
        let expected = "cat ~/very-long-path-name/use-redrevival.sh << 'EOF'\nheredoc content\nEOF";
        assert_eq!(fix(input), expected);
    }

    #[test]
    fn test_multiple_heredocs_sequential() {
        let input = "cat > a << 'A'\ncontent a\nA\ncat > b << 'B'\ncontent b\nB";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_unquoted_heredoc() {
        let input = "cat > file << EOF\nline1\nline2\nEOF";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_single_line_no_change() {
        let input = "git commit -m \"hello world\"";
        assert_eq!(fix(input), input);
    }

    #[test]
    fn test_join_and_then_separate() {
        let input = "npm install --save-dev --save-exact lodash typescript react react-dom react-router-dom\nstyled-components\nnpm install react";
        let expected = "npm install --save-dev --save-exact lodash typescript react react-dom react-router-dom styled-components\nnpm install react";
        assert_eq!(fix(input), expected);
    }
}
