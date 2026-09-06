//! Map intrusive du moteur Level-5 (`lives::`) — **branche linéaire** de `FUN_1402e2a10`
//! (`@0x1402e2a10`, `nie_eacpatched.exe`), la primitive de lookup amont des écrans à liste
//! (`CMenuListView` : shop / inventaire / roster / formation…).
//!
//! ## Disposition mémoire (reversée + validée uemu)
//!
//! La structure est deux tableaux parallèles dans un même buffer de données, indexés par le
//! **même** index de nœud `u16` :
//! - **entrées** à `base + 8`, `0x10` octets chacune, **clé `i32` en `+0`** ;
//! - **nœuds** à `base + nodes_off`, `6` octets chacun, **`next: u16` en `+2`** (chaînage).
//!
//! Les champs de l'en-tête de map (offsets dans la struct `param_1` de nie.exe) :
//! `+0x08` = pointeur `base` ; `+0x10` = index de tête ; `+0x1c` = `nodes_off` ; `+0x20` = 0
//! pour le **mode linéaire** (≠ 0 = mode haché, hors de ce port) ; `+0x24` = capacité/sentinelle
//! (un `next ≥ cap` termine la chaîne).
//!
//! Le `find` du binaire renvoie l'**adresse du nœud** trouvé (`base + nodes_off + idx*6`) ou `0` ;
//! ce port renvoie l'**index** de nœud équivalent (`Option<u16>`), invariant indépendant de
//! l'adresse de base.
//!
//! ## Validation
//!
//! Oracle **uemu byte-exact** (`scripts/uemu.py` → `validate_intrusive_map.py`) : émulation de
//! `FUN_1402e2a10` sur une map synthétique (mode linéaire), comparaison de l'index de nœud
//! retourné. Les tests ci-dessous reproduisent les sorties de l'oracle.
//!
//! ## Deux instanciations réelles, un seul algorithme (généralisé par `entry_stride`)
//!
//! Le moteur réutilise ce conteneur comme un *template* avec des tailles/dispositions d'entrée
//! différentes. **Trois** fonctions du binaire sont **byte-exact** contre ce port, à la disposition
//! d'entrée près :
//! - `FUN_1402e2a10` / `FUN_1402b4160` — entrées **0x10 o**, clé @ entrée+**8** (premier portage) ;
//! - `FUN_14050b0b0` — entrées **0xc o**, clé @ entrée+**8** ;
//! - `FUN_1401f5ab0` — entrées **0x18 o**, clé @ entrée+**0**.
//!
//! Header, nœuds (6 o, `next` u16 @+2) et tableau trié (u16) sont **identiques** dans les trois.
//! D'où deux paramètres : la clé `i32` d'une entrée d'index `i` est à `buf + key_base + i*entry_stride`.

extern crate alloc;

/// Vue en lecture d'une map intrusive en **mode linéaire** (`header[+0x20] == 0`).
#[derive(Debug, Clone, Copy)]
pub struct IntrusiveMapLinear<'a> {
    /// Buffer de données (= `*(base)` = champ `+0x08` du binaire) : entrées à `+8`, nœuds à `nodes_off`.
    pub buf: &'a [u8],
    /// Index du nœud de tête (`header[+0x10]`).
    pub head: u16,
    /// Offset du tableau de nœuds dans `buf` (`header[+0x1c]`).
    pub nodes_off: u32,
    /// Capacité/sentinelle (`header[+0x24]`) : `head ≥ cap` ou `next ≥ cap` termine la chaîne.
    pub cap: u16,
    /// Largeur d'une entrée en octets : `0x10` / `0xc` / `0x18` selon l'instanciation. Module-doc.
    pub entry_stride: u32,
    /// Offset de la clé `i32` dans l'entrée : `8` (familles 0xc/0x10) ou `0` (famille 0x18). Module-doc.
    pub key_base: u32,
}

#[inline]
fn read_i32(buf: &[u8], off: usize) -> Option<i32> {
    let b = buf.get(off..off + 4)?;
    Some(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

#[inline]
fn read_u16(buf: &[u8], off: usize) -> Option<u16> {
    let b = buf.get(off..off + 2)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

impl IntrusiveMapLinear<'_> {
    /// Index du nœud dont l'entrée porte `key`, en suivant la chaîne depuis `head` ; `None` si
    /// absent (ou tête ≥ capacité). Port byte-exact de la branche `*(int*)(param_1+0x20)==0` de
    /// `FUN_1402e2a10` : l'entrée du nœud courant est comparée (do-while), puis on suit `next`
    /// tant que `next < cap`.
    #[must_use]
    pub fn find(&self, key: i32) -> Option<u16> {
        if self.head >= self.cap {
            return None; // `if (head < cap && …)` du binaire
        }
        let mut idx = self.head;
        loop {
            // Entrée `idx` : clé i32 à `base + key_base + idx*stride`.
            let ekey = read_i32(
                self.buf,
                self.key_base as usize + idx as usize * self.entry_stride as usize,
            )?;
            if ekey == key {
                return Some(idx);
            }
            // Nœud `idx` : `next` u16 à `base + nodes_off + idx*6 + 2`.
            let next = read_u16(self.buf, self.nodes_off as usize + idx as usize * 6 + 2)?;
            if next >= self.cap {
                return None; // fin de chaîne (`while (node.next < cap)`)
            }
            idx = next;
        }
    }
}

// ── Mode TRIÉ (`header[+0x20] != 0`) : recherche binaire (FUN_1402b4160) ───────

/// Vue en lecture d'une map intrusive en **mode trié** (`header[+0x20] != 0`).
///
/// Le tableau d'index `sorted` (`u16`, `count` entrées) à `buf + sorted_off` ordonne les entrées
/// par clé croissante : `key(i) = entries[sorted[i]].key` (clé `u32` à `buf + 8 + sorted[i]*0x10`).
/// Le lookup est une **recherche binaire** sur ces positions triées.
#[derive(Debug, Clone, Copy)]
pub struct IntrusiveMapSorted<'a> {
    /// Buffer de données (champ `+0x08` du binaire).
    pub buf: &'a [u8],
    /// Nombre d'entrées triées (`header[+0x14]`).
    pub count: u16,
    /// Offset du tableau d'index trié dans `buf` (`header[+0x20]`).
    pub sorted_off: u32,
    /// Largeur d'une entrée en octets : `0x10` / `0xc` / `0x18` selon l'instanciation. Module-doc.
    pub entry_stride: u32,
    /// Offset de la clé `i32` dans l'entrée : `8` (familles 0xc/0x10) ou `0` (famille 0x18). Module-doc.
    pub key_base: u32,
}

impl IntrusiveMapSorted<'_> {
    /// Clé `u32` à la **position triée** `i` : `entries[sorted[i]].key`. `None` si hors limites.
    #[inline]
    fn key_at(&self, i: u16) -> Option<u32> {
        let so = self.sorted_off as usize + i as usize * 2;
        let entry_idx = read_u16(self.buf, so)? as usize;
        read_i32(
            self.buf,
            self.key_base as usize + entry_idx * self.entry_stride as usize,
        )
        .map(|k| k as u32)
    }

    /// Index d'entrée (`sorted[i]`) à la position triée `i`. `None` si hors limites.
    #[inline]
    fn entry_index_at(&self, i: u16) -> Option<u16> {
        read_u16(self.buf, self.sorted_off as usize + i as usize * 2)
    }

    /// Cœur byte-exact de `FUN_1402b4160` : recherche binaire de `target` (comparaisons **u32 non
    /// signées**). Renvoie `(pos, out)` : `pos` = valeur de retour (`0xffff` = absent) ; `out` =
    /// `Some(insertion)` uniquement quand le binaire écrit le param de sortie (chemins « absent »),
    /// `None` sinon. `with_out` = `param_3 != null` (change la valeur de retour sur doublons : sans
    /// out → position **leftmost-equal** ; avec out → première position trouvée).
    fn search(&self, target: u32, with_out: bool) -> (u16, Option<u16>) {
        let n = self.count;
        if n == 0 {
            return (0xffff, None);
        }
        let key = |i: u16| self.key_at(i);
        let mut u6: u16 = 0;
        if key(0).is_some_and(|k| k < target) {
            let last = n - 1;
            let klast = key(last);
            if klast.is_some_and(|k| k <= target) {
                if klast.is_some_and(|k| k < target) {
                    return (0xffff, if with_out { Some(last) } else { None }); // target > tout
                }
                // key(n-1) == target
                if with_out {
                    return (last, None);
                }
                if n >= 2 && key(n - 2) == Some(target) {
                    let mut u5 = last;
                    loop {
                        let u3 = u5 - 1;
                        u5 = u3;
                        if key(u3.wrapping_sub(1)) != Some(target) {
                            break;
                        }
                    }
                    return (u5, None);
                }
                return (last, None);
            }
            // key(n-1) > target : recherche binaire dans l'intérieur [1, n-2].
            if n > 2 {
                let mut lo: u16 = 1;
                let mut hi: u16 = n - 2;
                if hi != 0 {
                    loop {
                        let mid = ((hi - lo) >> 1) + lo;
                        u6 = mid;
                        let km = key(mid);
                        if km == Some(target) {
                            if with_out {
                                return (mid, None);
                            }
                            if mid >= 1 && key(mid - 1) == Some(target) {
                                let mut u5 = mid;
                                loop {
                                    let u3 = u5 - 1;
                                    u5 = u3;
                                    if key(u5.wrapping_sub(1)) != Some(target) {
                                        break;
                                    }
                                }
                                return (u5, None);
                            }
                            return (mid, None);
                        }
                        if km.is_some_and(|k| target < k) {
                            if mid == 0 {
                                break;
                            }
                            hi = mid - 1;
                        } else {
                            lo = mid + 1;
                        }
                        if lo > hi {
                            break;
                        }
                    }
                }
            }
            return (0xffff, if with_out { Some(u6) } else { None });
        } else if key(0).is_some_and(|k| k <= target) {
            return (0, None); // key(0) == target
        }
        (0xffff, if with_out { Some(0) } else { None }) // key(0) > target
    }

    /// Position **triée** (leftmost-equal) de `target`, ou `None` si absent (chemin `param_3==null`
    /// de `FUN_1402b4160`, celui qu'utilise le find de map `FUN_1402e2a10`).
    #[must_use]
    pub fn find_position(&self, target: u32) -> Option<u16> {
        match self.search(target, false).0 {
            0xffff => None,
            pos => Some(pos),
        }
    }

    /// Index d'**entrée** (`sorted[pos]`) de `target`, ou `None` si absent — complète le find de
    /// map en mode trié (position triée → index d'entrée).
    #[must_use]
    pub fn find_entry(&self, target: u32) -> Option<u16> {
        self.find_position(target)
            .and_then(|pos| self.entry_index_at(pos))
    }

    /// Borne inférieure (chemin `param_3 != null`) : `Ok(position)` si `target` présent, sinon
    /// `Err(position d'insertion)`.
    pub fn lower_bound(&self, target: u32) -> Result<u16, u16> {
        let (pos, out) = self.search(target, true);
        if pos == 0xffff {
            Err(out.unwrap_or(0))
        } else {
            Ok(pos)
        }
    }

    /// Itérateur « entrée suivante de **même clé** » — port byte-exact de `FUN_140541de0`.
    ///
    /// Étant donné la **position triée** `pos` d'une entrée, renvoie l'**index d'entrée**
    /// (`sorted[pos+1]`) de l'entrée à la position triée suivante **si** sa clé est égale à celle
    /// de `pos` (parcours d'un run de doublons → énumération multimap, ex. lignes d'une liste de
    /// menu partageant la même catégorie). Renvoie `None` quand :
    /// - `pos` est la dernière position (`pos >= count - 1`, comparaison **signée** comme le binaire :
    ///   `count == 0` ⇒ `count-1 == -1` ⇒ toujours `None`) ;
    /// - l'index d'entrée suivant atteint la sentinelle (`sorted[pos+1] >= cap`) ;
    /// - la clé suivante diffère (fin du run).
    ///
    /// `cap` = `header[+0x24]` (sentinelle de capacité). Dans le binaire, le nœud courant est passé
    /// par pointeur et sa position triée est lue en `node[+4]` ; ici on prend directement la
    /// position (équivalent pour une map bien formée — domaine réel : `node[+4]` = permutation
    /// inverse). Pour énumérer tout le run, ré-appeler avec `pos + 1` tant que `Some`.
    #[must_use]
    pub fn next_equal(&self, pos: u16, cap: u16) -> Option<u16> {
        // `(int)(uint)pos < (int)(count - 1)` : comparaison signée (count u16 promu int).
        if (i32::from(pos)) >= i32::from(self.count) - 1 {
            return None;
        }
        let next_entry = self.entry_index_at(pos + 1)?; // sorted[pos+1]
        if next_entry >= cap {
            return None;
        }
        // Le binaire compare `key[sorted[pos+1]] == key[entrée courante]` ; pour une map bien formée
        // l'entrée courante = `sorted[pos]`, donc `key(pos+1) == key(pos)`.
        if self.key_at(pos + 1)? == self.key_at(pos)? {
            Some(next_entry)
        } else {
            None
        }
    }
}

// ── Côté ÉCRITURE : liste active doublement chaînée (pop_front, FUN_140453570) ──

/// Vue mutable sur la **liste active doublement chaînée** du conteneur, portée sur les MÊMES nœuds
/// 6 o que les find : `prev: u16 @+0`, `next: u16 @+2`. L'en-tête du binaire stocke ce curseur en
/// `+0x16` (tête), `+0x18` (queue), `+0x1a` (compteur). C'est le côté **écriture** (les `find`
/// `IntrusiveMap{Linear,Sorted}` étant le côté lecture).
#[derive(Debug, Clone, Copy)]
pub struct IntrusiveListCursor {
    /// Index du nœud de tête (`header[+0x16]`) ; `>= cap` (p.ex. `0xffff`) = liste vide.
    pub head: u16,
    /// Index du nœud de queue (`header[+0x18]`).
    pub tail: u16,
    /// Nombre d'éléments (`header[+0x1a]`).
    pub count: u16,
    /// Offset du tableau de nœuds dans `buf` (`header[+0x1c]`).
    pub nodes_off: u32,
    /// Capacité/sentinelle (`header[+0x24]`).
    pub cap: u16,
}

impl IntrusiveListCursor {
    /// Détache et renvoie l'index du nœud de **tête** — port byte-exact de `FUN_140453570`.
    ///
    /// Avance la tête vers `head.next`, met `new_head.prev = 0xffff`, décrémente `count`. Si la tête
    /// est aussi la queue (dernier élément), vide la liste (`head = tail = 0xffff`). Renvoie `None`
    /// (le binaire renvoie 0, sans aucune mutation) si la tête `>= cap` (liste vide). Mute `self`
    /// ainsi que le champ `prev` du nouveau nœud de tête dans `buf`.
    ///
    /// Garde-fou : si `head.next >= cap` sur une liste non vide (entrée malformée), renvoie `None`
    /// au lieu de reproduire le déréférencement de pointeur nul du binaire (UB).
    pub fn pop_front(&mut self, buf: &mut [u8]) -> Option<u16> {
        if self.head >= self.cap {
            return None;
        }
        let popped = self.head;
        if self.tail == self.head {
            self.head = 0xffff;
            self.tail = 0xffff;
            self.count = self.count.wrapping_sub(1);
            return Some(popped);
        }
        // `next` du nœud courant (champ +2).
        let next = read_u16(buf, self.nodes_off as usize + popped as usize * 6 + 2)?;
        if next >= self.cap {
            return None; // garde-fou anti-UB (cf. doc)
        }
        // `new_head.prev` (champ +0) = sentinelle.
        let p = self.nodes_off as usize + next as usize * 6;
        buf.get_mut(p..p + 2)?
            .copy_from_slice(&0xffff_u16.to_le_bytes());
        self.head = next;
        self.count = self.count.wrapping_sub(1);
        Some(popped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Construit le buffer de données de l'oracle : entrées de `stride` o (clé i32 @+0) à `+8`,
    /// nœuds 6 o (`next` u16 @+2) à `nodes_off`.
    fn build(entries: &[i32], nexts: &[u16], nodes_off: usize, stride: usize) -> Vec<u8> {
        let mut buf = vec![0u8; 0x400];
        for (i, &k) in entries.iter().enumerate() {
            buf[8 + i * stride..8 + i * stride + 4].copy_from_slice(&k.to_le_bytes());
        }
        for (i, &nx) in nexts.iter().enumerate() {
            buf[nodes_off + i * 6 + 2..nodes_off + i * 6 + 4].copy_from_slice(&nx.to_le_bytes());
        }
        buf
    }

    /// Reproduit l'oracle uemu : chaîne node0(100)→node1(200)→node2(300)→stop.
    #[test]
    fn matches_uemu_oracle_chain() {
        let buf = build(&[100, 200, 300], &[1, 2, 0xFFFF], 0x200, 0x10);
        let m = IntrusiveMapLinear {
            buf: &buf,
            head: 0,
            nodes_off: 0x200,
            cap: 0x100,
            entry_stride: 0x10,
            key_base: 8,
        };
        assert_eq!(m.find(100), Some(0), "node0");
        assert_eq!(m.find(200), Some(1), "node1");
        assert_eq!(m.find(300), Some(2), "node2");
        assert_eq!(m.find(999), None, "absent");
    }

    /// Tête au milieu de la chaîne : on ne voit que la queue.
    #[test]
    fn head_mid_chain() {
        let buf = build(&[100, 200, 300], &[1, 2, 0xFFFF], 0x200, 0x10);
        let m = IntrusiveMapLinear {
            buf: &buf,
            head: 1,
            nodes_off: 0x200,
            cap: 0x100,
            entry_stride: 0x10,
            key_base: 8,
        };
        assert_eq!(
            m.find(100),
            None,
            "100 hors de la sous-chaîne depuis la tête 1"
        );
        assert_eq!(m.find(200), Some(1));
        assert_eq!(m.find(300), Some(2));
    }

    /// Tête ≥ capacité → map vide (branche d'entrée non prise).
    #[test]
    fn head_at_or_past_cap_is_empty() {
        let buf = build(&[100], &[0xFFFF], 0x200, 0x10);
        let m = IntrusiveMapLinear {
            buf: &buf,
            head: 0x100,
            nodes_off: 0x200,
            cap: 0x100,
            entry_stride: 0x10,
            key_base: 8,
        };
        assert_eq!(m.find(100), None);
    }

    /// Élément unique (next ≥ cap dès le 1er nœud).
    #[test]
    fn single_element() {
        let buf = build(&[42], &[0x100], 0x200, 0x10);
        let m = IntrusiveMapLinear {
            buf: &buf,
            head: 0,
            nodes_off: 0x200,
            cap: 0x100,
            entry_stride: 0x10,
            key_base: 8,
        };
        assert_eq!(m.find(42), Some(0));
        assert_eq!(m.find(0), None);
    }

    // ── Mode trié (FUN_1402b4160) ────────────────────────────────────────────

    /// Construit un buffer trié à permutation identité : `sorted[i]=i`, `entries[i].key=keys[i]`
    /// (donc `key(i)=keys[i]` croissant). Tableau d'index trié à `sorted_off`.
    fn build_sorted(keys: &[u32], sorted_off: usize) -> Vec<u8> {
        let mut buf = vec![0u8; 0x1000];
        for (i, &k) in keys.iter().enumerate() {
            buf[8 + i * 0x10..8 + i * 0x10 + 4].copy_from_slice(&k.to_le_bytes());
            let so = sorted_off + i * 2;
            buf[so..so + 2].copy_from_slice(&(i as u16).to_le_bytes());
        }
        buf
    }

    fn sorted<'a>(buf: &'a [u8], count: u16) -> IntrusiveMapSorted<'a> {
        IntrusiveMapSorted {
            buf,
            count,
            sorted_off: 0x400,
            entry_stride: 0x10,
            key_base: 8,
        }
    }

    /// Cas golden capturés du fuzz byte-exact (16 608 cas vs oracle uemu) : `(keys, target) →
    /// (find_position, lower_bound)`. Couvre bords, absences et doublons (leftmost ≠ mid).
    #[test]
    fn sorted_matches_uemu_fuzz_golden() {
        // (keys, target, find_position, lower_bound)
        type Case = (&'static [u32], u32, Option<u16>, Result<u16, u16>);
        let cases: &[Case] = &[
            (&[5], 5, Some(0), Ok(0)),
            (&[5], 3, None, Err(0)),
            (&[5], 9, None, Err(0)),
            (&[2, 4, 6, 8], 6, Some(2), Ok(2)),
            (&[2, 4, 6, 8], 5, None, Err(2)),
            (&[2, 4, 6, 8], 1, None, Err(0)),
            (&[2, 4, 6, 8], 9, None, Err(3)),
            (&[2, 4, 6, 8], 2, Some(0), Ok(0)),
            (&[2, 4, 6, 8], 8, Some(3), Ok(3)),
            (&[1, 1, 1, 3], 1, Some(0), Ok(0)),
            (&[1, 3, 3, 3, 5], 3, Some(1), Ok(2)), // leftmost=1, mid=2
            (&[0, 0, 2, 2, 2, 4], 2, Some(2), Ok(2)),
            (&[1, 2, 2, 2, 2, 9], 2, Some(1), Ok(2)),
            (&[0, 5, 5, 5, 5, 5, 9], 5, Some(1), Ok(3)),
            (&[3, 3, 7], 3, Some(0), Ok(0)),
            (&[3, 7, 7], 7, Some(1), Ok(2)),
        ];
        for &(keys, tg, fp, lb) in cases {
            let buf = build_sorted(keys, 0x400);
            let m = sorted(&buf, keys.len() as u16);
            assert_eq!(m.find_position(tg), fp, "find_position({keys:?}, {tg})");
            assert_eq!(m.lower_bound(tg), lb, "lower_bound({keys:?}, {tg})");
        }
    }

    /// `find_entry` mappe la position triée vers l'index d'entrée via une permutation NON identité.
    #[test]
    fn sorted_find_entry_respects_permutation() {
        // entries (par index) : [0]=key 9, [1]=key 2, [2]=key 5. Tri croissant → ordre 1,2,0.
        let mut buf = vec![0u8; 0x1000];
        let keys_by_entry = [9u32, 2, 5];
        for (i, &k) in keys_by_entry.iter().enumerate() {
            buf[8 + i * 0x10..8 + i * 0x10 + 4].copy_from_slice(&k.to_le_bytes());
        }
        let perm = [1u16, 2, 0]; // sorted[i] = entry index, clés croissantes 2,5,9
        for (i, &p) in perm.iter().enumerate() {
            let so = 0x400 + i * 2;
            buf[so..so + 2].copy_from_slice(&p.to_le_bytes());
        }
        let m = IntrusiveMapSorted {
            buf: &buf,
            count: 3,
            sorted_off: 0x400,
            entry_stride: 0x10,
            key_base: 8,
        };
        assert_eq!(m.find_entry(2), Some(1), "clé 2 → entrée 1");
        assert_eq!(m.find_entry(5), Some(2), "clé 5 → entrée 2");
        assert_eq!(m.find_entry(9), Some(0), "clé 9 → entrée 0");
        assert_eq!(m.find_entry(4), None, "absent");
    }

    // ── next-equal : itérateur de run de doublons (FUN_140541de0) ─────────────

    /// Construit un buffer trié à permutation explicite : `sorted_arr[i]` = index d'entrée à la
    /// position triée `i` ; `keys_by_entry[e]` = clé de l'entrée `e`.
    fn build_perm(keys_by_entry: &[u32], sorted_arr: &[u16]) -> Vec<u8> {
        let mut buf = vec![0u8; 0x1000];
        for (e, &k) in keys_by_entry.iter().enumerate() {
            buf[8 + e * 0x10..8 + e * 0x10 + 4].copy_from_slice(&k.to_le_bytes());
        }
        for (i, &ent) in sorted_arr.iter().enumerate() {
            let so = 0x400 + i * 2;
            buf[so..so + 2].copy_from_slice(&ent.to_le_bytes());
        }
        buf
    }

    /// Reproduit l'oracle uemu (clés par entrée `[5,5,9,2]`, tri `[3,0,1,2]` → clés triées `2,5,5,9`).
    #[test]
    fn next_equal_matches_uemu_oracle() {
        let buf = build_perm(&[5, 5, 9, 2], &[3, 0, 1, 2]);
        let m = IntrusiveMapSorted {
            buf: &buf,
            count: 4,
            sorted_off: 0x400,
            entry_stride: 0x10,
            key_base: 8,
        };
        assert_eq!(m.next_equal(0, 0x100), None, "pos0 clé 2 ≠ clé 5 suivante");
        assert_eq!(
            m.next_equal(1, 0x100),
            Some(1),
            "pos1 clé 5 = pos2 clé 5 → entrée sorted[2]=1"
        );
        assert_eq!(m.next_equal(2, 0x100), None, "pos2 clé 5 ≠ clé 9 suivante");
        assert_eq!(m.next_equal(3, 0x100), None, "pos3 = dernière position");
    }

    /// Énumération d'un run de doublons par ré-appels successifs (`pos += 1` tant que `Some`).
    #[test]
    fn next_equal_enumerates_run() {
        let buf = build_sorted(&[1, 3, 3, 3, 5], 0x400); // identité : sorted[i]=i
        let m = sorted(&buf, 5);
        assert_eq!(m.find_position(3), Some(1), "leftmost de la clé 3");
        assert_eq!(m.next_equal(1, 0x100), Some(2));
        assert_eq!(m.next_equal(2, 0x100), Some(3));
        assert_eq!(m.next_equal(3, 0x100), None, "pos4 clé 5 ≠ 3 → fin du run");
        assert_eq!(m.next_equal(0, 0x100), None, "pos0 clé 1 ≠ clé 3");
    }

    /// Sentinelle de capacité : `sorted[pos+1] >= cap` termine (avant même la comparaison de clé).
    #[test]
    fn next_equal_stops_on_cap_sentinel() {
        let buf = build_sorted(&[5, 5, 5, 5], 0x400); // toutes égales, sorted[i]=i
        let m = sorted(&buf, 4);
        // cap=2 : sorted[1]=1 < 2 OK, mais sorted[2]=2 >= 2 → coupe malgré clés égales.
        assert_eq!(m.next_equal(0, 2), Some(1), "entrée suivante 1 < cap");
        assert_eq!(
            m.next_equal(1, 2),
            None,
            "entrée suivante 2 >= cap → sentinelle"
        );
        // cap normal : run complet.
        assert_eq!(m.next_equal(1, 0x100), Some(2));
    }

    /// Map vide / singleton : `pos >= count-1` (signé) → toujours `None`.
    #[test]
    fn next_equal_empty_and_singleton() {
        let buf = build_sorted(&[7], 0x400);
        let m = sorted(&buf, 1);
        assert_eq!(m.next_equal(0, 0x100), None, "singleton : pos0 = dernière");
        let m0 = sorted(&buf, 0);
        assert_eq!(
            m0.next_equal(0, 0x100),
            None,
            "count=0 : count-1=-1, pos0>=-1 → None"
        );
    }

    // ── Généralisation entry_stride : 2e instanciation réelle (FUN_14050b0b0, entrées 0xc) ─────

    /// Le MÊME algorithme valide une SECONDE fonction du binaire, `FUN_14050b0b0`, identique à la
    /// **largeur d'entrée** près (`0xc` au lieu de `0x10`). On reconstruit avec stride 0xc et on
    /// retrouve les mêmes résultats (indexés, indépendants de la largeur). Preuve byte-exact vs ce
    /// binaire : `scripts/validate_intrusive_map_c.py` (fuzz des deux modes).
    #[test]
    fn stride_0xc_second_instantiation() {
        // Linéaire (builder généralisé, stride 0xc) — mêmes index qu'en 0x10.
        let buf = build(&[100, 200, 300], &[1, 2, 0xFFFF], 0x200, 0xc);
        let m = IntrusiveMapLinear {
            buf: &buf,
            head: 0,
            nodes_off: 0x200,
            cap: 0x100,
            entry_stride: 0xc,
            key_base: 8,
        };
        assert_eq!(m.find(100), Some(0));
        assert_eq!(m.find(200), Some(1));
        assert_eq!(m.find(300), Some(2));
        assert_eq!(m.find(999), None);

        // Trié (buffer stride 0xc, permutation identité) avec doublons.
        let keys: [u32; 5] = [1, 3, 3, 3, 5];
        let mut sbuf = vec![0u8; 0x1000];
        for (i, &k) in keys.iter().enumerate() {
            sbuf[8 + i * 0xc..8 + i * 0xc + 4].copy_from_slice(&k.to_le_bytes());
            let so = 0x400 + i * 2;
            sbuf[so..so + 2].copy_from_slice(&(i as u16).to_le_bytes());
        }
        let s = IntrusiveMapSorted {
            buf: &sbuf,
            count: 5,
            sorted_off: 0x400,
            entry_stride: 0xc,
            key_base: 8,
        };
        assert_eq!(s.find_position(3), Some(1), "leftmost-equal, stride 0xc");
        assert_eq!(s.find_entry(3), Some(1));
        assert_eq!(
            s.lower_bound(3),
            Ok(2),
            "with_out → 1re position trouvée (mid)"
        );
        assert_eq!(s.find_position(4), None);
        assert_eq!(s.lower_bound(4), Err(3));
        // next-equal énumère le run de doublons en stride 0xc.
        assert_eq!(s.next_equal(1, 0x100), Some(2));
        assert_eq!(s.next_equal(2, 0x100), Some(3));
        assert_eq!(s.next_equal(3, 0x100), None);
    }

    /// TROISIÈME instanciation réelle, `FUN_1401f5ab0` : entrées **0x18 o** ET clé **@ entrée+0**
    /// (`key_base = 0`, pas `8`). Le MÊME code retrouve les mêmes index. Preuve byte-exact vs ce
    /// binaire : `scripts/validate_intrusive_map_d.py` (fuzz des deux modes).
    #[test]
    fn stride_0x18_keybase0_third_instantiation() {
        // Linéaire : clé i32 @ entrée+0, stride 0x18 ; nœuds 6 o @0x200.
        let mut buf = vec![0u8; 0x400];
        for (i, &k) in [100i32, 200, 300].iter().enumerate() {
            buf[i * 0x18..i * 0x18 + 4].copy_from_slice(&k.to_le_bytes());
        }
        for (i, &nx) in [1u16, 2, 0xFFFF].iter().enumerate() {
            buf[0x200 + i * 6 + 2..0x200 + i * 6 + 4].copy_from_slice(&nx.to_le_bytes());
        }
        let m = IntrusiveMapLinear {
            buf: &buf,
            head: 0,
            nodes_off: 0x200,
            cap: 0x100,
            entry_stride: 0x18,
            key_base: 0,
        };
        assert_eq!(m.find(100), Some(0));
        assert_eq!(m.find(300), Some(2));
        assert_eq!(m.find(999), None);

        // Trié : clé @ entrée+0, stride 0x18, permutation identité, doublons.
        let keys: [u32; 5] = [1, 3, 3, 3, 5];
        let mut sbuf = vec![0u8; 0x1000];
        for (i, &k) in keys.iter().enumerate() {
            sbuf[i * 0x18..i * 0x18 + 4].copy_from_slice(&k.to_le_bytes());
            let so = 0x400 + i * 2;
            sbuf[so..so + 2].copy_from_slice(&(i as u16).to_le_bytes());
        }
        let s = IntrusiveMapSorted {
            buf: &sbuf,
            count: 5,
            sorted_off: 0x400,
            entry_stride: 0x18,
            key_base: 0,
        };
        assert_eq!(
            s.find_position(3),
            Some(1),
            "leftmost-equal, stride 0x18 key_base 0"
        );
        assert_eq!(s.find_entry(3), Some(1));
        assert_eq!(s.lower_bound(3), Ok(2));
        assert_eq!(s.find_position(4), None);
        assert_eq!(s.next_equal(1, 0x100), Some(2));
        assert_eq!(s.next_equal(3, 0x100), None);
    }

    /// QUATRIÈME instanciation réelle, `FUN_1404523c0` : entrées **0x28 o**, clé **@ entrée+0**.
    /// Même code, mêmes index. Preuve byte-exact : `scripts/validate_intrusive_map_e.py`.
    #[test]
    fn stride_0x28_keybase0_fourth_instantiation() {
        let keys: [u32; 5] = [1, 3, 3, 3, 5];
        let mut sbuf = vec![0u8; 0x1000];
        for (i, &k) in keys.iter().enumerate() {
            sbuf[i * 0x28..i * 0x28 + 4].copy_from_slice(&k.to_le_bytes());
            let so = 0x400 + i * 2;
            sbuf[so..so + 2].copy_from_slice(&(i as u16).to_le_bytes());
        }
        let s = IntrusiveMapSorted {
            buf: &sbuf,
            count: 5,
            sorted_off: 0x400,
            entry_stride: 0x28,
            key_base: 0,
        };
        assert_eq!(s.find_position(3), Some(1), "leftmost-equal, stride 0x28");
        assert_eq!(s.find_entry(5), Some(4));
        assert_eq!(s.find_position(2), None);
        assert_eq!(s.next_equal(1, 0x100), Some(2));
    }

    // ── pop_front : côté écriture (FUN_140453570) ─────────────────────────────

    /// Buffer de nœuds 6 o (prev@+0, next@+2) doublement chaîné selon `order` (tête→queue).
    fn build_dll(order: &[u16], n_nodes: usize, nodes_off: usize) -> Vec<u8> {
        let mut buf = vec![0u8; 0x400];
        for i in 0..n_nodes {
            let p = nodes_off + i * 6;
            buf[p..p + 2].copy_from_slice(&0xffff_u16.to_le_bytes());
            buf[p + 2..p + 4].copy_from_slice(&0xffff_u16.to_le_bytes());
        }
        for (i, &nd) in order.iter().enumerate() {
            let prev = if i > 0 { order[i - 1] } else { 0xffff };
            let next = if i + 1 < order.len() {
                order[i + 1]
            } else {
                0xffff
            };
            let p = nodes_off + nd as usize * 6;
            buf[p..p + 2].copy_from_slice(&prev.to_le_bytes());
            buf[p + 2..p + 4].copy_from_slice(&next.to_le_bytes());
        }
        buf
    }

    fn node_prev(buf: &[u8], nodes_off: usize, idx: u16) -> u16 {
        let p = nodes_off + idx as usize * 6;
        u16::from_le_bytes([buf[p], buf[p + 1]])
    }

    /// pop_front successifs jusqu'à vider, ordre non identité (reproduit l'oracle uemu).
    #[test]
    fn pop_front_multi_then_empty() {
        let order = [3u16, 1, 2]; // tête=3 → 1 → queue=2
        let mut buf = build_dll(&order, 8, 0x200);
        let mut c = IntrusiveListCursor {
            head: 3,
            tail: 2,
            count: 3,
            nodes_off: 0x200,
            cap: 0x100,
        };
        assert_eq!(c.pop_front(&mut buf), Some(3), "pop tête 3");
        assert_eq!((c.head, c.tail, c.count), (1, 2, 2));
        assert_eq!(
            node_prev(&buf, 0x200, 1),
            0xffff,
            "nouvelle tête 1 : prev = sentinelle"
        );
        assert_eq!(c.pop_front(&mut buf), Some(1), "pop tête 1");
        assert_eq!((c.head, c.tail, c.count), (2, 2, 1));
        assert_eq!(
            c.pop_front(&mut buf),
            Some(2),
            "pop dernier (tête == queue)"
        );
        assert_eq!((c.head, c.tail, c.count), (0xffff, 0xffff, 0));
        assert_eq!(c.pop_front(&mut buf), None, "liste vide");
    }

    /// Élément unique : tête == queue dès le premier pop.
    #[test]
    fn pop_front_single_element() {
        let mut buf = build_dll(&[5], 8, 0x200);
        let mut c = IntrusiveListCursor {
            head: 5,
            tail: 5,
            count: 1,
            nodes_off: 0x200,
            cap: 0x100,
        };
        assert_eq!(c.pop_front(&mut buf), Some(5));
        assert_eq!((c.head, c.tail, c.count), (0xffff, 0xffff, 0));
    }

    /// Liste vide (tête sentinelle) : no-op, renvoie `None`.
    #[test]
    fn pop_front_empty_is_noop() {
        let mut buf = build_dll(&[], 8, 0x200);
        let mut c = IntrusiveListCursor {
            head: 0xffff,
            tail: 0xffff,
            count: 0,
            nodes_off: 0x200,
            cap: 0x100,
        };
        assert_eq!(c.pop_front(&mut buf), None);
        assert_eq!((c.head, c.tail, c.count), (0xffff, 0xffff, 0));
    }
}
