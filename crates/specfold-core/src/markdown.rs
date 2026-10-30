//! Reading and writing the Markdown files documents are made of.

pub mod body;
pub mod link_dest;

/// C135: where fenced code starts and ends, the CommonMark way: a fence opens with three or
/// more backticks or tildes and closes only with the same character, at least as many, and
/// nothing else on the line.
#[derive(Debug, Clone, Default)]
pub struct Fence(Option<(char, usize)>);

impl Fence {
    /// Feeds one line; true when it opens or closes a fence.
    pub fn step(&mut self, line: &str) -> bool {
        let t = line.trim_start();
        let run = |ch: char| t.chars().take_while(|c| *c == ch).count();
        match self.0 {
            Some((ch, n)) => {
                let m = run(ch);
                if m >= n && t[m * ch.len_utf8()..].trim().is_empty() {
                    self.0 = None;
                    return true;
                }
                false
            }
            None => {
                for ch in ['`', '~'] {
                    let n = run(ch);
                    // A backtick fence's info string cannot hold a backtick.
                    if n >= 3 && !(ch == '`' && t[n..].contains('`')) {
                        self.0 = Some((ch, n));
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Whether the lines fed so far leave a fence open.
    pub fn inside(&self) -> bool {
        self.0.is_some()
    }
}

pub mod counts;
pub mod frontmatter;
pub mod sections;
pub mod tables;
pub mod tasks;
