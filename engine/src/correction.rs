use anyhow::Error;
use once_cell::sync::OnceCell;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use symspell::{AsciiStringStrategy, SymSpell};


static DATA_DIR: &str = "datasets/dictionaries";
static SYMSPELL: OnceCell<SymSpell<AsciiStringStrategy>> = OnceCell::new();
static ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn enable_load_dictionaries() -> anyhow::Result<()> {
    ENABLED.store(true, Ordering::Relaxed);

    let mut symspell: SymSpell<AsciiStringStrategy> = SymSpell::default();

    for entry in fs::read_dir(DATA_DIR)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            return Err(Error::msg("directories are not expected"));
        }

        symspell.load_dictionary(path.as_os_str().to_str().unwrap(), 0, 1, " ");
    }

    let _ = SYMSPELL.set(symspell);

    Ok(())
}

pub(crate) fn correct_sentence(query: &str) -> String {
    let sym = SYMSPELL.get().expect("get symspell");

    let mut suggestions = sym.lookup_compound(query, 1);

    if suggestions.len() == 0 {
        return query.into();
    }

    return suggestions.remove(0).term;
}
