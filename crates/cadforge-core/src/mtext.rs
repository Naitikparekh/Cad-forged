//! Inline formatting of multiline text (the codes AutoCAD stores inside MTEXT): bold and italic from `\F`, underline /
//! overline / strike-through (`\L \O \K`), colour (`\C` index, `\c` true colour), height (`\H`), stacked fractions (`\S`),
//! `{ }` groups, `\P` paragraphs, the `%%u %%o %%d %%p %%c` shortcuts, and the escapes `\\ \{ \} \~ \U+XXXX`.
//!
//! Not applied (the codes are read and dropped): font names (everything is drawn in the one built-in font), width factor
//! `\W`, tracking `\T`, oblique angle `\Q`, alignment `\A`, columns.
use crate::style::Color;

#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub overline: bool,
    pub strike: bool,
    pub color: Option<Color>,
    /// Height relative to the text object's height.
    pub scale: f64,
}

impl Default for Style {
    fn default() -> Style {
        Style { bold: false, italic: false, underline: false, overline: false, strike: false, color: None, scale: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(String),
    /// A stacked fraction: `bar` is false for the tolerance form (`\Sa^b;`).
    Stack { num: String, den: String, bar: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub style: Style,
    pub piece: Piece,
}

/// Whether `s` holds anything [`parse`] would treat specially. `mtext` turns on the backslash and brace codes (multiline
/// text); the `%%` shortcuts apply to every text.
pub fn has_formatting(s: &str, mtext: bool) -> bool {
    s.contains("%%") || (mtext && (s.contains('\\') || s.contains('{') || s.contains('}')))
}

/// Split `s` into paragraphs of runs. `base_h` is the text height, the reference for absolute `\H` heights.
pub fn parse(s: &str, base_h: f64, mtext: bool) -> Vec<Vec<Run>> {
    let chars: Vec<char> = s.chars().collect();
    let mut st = Style::default();
    let mut groups: Vec<Style> = vec![];
    let mut cur = String::new();
    let mut runs: Vec<Run> = vec![];
    let mut paras: Vec<Vec<Run>> = vec![];
    let flush = |cur: &mut String, st: &Style, runs: &mut Vec<Run>| {
        if !cur.is_empty() {
            runs.push(Run { style: st.clone(), piece: Piece::Text(std::mem::take(cur)) });
        }
    };
    // text up to the next ';' (the code's argument) and the index after it
    let arg = |from: usize| -> (String, usize) {
        match chars[from..].iter().position(|c| *c == ';') {
            Some(j) => (chars[from..from + j].iter().collect(), from + j + 1),
            None => (chars[from..].iter().collect(), chars.len()),
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if mtext && i + 1 < chars.len() => {
                let n = chars[i + 1];
                i += 2;
                match n {
                    'P' | 'p' => {
                        flush(&mut cur, &st, &mut runs);
                        paras.push(std::mem::take(&mut runs));
                    }
                    '\\' | '{' | '}' => cur.push(n),
                    '~' => cur.push(' '),
                    'L' | 'l' | 'O' | 'o' | 'K' | 'k' => {
                        flush(&mut cur, &st, &mut runs);
                        let on = n.is_ascii_uppercase();
                        match n.to_ascii_uppercase() {
                            'L' => st.underline = on,
                            'O' => st.overline = on,
                            _ => st.strike = on,
                        }
                    }
                    'F' | 'f' => {
                        let (a, next) = arg(i);
                        i = next;
                        flush(&mut cur, &st, &mut runs);
                        for flag in a.split('|').skip(1) {
                            match flag {
                                "b1" => st.bold = true,
                                "b0" => st.bold = false,
                                "i1" => st.italic = true,
                                "i0" => st.italic = false,
                                _ => {}
                            }
                        }
                    }
                    'H' | 'h' => {
                        let (a, next) = arg(i);
                        i = next;
                        flush(&mut cur, &st, &mut runs);
                        if let Some(rel) = a.strip_suffix('x') {
                            if let Ok(v) = rel.trim().parse::<f64>() {
                                if v > 0.0 {
                                    st.scale *= v;
                                }
                            }
                        } else if let Ok(v) = a.trim().parse::<f64>() {
                            if v > 0.0 && base_h > 0.0 {
                                st.scale = v / base_h;
                            }
                        }
                    }
                    'C' => {
                        let (a, next) = arg(i);
                        i = next;
                        flush(&mut cur, &st, &mut runs);
                        if let Ok(v) = a.trim().parse::<u32>() {
                            st.color = Some(if v == 0 { Color::ByBlock } else if v == 256 { Color::ByLayer } else { Color::Aci(v.min(255) as u8) });
                        }
                    }
                    'c' => {
                        let (a, next) = arg(i);
                        i = next;
                        flush(&mut cur, &st, &mut runs);
                        if let Ok(v) = a.trim().parse::<u32>() {
                            // the integer holds blue in the high byte: 0xBBGGRR
                            st.color = Some(Color::Rgb((v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8));
                        }
                    }
                    'W' | 'w' | 'T' | 't' | 'Q' | 'q' | 'A' | 'a' | 'X' | 'x' => {
                        let (_, next) = arg(i);
                        i = next;
                    }
                    'S' | 's' => {
                        let (body, next) = arg(i);
                        i = next;
                        flush(&mut cur, &st, &mut runs);
                        // the first unescaped separator splits numerator and denominator
                        let mut num = String::new();
                        let mut sep = None;
                        let bc: Vec<char> = body.chars().collect();
                        let mut k = 0;
                        while k < bc.len() {
                            match bc[k] {
                                '\\' if k + 1 < bc.len() => {
                                    num.push(bc[k + 1]);
                                    k += 2;
                                }
                                ch @ ('/' | '#' | '^') => {
                                    sep = Some(ch);
                                    k += 1;
                                    break;
                                }
                                ch => {
                                    num.push(ch);
                                    k += 1;
                                }
                            }
                        }
                        let den: String = bc[k..].iter().collect();
                        match sep {
                            Some(sep) => runs.push(Run { style: st.clone(), piece: Piece::Stack { num, den, bar: sep != '^' } }),
                            None => cur.push_str(&num),
                        }
                    }
                    'U' | 'u' if chars.get(i) == Some(&'+') => {
                        let hex: String = chars[i + 1..].iter().take(4).collect();
                        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            Some(ch) => {
                                cur.push(ch);
                                i += 5;
                            }
                            None => cur.push(n),
                        }
                    }
                    'M' | 'm' if chars.get(i) == Some(&'+') => {
                        // multi-byte code page character: not decoded
                        i = (i + 6).min(chars.len());
                    }
                    other => cur.push(other),
                }
                continue;
            }
            '{' if mtext => {
                flush(&mut cur, &st, &mut runs);
                groups.push(st.clone());
            }
            '}' if mtext => {
                flush(&mut cur, &st, &mut runs);
                if let Some(prev) = groups.pop() {
                    st = prev;
                }
            }
            '\n' => {
                flush(&mut cur, &st, &mut runs);
                paras.push(std::mem::take(&mut runs));
            }
            '%' if chars.get(i + 1) == Some(&'%') && i + 2 < chars.len() => {
                let code = chars[i + 2];
                match code {
                    'u' | 'U' | 'o' | 'O' => {
                        flush(&mut cur, &st, &mut runs);
                        if code.eq_ignore_ascii_case(&'u') {
                            st.underline = !st.underline;
                        } else {
                            st.overline = !st.overline;
                        }
                    }
                    'd' | 'D' => cur.push('°'),
                    'p' | 'P' => cur.push('±'),
                    'c' | 'C' => cur.push('Ø'),
                    '%' => cur.push('%'),
                    _ => {
                        cur.push('%');
                        i += 1; // keep "%%x" as typed when x is not a code
                        continue;
                    }
                }
                i += 3;
                continue;
            }
            _ => cur.push(c),
        }
        i += 1;
    }
    flush(&mut cur, &st, &mut runs);
    paras.push(runs);
    paras
}

/// The text without any formatting codes: paragraphs joined by `\n`, fractions as `a/b`.
pub fn plain(s: &str, mtext: bool) -> String {
    parse(s, 1.0, mtext)
        .into_iter()
        .map(|para| {
            para.into_iter()
                .map(|r| match r.piece {
                    Piece::Text(t) => t,
                    Piece::Stack { num, den, .. } => format!("{num}/{den}"),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(p: &[Vec<Run>]) -> Vec<Vec<String>> {
        p.iter().map(|para| para.iter().map(|r| if let Piece::Text(t) = &r.piece { t.clone() } else { "<stack>".into() }).collect()).collect()
    }

    #[test]
    fn plain_text_is_one_run() {
        let p = parse("Hello world", 2.5, true);
        assert_eq!(texts(&p), vec![vec!["Hello world".to_string()]]);
        assert_eq!(p[0][0].style, Style::default());
        assert!(!has_formatting("Hello world", true));
        assert!(has_formatting("a\\Pb", true) && !has_formatting("a\\Pb", false));
        assert!(has_formatting("10%%c", false));
    }

    #[test]
    fn paragraphs_groups_and_escapes() {
        let p = parse("one\\Ptwo {\\Lunder} done\\\\ \\{x\\}", 2.5, true);
        assert_eq!(p.len(), 2);
        assert_eq!(texts(&p)[0], vec!["one"]);
        assert_eq!(texts(&p)[1], vec!["two ", "under", " done\\ {x}"]);
        assert!(p[1][1].style.underline && !p[1][0].style.underline && !p[1][2].style.underline, "the group ends at its brace");
        assert_eq!(parse("a\nb", 1.0, false).len(), 2, "a plain newline is a paragraph break too");
    }

    #[test]
    fn font_flags_colour_and_height() {
        let p = parse("\\fArial|b1|i1|c0;bold italic{\\C1;red}\\C3;green\\H2x;big\\H5;abs", 2.5, true);
        let r = &p[0];
        assert!(r[0].style.bold && r[0].style.italic);
        assert_eq!(r[1].style.color, Some(Color::Aci(1)));
        assert!(r[1].style.bold, "a group inherits what is set outside");
        assert_eq!(r[2].style.color, Some(Color::Aci(3)), "after the group the colour set inside it is gone, the new one applies");
        assert!((r[3].style.scale - 2.0).abs() < 1e-12, "\\H2x; doubles");
        assert!((r[4].style.scale - 2.0).abs() < 1e-12, "\\H5; on 2.5 high text is 2x");
        let off = parse("\\fArial|b1;x\\fArial|b0;y", 1.0, true);
        assert!(off[0][0].style.bold && !off[0][1].style.bold);
        // true colour: blue in the high byte
        let rgb = parse("\\c255;red \\c16711680;blue", 1.0, true);
        assert_eq!(rgb[0][0].style.color, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(rgb[0][1].style.color, Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn line_decorations_toggle() {
        let p = parse("\\Lu\\lx\\Oo\\ok\\Kstrike\\k.", 1.0, true);
        let flags: Vec<(bool, bool, bool)> = p[0].iter().map(|r| (r.style.underline, r.style.overline, r.style.strike)).collect();
        assert_eq!(flags, vec![(true, false, false), (false, false, false), (false, true, false), (false, false, false), (false, false, true), (false, false, false)]);
    }

    #[test]
    fn stacked_fractions() {
        let p = parse("a \\S1/2; b \\S+0.1^-0.2; c \\S3#4;", 1.0, true);
        let stacks: Vec<(String, String, bool)> = p[0].iter().filter_map(|r| if let Piece::Stack { num, den, bar } = &r.piece { Some((num.clone(), den.clone(), *bar)) } else { None }).collect();
        assert_eq!(stacks, vec![("1".into(), "2".into(), true), ("+0.1".into(), "-0.2".into(), false), ("3".into(), "4".into(), true)]);
        assert_eq!(plain("a \\S1/2; b", true), "a 1/2 b");
    }

    #[test]
    fn percent_codes_work_in_every_text() {
        for mtext in [false, true] {
            assert_eq!(plain("45%%d  10%%c  %%p0.1  100%%%", mtext), "45°  10Ø  ±0.1  100%");
        }
        let p = parse("%%uunder%%u plain %%oover", 1.0, false);
        assert!(p[0][0].style.underline && !p[0][1].style.underline && p[0][2].style.overline);
        assert_eq!(plain("%%x stays", false), "%%x stays");
    }

    #[test]
    fn unknown_and_broken_codes_do_not_panic_or_lose_text() {
        for s in ["\\", "{", "}}", "\\f", "\\H;", "\\S", "\\Sno separator;", "\\U+12", "\\U+zzzz;", "\\C;x", "%", "%%", "tail\\", "\\M+1A2B3c", "\\q"] {
            let _ = parse(s, 2.5, true);
        }
        assert_eq!(plain("\\Sno separator;!", true), "no separator!");
        assert_eq!(plain("a\\U+00B0b", true), "a°b");
        assert_eq!(plain("\\W2;\\T3;\\Q15;\\A1;text", true), "text", "width, tracking, oblique and alignment codes are read and dropped");
    }
}
