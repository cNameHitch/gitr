//! Diff output formatters.

pub mod combined;
pub mod nameonly;
pub mod raw;
pub mod stat;
pub mod unified;
pub mod word_diff;

use crate::{DiffOptions, DiffOutputFormat, DiffResult};

/// Format a DiffResult according to the specified output format.
pub fn format_diff(result: &DiffResult, options: &DiffOptions) -> String {
    match options.output_format {
        DiffOutputFormat::Unified => unified::format(result, options),
        DiffOutputFormat::Stat => stat::format_stat(result, options),
        DiffOutputFormat::ShortStat => stat::format_short_stat(result),
        DiffOutputFormat::NumStat => stat::format_numstat(result),
        DiffOutputFormat::Raw => raw::format(result),
        DiffOutputFormat::NameOnly => nameonly::format_name_only(result),
        DiffOutputFormat::NameStatus => nameonly::format_name_status(result),
        DiffOutputFormat::Summary => nameonly::format_summary(result),
        DiffOutputFormat::WordDiff => word_diff::format_word_diff(result),
    }
}
