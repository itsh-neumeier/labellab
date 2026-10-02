//! Inline styles inside one text element: `**bold**` and `__italic__`
//! (may be combined and span line breaks). A marker without a partner is
//! kept as literal text, so a lone `**` in a label still prints.

/// Marker that toggles bold.
pub const BOLD_MARK: &str = "**";
/// Marker that toggles italic.
pub const ITALIC_MARK: &str = "__";

/// A piece of text in one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

/// Whether `text` contains any style marker.
pub fn has_markup(text: &str) -> bool {
    text.contains(BOLD_MARK) || text.contains(ITALIC_MARK)
}

/// Splits `text` into runs. `bold`/`italic` are the element's base style;
/// markers switch the respective style on and off again.
pub fn parse(text: &str, bold: bool, italic: bool) -> Vec<Run> {
    let bold_marks = paired_marks(text, BOLD_MARK);
    let italic_marks = paired_marks(text, ITALIC_MARK);
    let mut runs: Vec<Run> = Vec::new();
    let (mut b, mut i) = (false, false);
    let mut current = String::new();
    let mut pos = 0;
    while pos < text.len() {
        let mark = if bold_marks.contains(&pos) {
            Some((BOLD_MARK, true))
        } else if italic_marks.contains(&pos) {
            Some((ITALIC_MARK, false))
        } else {
            None
        };
        if let Some((m, is_bold)) = mark {
            push(&mut runs, &mut current, bold || b, italic || i);
            if is_bold {
                b = !b;
            } else {
                i = !i;
            }
            pos += m.len();
            continue;
        }
        let Some(c) = text[pos..].chars().next() else {
            break;
        };
        current.push(c);
        pos += c.len_utf8();
    }
    push(&mut runs, &mut current, bold || b, italic || i);
    runs
}

/// The text without style markers.
pub fn plain(text: &str) -> String {
    parse(text, false, false)
        .into_iter()
        .map(|r| r.text)
        .collect()
}

fn push(runs: &mut Vec<Run>, current: &mut String, bold: bool, italic: bool) {
    if current.is_empty() {
        return;
    }
    let text = std::mem::take(current);
    match runs.last_mut() {
        Some(last) if last.bold == bold && last.italic == italic => last.text.push_str(&text),
        _ => runs.push(Run { text, bold, italic }),
    }
}

/// Byte positions of `mark` occurrences that form pairs (a trailing odd
/// one is literal text). `***` counts as `**` followed by `*`.
fn paired_marks(text: &str, mark: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut pos = 0;
    while let Some(i) = text[pos..].find(mark) {
        found.push(pos + i);
        pos += i + mark.len();
    }
    if found.len() % 2 == 1 {
        found.pop();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, bold: bool, italic: bool) -> Run {
        Run {
            text: text.into(),
            bold,
            italic,
        }
    }

    #[test]
    fn plain_text_is_one_run() {
        assert_eq!(
            parse("Rack 1", false, false),
            vec![run("Rack 1", false, false)]
        );
        assert!(!has_markup("Rack 1"));
    }

    #[test]
    fn markers_switch_styles() {
        assert_eq!(
            parse("Server **42**\n__kalt__ **__beides__**", false, false),
            vec![
                run("Server ", false, false),
                run("42", true, false),
                run("\n", false, false),
                run("kalt", false, true),
                run(" ", false, false),
                run("beides", true, true),
            ]
        );
        assert_eq!(plain("a **b** __c__"), "a b c");
    }

    #[test]
    fn base_style_applies_everywhere() {
        assert_eq!(parse("A **B**", true, false), vec![run("A B", true, false)]);
    }

    #[test]
    fn unpaired_marker_stays_literal() {
        assert_eq!(
            parse("5** Hotel", false, false),
            vec![run("5** Hotel", false, false)]
        );
        assert_eq!(
            parse("**a** b**", false, false),
            vec![run("a", true, false), run(" b**", false, false)]
        );
        assert_eq!(plain("Größe **ä**"), "Größe ä");
    }
}
