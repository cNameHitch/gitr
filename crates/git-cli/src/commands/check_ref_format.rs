use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_ref::RefName;

#[derive(Args)]
pub struct CheckRefFormatArgs {
    /// Allow single-level ref names (e.g., just "main" instead of "refs/heads/main")
    #[arg(long)]
    allow_onelevel: bool,

    /// Normalize the refname (print it if valid)
    #[arg(long)]
    normalize: bool,

    /// Reference name(s) to check
    #[arg(required = true)]
    refname: Vec<String>,
}

pub fn run(args: &CheckRefFormatArgs) -> Result<i32> {
    for refname_str in &args.refname {
        match RefName::new(BString::from(refname_str.as_str())) {
            Ok(refname) => {
                if args.normalize {
                    println!("{}", refname.as_str());
                }
                // Valid — continue
            }
            Err(_) => {
                // Invalid ref name
                return Ok(1);
            }
        }
    }

    Ok(0)
}
