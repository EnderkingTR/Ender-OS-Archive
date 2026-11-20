//! Ender-OS kabuk komut ayrıştırıcısı (tokenizer + pipeline/redirect parser).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectMode {
    Truncate,
    Append,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    pub path: String,
    pub mode: RedirectMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub program: String,
    pub args: Vec<String>,
    pub stdout_redirect: Option<Redirect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub stages: Vec<Stage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Pipe,
    RedirectTruncate,
    RedirectAppend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quote {
    None,
    Single,
    Double,
}

fn flush_word(tokens: &mut Vec<Token>, current: &mut String, has_token: &mut bool) {
    if *has_token {
        tokens.push(Token::Word(std::mem::take(current)));
        *has_token = false;
    }
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut has_token = false;
    let mut quote = Quote::None;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            Quote::Single => {
                if ch == '\'' {
                    quote = Quote::None;
                } else {
                    current.push(ch);
                }
            }
            Quote::Double => {
                if ch == '"' {
                    quote = Quote::None;
                } else if ch == '\\' {
                    if let Some(&next) = chars.peek() {
                        if next == '"' || next == '\\' {
                            current.push(next);
                            chars.next();
                        } else {
                            current.push(ch);
                        }
                    } else {
                        return Err("Kapanmamış kaçış karakteri ('\\').".to_string());
                    }
                } else {
                    current.push(ch);
                }
            }
            Quote::None => match ch {
                c if c.is_whitespace() => flush_word(&mut tokens, &mut current, &mut has_token),
                '#' if !has_token => break,
                '\'' => {
                    quote = Quote::Single;
                    has_token = true;
                }
                '"' => {
                    quote = Quote::Double;
                    has_token = true;
                }
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                        has_token = true;
                    } else {
                        return Err("Satır sonunda tamamlanmamış '\\'.".to_string());
                    }
                }
                '|' => {
                    flush_word(&mut tokens, &mut current, &mut has_token);
                    tokens.push(Token::Pipe);
                }
                '>' => {
                    flush_word(&mut tokens, &mut current, &mut has_token);
                    if chars.peek() == Some(&'>') {
                        chars.next();
                        tokens.push(Token::RedirectAppend);
                    } else {
                        tokens.push(Token::RedirectTruncate);
                    }
                }
                _ => {
                    current.push(ch);
                    has_token = true;
                }
            },
        }
    }

    if quote != Quote::None {
        return Err("Kapanmamış tırnak işareti.".to_string());
    }

    flush_word(&mut tokens, &mut current, &mut has_token);
    Ok(tokens)
}

/// Bir kabuk satırını boru hattı (`|`) ve çıktı yönlendirme (`>`, `>>`) aşamalarına ayırır.
pub fn parse_pipeline(input: &str) -> Result<Option<Pipeline>, String> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Ok(None);
    }

    let mut stages = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut stdout_redirect: Option<Redirect> = None;
    let mut iter = tokens.into_iter();

    while let Some(tok) = iter.next() {
        match tok {
            Token::Word(w) => words.push(w),
            Token::RedirectTruncate | Token::RedirectAppend => {
                let mode = if tok == Token::RedirectAppend {
                    RedirectMode::Append
                } else {
                    RedirectMode::Truncate
                };
                match iter.next() {
                    Some(Token::Word(path)) => {
                        stdout_redirect = Some(Redirect { path, mode });
                    }
                    _ => {
                        return Err("Yönlendirme ('>' / '>>') için hedef dosya adı eksik.".to_string());
                    }
                }
            }
            Token::Pipe => {
                if words.is_empty() {
                    return Err("'|' öncesinde geçerli bir komut yok.".to_string());
                }
                let program = words.remove(0);
                stages.push(Stage {
                    program,
                    args: std::mem::take(&mut words),
                    stdout_redirect: stdout_redirect.take(),
                });
            }
        }
    }

    if words.is_empty() {
        return Err("Boru hattı ('|') veya yönlendirme sonrasında komut eksik.".to_string());
    }

    let program = words.remove(0);
    stages.push(Stage {
        program,
        args: words,
        stdout_redirect,
    });

    Ok(Some(Pipeline { stages }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        let p = parse_pipeline("ls -la /bin").unwrap().unwrap();
        assert_eq!(p.stages.len(), 1);
        assert_eq!(p.stages[0].program, "ls");
        assert_eq!(p.stages[0].args, vec!["-la", "/bin"]);
        assert_eq!(p.stages[0].stdout_redirect, None);
    }

    #[test]
    fn parses_quoted_arguments() {
        let p = parse_pipeline("echo \"Merhaba Ender-OS\" 'test 123'")
            .unwrap()
            .unwrap();
        assert_eq!(p.stages[0].program, "echo");
        assert_eq!(p.stages[0].args, vec!["Merhaba Ender-OS", "test 123"]);
    }

    #[test]
    fn parses_truncate_and_append_redirection() {
        let p1 = parse_pipeline("echo selam > /tmp/not.txt").unwrap().unwrap();
        assert_eq!(
            p1.stages[0].stdout_redirect,
            Some(Redirect {
                path: "/tmp/not.txt".to_string(),
                mode: RedirectMode::Truncate,
            })
        );

        let p2 = parse_pipeline("echo ek >>/tmp/not.txt").unwrap().unwrap();
        assert_eq!(
            p2.stages[0].stdout_redirect,
            Some(Redirect {
                path: "/tmp/not.txt".to_string(),
                mode: RedirectMode::Append,
            })
        );
    }

    #[test]
    fn parses_multi_stage_pipeline() {
        let p = parse_pipeline("cat /proc/meminfo | grep Mem | head -n 2")
            .unwrap()
            .unwrap();
        assert_eq!(p.stages.len(), 3);
        assert_eq!(p.stages[0].program, "cat");
        assert_eq!(p.stages[0].args, vec!["/proc/meminfo"]);
        assert_eq!(p.stages[1].program, "grep");
        assert_eq!(p.stages[1].args, vec!["Mem"]);
        assert_eq!(p.stages[2].program, "head");
        assert_eq!(p.stages[2].args, vec!["-n", "2"]);
    }

    #[test]
    fn ignores_empty_and_comment_lines() {
        assert_eq!(parse_pipeline("   \t  ").unwrap(), None);
        assert_eq!(parse_pipeline("   # bu bir yorumdur").unwrap(), None);
        let p = parse_pipeline("pwd # geçerli dizin").unwrap().unwrap();
        assert_eq!(p.stages[0].program, "pwd");
        assert!(p.stages[0].args.is_empty());
    }

    #[test]
    fn rejects_syntax_errors() {
        assert!(parse_pipeline("echo \"kapanmamış").is_err());
        assert!(parse_pipeline("ls |").is_err());
        assert!(parse_pipeline("| ls").is_err());
        assert!(parse_pipeline("echo merhaba >").is_err());
    }
}
