use crucible_utils::{define_index, mem::StrSplicer, newtypes::Index as _};

// === Core === //

pub const MANGLE_SEP: &str = "_MANGLE_WGSL_LINK_";

define_index! {
    pub struct MangleIndex: u64;
}

pub fn has_stray_mangles(file: &str) -> bool {
    memchr::memmem::find(file.as_bytes(), MANGLE_SEP.as_bytes()).is_some()
}

pub fn mangle_mut(name: &mut String, idx: MangleIndex) {
    use std::fmt::Write as _;

    assert!(!has_stray_mangles(name));
    write!(name, "{}{}_", MANGLE_SEP, idx.0).unwrap();
}

pub fn try_demangle(name: &str) -> Option<(&str, MangleIndex)> {
    let idx = memchr::memmem::rfind(name.as_bytes(), MANGLE_SEP.as_bytes())?;
    let left = &name[..idx];
    let right = &name[idx..][MANGLE_SEP.len()..];
    let right = &right[..(right.len() - 1)];
    let right = MangleIndex::from_usize(right.parse().unwrap());

    Some((left, right))
}

// === Replace Mangles === //

pub struct MangleReplaceOut<'a, 'b> {
    did_replace: &'a mut bool,
    splicer: &'a mut StrSplicer<'b>,
    end_pos: usize,
}

impl MangleReplaceOut<'_, '_> {
    pub fn replace(self, mangled_name: &str, new_name: &str) {
        assert_eq!(
            &self.splicer.remaining()[(self.end_pos - mangled_name.len())..self.end_pos],
            mangled_name.as_bytes(),
        );
        self.replace_known_len(mangled_name.len(), new_name);
    }

    pub fn replace_known_len(self, mangled_name_len: usize, new_name: &str) {
        *self.did_replace = true;
        self.splicer.splice(
            self.end_pos - mangled_name_len,
            mangled_name_len,
            new_name.as_bytes(),
        );
    }
}

pub fn replace_mangles(
    target: &mut String,
    mut replace: impl FnMut(MangleIndex, MangleReplaceOut<'_, '_>),
) {
    let mut splicer = StrSplicer::new(target);

    while let Some(pos) = memchr::memmem::find(splicer.remaining(), MANGLE_SEP.as_bytes()) {
        // Parse mangle index
        let pos = pos + MANGLE_SEP.len();
        let end_pos = pos + memchr::memchr(b'_', &splicer.remaining()[pos..]).unwrap();
        let idx = std::str::from_utf8(&splicer.remaining()[pos..end_pos])
            .unwrap()
            .parse()
            .unwrap();
        let idx = MangleIndex::from_usize(idx);
        let end_pos = end_pos + 1;

        // Allow user to replace string
        let mut did_replace = false;
        replace(
            idx,
            MangleReplaceOut {
                did_replace: &mut did_replace,
                splicer: &mut splicer,
                end_pos,
            },
        );

        if !did_replace {
            splicer.splice(end_pos, 0, &[]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mangle_replace_works() {
        let mut target =
            "fn WHEE_MANGLE_WGSL_LINK_0_(a: u32, b: i32); export WHEE_MANGLE_WGSL_LINK_0_;"
                .to_string();

        replace_mangles(&mut target, |var, rep| {
            assert_eq!(var, MangleIndex::from_usize(0));
            rep.replace("WHEE_MANGLE_WGSL_LINK_0_", "my_new_name");
        });

        assert_eq!(
            target.as_str(),
            "fn my_new_name(a: u32, b: i32); export my_new_name;"
        );
    }
}
