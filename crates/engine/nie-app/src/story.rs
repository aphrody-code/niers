//! Logique du mode HISTOIRE (unification Phase 5). Le chargement de scène (SQLite) vit dans le
//! front-end (qui possède la connexion DB) ; ici la transformation PURE et testable des dialogues.

/// Nettoie un dialogue IEVR pour l'affichage : `<FLC:NAME>`/`<FUL:NAME>` → `Name` (Title-case),
/// retire les autres balises `<…>`, remplace les `\n` littéraux par une espace, compacte les blancs.
#[must_use]
pub fn clean_dialogue(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<'
            && let Some(end) = s[i..].find('>')
        {
            let tag = &s[i + 1..i + end];
            if let Some((_, name)) = tag.split_once(':') {
                let mut cs = name.chars();
                if let Some(f) = cs.next() {
                    out.push(f);
                    out.extend(cs.flat_map(char::to_lowercase));
                }
            }
            i += end + 1;
            continue;
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'n' {
            out.push(' ');
            i += 2;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::clean_dialogue;

    #[test]
    fn resout_les_tags_perso() {
        assert_eq!(
            clean_dialogue("You good, <FLC:KISOJI>?"),
            "You good, Kisoji?"
        );
        assert_eq!(clean_dialogue("<FUL:SAKURAZAKI>..."), "Sakurazaki...");
    }

    #[test]
    fn remplace_newline_et_compacte() {
        assert_eq!(
            clean_dialogue("But I wasn't having fun myself,\\nso it didn't come across..."),
            "But I wasn't having fun myself, so it didn't come across..."
        );
    }

    #[test]
    fn texte_sans_balise_inchange() {
        assert_eq!(clean_dialogue("Hm?"), "Hm?");
        assert_eq!(
            clean_dialogue("Can anyone bring down Raimon's unshakable fortress?!"),
            "Can anyone bring down Raimon's unshakable fortress?!"
        );
    }
}
