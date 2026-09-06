//! VCDSCRIPT AST definitions and parser.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CondOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Var(u8), // 'A'..='Z'
    Const(i32),
    Binary(Box<Expr>, BinOp, Box<Expr>),
}

impl Expr {
    pub fn eval(&self, vars: &[i32; 26]) -> i32 {
        match self {
            Expr::Var(v) => {
                let idx = (v.to_ascii_uppercase() as usize).saturating_sub('A' as usize);
                if idx < 26 {
                    vars[idx]
                } else {
                    0
                }
            }
            Expr::Const(n) => *n,
            Expr::Binary(lhs, op, rhs) => {
                let l = lhs.eval(vars);
                let r = rhs.eval(vars);
                match op {
                    BinOp::Add => l.saturating_add(r),
                    BinOp::Sub => l.saturating_sub(r),
                    BinOp::Mul => l.saturating_mul(r),
                    BinOp::Div => {
                        if r != 0 {
                            l / r
                        } else {
                            0
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Assign(u8, Expr),
    CallIrkey(u8),
    CallTime(u8),
    CallRand(u8),
    DrawCursor(Expr, Expr),
    DrawImage {
        file: String,
        x: Expr,
        y: Expr,
        mode: Expr,
    },
    PlaySound(String),
    PlayVideo {
        file: String,
        start_frame: Expr,
        end_frame: Expr,
        exit_page: Option<String>,
    },
    KaraokeSet(Expr, Expr),
    KaraokeGet {
        index: Expr,
        target_var: u8,
    },
    KaraokeDel(Expr),
    KaraokeIns(Expr, Expr),
    KaraokePlay,
    Goto(Expr),
    Gosub(Expr),
    Return,
    IfThen {
        lhs: Expr,
        op: CondOp,
        rhs: Expr,
        then_stmt: Box<Statement>,
        else_stmt: Option<Box<Statement>>,
    },
    ForTo {
        var: u8,
        start: Expr,
        end: Expr,
    },
    Next(u8),
    End,
    Rem(String),
    Unknown {
        raw: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptProgram {
    pub lines: BTreeMap<u32, Vec<Statement>>,
}

impl ScriptProgram {
    pub fn parse(source: &str) -> Self {
        let mut lines = BTreeMap::new();
        let mut current_line_no = 0u32;

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let (line_no, rest) = parse_line_header(trimmed, current_line_no);
            current_line_no = line_no;

            let stmts = parse_line_statements(rest);
            if !stmts.is_empty() {
                lines.entry(line_no).or_insert_with(Vec::new).extend(stmts);
            }
        }

        Self { lines }
    }
}

/// Splits a line by `:` ignoring colons inside string quotes and colons inside REM comments.
fn split_statements(line: &str) -> Vec<&str> {
    let mut stmts = Vec::new();
    let mut in_quotes = false;
    let mut start = 0;

    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            // If the statement segment starting at `start` begins with REM or ',
            // the rest of the line is comment and must not be split by `:`.
            let current = line[start..=i].trim_start();
            let upper = current.to_ascii_uppercase();
            let is_rem = if upper.starts_with('\'') {
                true
            } else if upper.starts_with("REM") {
                if upper.len() == 3 {
                    if i + 1 < bytes.len() {
                        let next_c = bytes[i + 1] as char;
                        next_c.is_whitespace() || next_c == ':'
                    } else {
                        true
                    }
                } else {
                    current[3..]
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_whitespace() || c == ':')
                }
            } else {
                false
            };

            if is_rem {
                let s = line[start..].trim();
                if !s.is_empty() {
                    stmts.push(s);
                }
                return stmts;
            }

            if bytes[i] == b':' {
                let s = line[start..i].trim();
                if !s.is_empty() {
                    stmts.push(s);
                }
                start = i + 1;
            }
        }
        i += 1;
    }

    let tail = line[start..].trim();
    if !tail.is_empty() {
        stmts.push(tail);
    }

    stmts
}

fn parse_line_header(line: &str, last_line_no: u32) -> (u32, &str) {
    let mut end = 0;
    for c in line.chars() {
        if c.is_ascii_digit() {
            end += c.len_utf8();
        } else {
            break;
        }
    }

    if end > 0 {
        if let Ok(num) = line[..end].parse::<u32>() {
            return (num, line[end..].trim());
        }
    }

    (last_line_no.saturating_add(1), line)
}

fn parse_line_statements(line_rest: &str) -> Vec<Statement> {
    let raw_stmts = split_statements(line_rest);
    let mut stmts = Vec::new();

    for raw in raw_stmts {
        stmts.push(parse_single_statement(raw));
    }

    stmts
}

pub fn parse_single_statement(s: &str) -> Statement {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Statement::Rem(String::new());
    }

    // ' comment
    if trimmed.starts_with('\'') {
        return Statement::Rem(trimmed[1..].trim().to_string());
    }

    let upper = trimmed.to_ascii_uppercase();

    // REM
    if upper.starts_with("REM")
        && (upper.len() == 3
            || upper.chars().nth(3).unwrap().is_whitespace()
            || upper.chars().nth(3).unwrap() == ':')
    {
        return Statement::Rem(trimmed[3..].trim().to_string());
    }

    // END
    if upper == "END" {
        return Statement::End;
    }

    // RETURN
    if upper == "RETURN" {
        return Statement::Return;
    }

    // GOTO <expr>
    if upper.starts_with("GOTO") && (upper.len() == 4 || upper.chars().nth(4).unwrap().is_whitespace()) {
        let rest = trimmed[4..].trim();
        if let Some(expr) = parse_expr(rest) {
            return Statement::Goto(expr);
        } else {
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("无效的 GOTO 行号表达式: '{}'", rest),
            };
        }
    }

    // GOSUB <expr>
    if upper.starts_with("GOSUB") && (upper.len() == 5 || upper.chars().nth(5).unwrap().is_whitespace()) {
        let rest = trimmed[5..].trim();
        if let Some(expr) = parse_expr(rest) {
            return Statement::Gosub(expr);
        } else {
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("无效的 GOSUB 行号表达式: '{}'", rest),
            };
        }
    }

    // CALL IRKEY(X)
    if upper.starts_with("CALL") && upper.contains("IRKEY") {
        if let Some(var) = extract_call_arg_var(trimmed, "IRKEY") {
            return Statement::CallIrkey(var);
        } else {
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: "CALL IRKEY 必须包含单变量参数，例如 CALL IRKEY(X)".to_string(),
            };
        }
    }

    // CALL TIME(X)
    if upper.starts_with("CALL") && upper.contains("TIME") {
        if let Some(var) = extract_call_arg_var(trimmed, "TIME") {
            return Statement::CallTime(var);
        } else {
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: "CALL TIME 必须包含单变量参数，例如 CALL TIME(X)".to_string(),
            };
        }
    }

    // CALL RAND(N)
    if upper.starts_with("CALL") && upper.contains("RAND") {
        if let Some(var) = extract_call_arg_var(trimmed, "RAND") {
            return Statement::CallRand(var);
        } else {
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: "CALL RAND 必须包含单变量参数，例如 CALL RAND(N)".to_string(),
            };
        }
    }

    // DRAWCURSOR X, Y
    if upper.starts_with("DRAWCURSOR") {
        let rest = trimmed[10..].trim();
        let parts: Vec<&str> = rest.split(',').collect();
        if parts.len() == 2 {
            if let (Some(x), Some(y)) = (parse_expr(parts[0].trim()), parse_expr(parts[1].trim())) {
                return Statement::DrawCursor(x, y);
            }
        }
        return Statement::Unknown {
            raw: trimmed.to_string(),
            reason: format!("无效的 DRAWCURSOR 参数: '{}'", rest),
        };
    }

    // DRAWIMAGE "filename", X, Y, mode (also supports DRAWIMGAE typo in disc scripts)
    if upper.starts_with("DRAWIMAGE") || upper.starts_with("DRAWIMGAE") {
        let rest = trimmed[9..].trim();
        if let Some((file, args)) = extract_string_and_args(rest) {
            let parts: Vec<&str> = args.split(',').collect();
            if parts.len() >= 2 {
                let x_expr = parse_expr(parts[0].trim());
                let y_expr = parse_expr(parts[1].trim());
                let mode_expr = if parts.len() >= 3 {
                    parse_expr(parts[2].trim()).unwrap_or(Expr::Const(0))
                } else {
                    Expr::Const(0)
                };

                if let (Some(x), Some(y)) = (x_expr, y_expr) {
                    return Statement::DrawImage {
                        file,
                        x,
                        y,
                        mode: mode_expr,
                    };
                }
            }
        }
        return Statement::Unknown {
            raw: trimmed.to_string(),
            reason: format!("无效的 DRAWIMAGE 参数: '{}'", rest),
        };
    }

    // PLAYSOUND "filename"
    if upper.starts_with("PLAYSOUND") {
        let rest = trimmed[9..].trim();
        if let Some(file) = extract_quoted_string(rest) {
            return Statement::PlaySound(file);
        } else {
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("PLAYSOUND 必须包含带双引号的音频文件名: '{}'", rest),
            };
        }
    }

    // PLAYVIDEO "filename", start_frame, end_frame, "exit_page"
    if upper.starts_with("PLAYVIDEO") {
        let rest = trimmed[9..].trim();
        if let Some((file, args)) = extract_string_and_args(rest) {
            let parts: Vec<&str> = args.split(',').collect();
            let start_frame = if !parts.is_empty() {
                parse_expr(parts[0].trim()).unwrap_or(Expr::Const(0))
            } else {
                Expr::Const(0)
            };
            let end_frame = if parts.len() >= 2 {
                parse_expr(parts[1].trim()).unwrap_or(Expr::Const(0))
            } else {
                Expr::Const(0)
            };
            let exit_page = if parts.len() >= 3 {
                extract_quoted_string(parts[2].trim())
                    .or_else(|| {
                        let p = parts[2].trim();
                        if !p.is_empty() {
                            Some(p.trim_matches('"').to_string())
                        } else {
                            None
                        }
                    })
            } else {
                None
            };

            return Statement::PlayVideo {
                file,
                start_frame,
                end_frame,
                exit_page,
            };
        }
        return Statement::Unknown {
            raw: trimmed.to_string(),
            reason: format!("无效的 PLAYVIDEO 参数: '{}'", rest),
        };
    }

    // KARAOKE / KRAROKE / KRARAOKE subcommands
    let is_karaoke = upper.starts_with("KARAOKE")
        || upper.starts_with("KRAROKE")
        || upper.starts_with("KRARAOKE");
    if is_karaoke {
        // 1. KARAOKE PLAY
        if upper.contains("PLAY") {
            return Statement::KaraokePlay;
        }

        // 2. KARAOKE GET index, var
        if upper.contains("GET") {
            let idx = upper.find("GET").unwrap() + 3;
            let rest = trimmed[idx..].trim();
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() == 2 {
                let var_part = parts[1].trim();
                if var_part.len() == 1 && var_part.chars().next().unwrap().is_ascii_alphabetic() {
                    let var = var_part.chars().next().unwrap().to_ascii_uppercase() as u8;
                    if let Some(index_expr) = parse_expr(parts[0].trim()) {
                        return Statement::KaraokeGet {
                            index: index_expr,
                            target_var: var,
                        };
                    }
                }
            }
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("无效的 KARAOKE GET 参数: '{}'", rest),
            };
        }

        // 3. KARAOKE DEL index
        if upper.contains("DEL") {
            let idx = upper.find("DEL").unwrap() + 3;
            let rest = trimmed[idx..].trim();
            if let Some(index_expr) = parse_expr(rest) {
                return Statement::KaraokeDel(index_expr);
            }
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("无效的 KARAOKE DEL 参数: '{}'", rest),
            };
        }

        // 4. KARAOKE INS index, val
        if upper.contains("INS") {
            let idx = upper.find("INS").unwrap() + 3;
            let rest = trimmed[idx..].trim();
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() == 2 {
                if let (Some(idx_expr), Some(val_expr)) = (parse_expr(parts[0].trim()), parse_expr(parts[1].trim())) {
                    return Statement::KaraokeIns(idx_expr, val_expr);
                }
            }
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("无效的 KARAOKE INS 参数: '{}'", rest),
            };
        }

        // 5. KARAOKE SET index, val
        if upper.contains("SET") {
            let idx = upper.find("SET").unwrap() + 3;
            let rest = trimmed[idx..].trim();
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() == 2 {
                if let (Some(ch), Some(mode)) = (parse_expr(parts[0].trim()), parse_expr(parts[1].trim())) {
                    return Statement::KaraokeSet(ch, mode);
                }
            }
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("无效的 KARAOKE SET 参数: '{}'", rest),
            };
        }
    }

    // IF <cond> THEN <then_stmt> [ELSE <else_stmt>]
    if upper.starts_with("IF") && upper.contains("THEN") {
        let then_pos = upper.find("THEN").unwrap();
        let cond_part = trimmed[2..then_pos].trim();
        let after_then = trimmed[then_pos + 4..].trim();

        let (then_part, else_part) = if let Some(else_idx) = find_keyword_outside_quotes(after_then, "ELSE") {
            (after_then[..else_idx].trim(), Some(after_then[else_idx + 4..].trim()))
        } else {
            (after_then, None)
        };

        if let Some((lhs, op, rhs)) = parse_condition(cond_part) {
            let parse_branch = |branch_str: &str| -> Statement {
                let b_trim = branch_str.trim();
                if let Ok(target_line) = b_trim.parse::<u32>() {
                    Statement::Goto(Expr::Const(target_line as i32))
                } else {
                    parse_single_statement(b_trim)
                }
            };

            let then_stmt = parse_branch(then_part);
            let else_stmt = else_part.map(parse_branch);

            return Statement::IfThen {
                lhs,
                op,
                rhs,
                then_stmt: Box::new(then_stmt),
                else_stmt: else_stmt.map(Box::new),
            };
        } else {
            return Statement::Unknown {
                raw: trimmed.to_string(),
                reason: format!("无法解析 IF 条件表达式: '{}'", cond_part),
            };
        }
    }

    // FOR <var> = <expr> TO <expr>
    if upper.starts_with("FOR") && upper.contains("TO") && upper.contains('=') {
        let eq_pos = upper.find('=').unwrap();
        let to_pos = upper.find("TO").unwrap();
        let var_part = trimmed[3..eq_pos].trim();
        let start_part = trimmed[eq_pos + 1..to_pos].trim();
        let end_part = trimmed[to_pos + 2..].trim();

        if var_part.len() == 1 && var_part.chars().next().unwrap().is_ascii_alphabetic() {
            let var = var_part.chars().next().unwrap().to_ascii_uppercase() as u8;
            if let (Some(start), Some(end)) = (parse_expr(start_part), parse_expr(end_part)) {
                return Statement::ForTo { var, start, end };
            }
        }
        return Statement::Unknown {
            raw: trimmed.to_string(),
            reason: format!("无法解析 FOR 循环语句: '{}'", trimmed),
        };
    }

    // NEXT <var>
    if upper.starts_with("NEXT") {
        let rest = trimmed[4..].trim();
        let var = if rest.len() == 1 && rest.chars().next().unwrap().is_ascii_alphabetic() {
            rest.chars().next().unwrap().to_ascii_uppercase() as u8
        } else {
            b'I'
        };
        return Statement::Next(var);
    }

    // Variable Assignment: <Var> = <Expr>
    if let Some(eq_idx) = trimmed.find('=') {
        let var_part = trimmed[..eq_idx].trim();
        let val_part = trimmed[eq_idx + 1..].trim();
        if var_part.len() == 1 && var_part.chars().next().unwrap().is_ascii_alphabetic() {
            let var = var_part.chars().next().unwrap().to_ascii_uppercase() as u8;
            if let Some(expr) = parse_expr(val_part) {
                return Statement::Assign(var, expr);
            } else {
                return Statement::Unknown {
                    raw: trimmed.to_string(),
                    reason: format!("无效的赋值表达式: '{}'", val_part),
                };
            }
        }
    }

    Statement::Unknown {
        raw: trimmed.to_string(),
        reason: format!("未识别的 VCDSCRIPT 指令: '{}'", trimmed),
    }
}

fn extract_call_arg_var(s: &str, func_name: &str) -> Option<u8> {
    let upper = s.to_ascii_uppercase();
    let idx = upper.find(func_name)?;
    let after = &s[idx + func_name.len()..];
    let open_paren = after.find('(')?;
    let close_paren = after.find(')')?;
    if close_paren > open_paren {
        let arg = after[open_paren + 1..close_paren].trim();
        if arg.len() == 1 && arg.chars().next()?.is_ascii_alphabetic() {
            return Some(arg.chars().next()?.to_ascii_uppercase() as u8);
        }
    }
    None
}

fn extract_quoted_string(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let end = s[start + 1..].find('"')?;
    Some(s[start + 1..start + 1 + end].to_string())
}

fn extract_string_and_args(s: &str) -> Option<(String, &str)> {
    let start = s.find('"')?;
    let end = s[start + 1..].find('"')?;
    let string_val = s[start + 1..start + 1 + end].to_string();
    let rest = s[start + 1 + end + 1..].trim();
    let args = if rest.starts_with(',') {
        rest[1..].trim()
    } else {
        rest
    };
    Some((string_val, args))
}

fn parse_condition(s: &str) -> Option<(Expr, CondOp, Expr)> {
    let ops = [
        ("<=", CondOp::Le),
        (">=", CondOp::Ge),
        ("<>", CondOp::Ne),
        ("=", CondOp::Eq),
        ("<", CondOp::Lt),
        (">", CondOp::Gt),
    ];

    for (symbol, op) in ops {
        if let Some(idx) = s.find(symbol) {
            let lhs_str = s[..idx].trim();
            let rhs_str = s[idx + symbol.len()..].trim();
            if let (Some(lhs), Some(rhs)) = (parse_expr(lhs_str), parse_expr(rhs_str)) {
                return Some((lhs, op, rhs));
            }
        }
    }

    None
}

/// Recursive descent parser for expressions with operator precedence (+, -, *, /).
pub fn parse_expr(s: &str) -> Option<Expr> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Binary Add/Sub (lowest precedence outside parens)
    let mut paren_depth = 0;
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => paren_depth += 1,
            b'(' => paren_depth -= 1,
            b'+' if paren_depth == 0 => {
                let lhs = parse_expr(&s[..i])?;
                let rhs = parse_expr(&s[i + 1..])?;
                return Some(Expr::Binary(Box::new(lhs), BinOp::Add, Box::new(rhs)));
            }
            b'-' if paren_depth == 0 && i > 0 => {
                let lhs = parse_expr(&s[..i])?;
                let rhs = parse_expr(&s[i + 1..])?;
                return Some(Expr::Binary(Box::new(lhs), BinOp::Sub, Box::new(rhs)));
            }
            _ => {}
        }
    }

    // Binary Mul/Div (higher precedence outside parens)
    paren_depth = 0;
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => paren_depth += 1,
            b'(' => paren_depth -= 1,
            b'*' if paren_depth == 0 => {
                let lhs = parse_expr(&s[..i])?;
                let rhs = parse_expr(&s[i + 1..])?;
                return Some(Expr::Binary(Box::new(lhs), BinOp::Mul, Box::new(rhs)));
            }
            b'/' if paren_depth == 0 => {
                let lhs = parse_expr(&s[..i])?;
                let rhs = parse_expr(&s[i + 1..])?;
                return Some(Expr::Binary(Box::new(lhs), BinOp::Div, Box::new(rhs)));
            }
            _ => {}
        }
    }

    // Parentheses
    if s.starts_with('(') && s.ends_with(')') {
        return parse_expr(&s[1..s.len() - 1]);
    }

    // Unary negative
    if s.starts_with('-') {
        let inner = parse_expr(&s[1..])?;
        return Some(Expr::Binary(
            Box::new(Expr::Const(0)),
            BinOp::Sub,
            Box::new(inner),
        ));
    }

    // Single Variable (A..Z)
    if s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic() {
        return Some(Expr::Var(
            s.chars().next().unwrap().to_ascii_uppercase() as u8,
        ));
    }

    // Integer Constant
    if let Ok(num) = s.parse::<i32>() {
        return Some(Expr::Const(num));
    }

    None
}

/// Finds the index of a case-insensitive keyword `kw` outside string quotes, respecting word boundaries.
fn find_keyword_outside_quotes(s: &str, kw: &str) -> Option<usize> {
    let mut in_quotes = false;
    let bytes = s.as_bytes();
    let kw_bytes = kw.as_bytes();
    let kw_len = kw_bytes.len();

    let mut i = 0;
    while i + kw_len <= bytes.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }
        if !in_quotes && s[i..i + kw_len].eq_ignore_ascii_case(kw) {
            let prev_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
            let next_pos = i + kw_len;
            let next_ok = next_pos == bytes.len() || bytes[next_pos].is_ascii_whitespace();
            if prev_ok && next_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

