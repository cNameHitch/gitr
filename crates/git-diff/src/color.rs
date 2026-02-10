//! Colored diff output formatting.
//!
//! Wraps diff lines in ANSI escape sequences using ColorConfig from git-utils.

use git_utils::color::{ColorConfig, ColorSlot};

use crate::{DiffLine, Hunk};

/// Format a diff line with color.
pub fn colorize_diff_line(line: &DiffLine, config: &ColorConfig, enabled: bool) -> String {
    if !enabled {
        return match line {
            DiffLine::Context(s) => format!(" {}", s),
            DiffLine::Addition(s) => format!("+{}", s),
            DiffLine::Deletion(s) => format!("-{}", s),
        };
    }

    let reset = config.get_color(ColorSlot::Reset);
    match line {
        DiffLine::Context(s) => format!(" {}", s),
        DiffLine::Addition(s) => {
            format!("{}+{}{}", config.get_color(ColorSlot::DiffNewNormal), s, reset)
        }
        DiffLine::Deletion(s) => {
            format!("{}-{}{}", config.get_color(ColorSlot::DiffOldNormal), s, reset)
        }
    }
}

/// Format a hunk header (@@ -a,b +c,d @@) with color.
pub fn colorize_hunk_header(hunk: &Hunk, config: &ColorConfig, enabled: bool) -> String {
    let header_text = if let Some(ref func) = hunk.header {
        format!(
            "@@ -{},{} +{},{} @@ {}",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count, func
        )
    } else {
        format!(
            "@@ -{},{} +{},{} @@",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
        )
    };

    if !enabled {
        return header_text;
    }

    let frag_color = config.get_color(ColorSlot::DiffFragInfo);
    let reset = config.get_color(ColorSlot::Reset);
    format!("{}{}{}", frag_color, header_text, reset)
}

/// Format a diff file header (diff --git a/... b/...) with color.
pub fn colorize_diff_header(header: &str, config: &ColorConfig, enabled: bool) -> String {
    if !enabled {
        return header.to_string();
    }
    let meta_color = config.get_color(ColorSlot::DiffMetaInfo);
    let reset = config.get_color(ColorSlot::Reset);
    format!("{}{}{}", meta_color, header, reset)
}
