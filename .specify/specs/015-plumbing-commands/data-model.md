# Data Model: Plumbing Commands

Plumbing commands are thin wrappers around library APIs. No new domain types are defined. Each command maps directly to library operations:

## Command → Library API Mapping

| Command | Primary Library API |
|---------|-------------------|
| cat-file | `ObjectDatabase::read()`, `read_header()` |
| hash-object | `ObjectDatabase::write_raw()`, `Hasher::hash_object()` |
| rev-parse | `Repository::discover()`, object name resolution |
| update-ref | `RefStore::transaction()`, `RefTransaction::commit()` |
| for-each-ref | `RefStore::iter()`, format string evaluation |
| show-ref | `RefStore::iter()` |
| symbolic-ref | `RefStore::resolve()`, `RefTransaction::set_symbolic()` |
| ls-files | `Index::iter()`, `Index::iter_matching()` |
| ls-tree | `ObjectDatabase::read()` tree, iterate entries |
| update-index | `Index::add()`, `Index::remove()`, `Index::write_to()` |
| check-ignore | `IgnoreStack::is_ignored()` |
| check-attr | `AttributeStack::get_attrs()` |
| mktree | `Tree::new()`, `ObjectDatabase::write()` |
| mktag | `Tag::parse()` (validate), `ObjectDatabase::write()` |
| commit-tree | `Commit::new()`, `ObjectDatabase::write()` |
| verify-pack | `PackFile::verify_checksum()`, iterate all entries |
| check-ref-format | `RefName::new()` (validates) |
| var | Read environment and config values |
| write-tree | `Index::write_tree()` |

## CLI Framework

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gitr")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Change directory (-C)
    #[arg(short = 'C', global = true)]
    directory: Option<PathBuf>,
    /// Set config (-c key=value)
    #[arg(short = 'c', global = true)]
    config: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    CatFile(CatFileArgs),
    HashObject(HashObjectArgs),
    RevParse(RevParseArgs),
    UpdateRef(UpdateRefArgs),
    ForEachRef(ForEachRefArgs),
    // ... etc
}
```
