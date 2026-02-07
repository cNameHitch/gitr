//! Diffcore transformation pipeline.
//!
//! Processes raw tree diff results through a series of transformations:
//! break, rename, merge-broken, pickaxe, order.

use bstr::BString;
use git_odb::ObjectDatabase;

use crate::rename::{detect_copies, detect_renames};
use crate::{DiffError, DiffOptions, DiffResult};

/// Run the diffcore pipeline on a raw diff result.
///
/// This applies the standard transformations in C git's order:
/// 1. diffcore-break (break complete rewrites)
/// 2. diffcore-rename (detect renames/copies)
/// 3. diffcore-merge-broken (re-merge unmatched broken pairs)
/// 4. diffcore-pickaxe (filter by string, if requested)
/// 5. diffcore-order (custom output ordering, if configured)
pub fn run_diffcore(
    odb: &ObjectDatabase,
    result: &mut DiffResult,
    options: &DiffOptions,
) -> Result<(), DiffError> {
    // Step 1: Break complete rewrites (not yet implemented — requires content comparison)
    // diffcore_break(result);

    // Step 2: Rename/copy detection
    if options.detect_renames {
        detect_renames(odb, result, options.rename_threshold)?;
    }

    if options.detect_copies {
        let all_files = result.files.clone();
        detect_copies(odb, result, options.copy_threshold, &all_files)?;
    }

    // Step 3: Re-merge broken pairs that weren't renamed
    // diffcore_merge_broken(result);

    // Step 4: Pickaxe filtering (not yet implemented)
    // Step 5: Custom ordering (not yet implemented)

    Ok(())
}

/// Filter diff results by pathspec.
pub fn filter_pathspec(result: &mut DiffResult, pathspecs: &[BString]) {
    if pathspecs.is_empty() {
        return;
    }
    result.files.retain(|f| {
        let path = f.path();
        pathspecs
            .iter()
            .any(|spec| path.starts_with(spec.as_slice()))
    });
}
