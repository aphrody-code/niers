//! Vues minimales sur le JSON `cfg.bin.json` produit par inagle (CfgBin → JSON).
//!
//! Format (vérifié sur les dumps réels) : un fichier a `{ "entries": [Node, ...] }` ou,
//! pour les `*_config` à listes, `{ "version": n, "lists": [{ "name", "typeName", "values": [...] }] }`.
//!
//! Un `Node` = `{ "name": String, "variables": [Var], "children": [Node] }`.
//!
//! ## Deux formes de `Var` — les deux sont acceptées
//!
//! | forme | exemple | producteur |
//! |-------|---------|------------|
//! | **iecode** (« étiquetée ») | `{"type":"Int","value":"128840881"}` | dumps `*.cfg.bin.json` (inagle / C#) |
//! | **native** (serde, tag externe) | `{"Int":128840881}` | [`nie_formats::cfgbin::Value`] via `niers decode` |
//!
//! La forme iecode porte **toujours la valeur en chaîne** ; la forme native porte un
//! `serde_json::Number` (ou une chaîne pour `String`). Sans le support de la seconde, un JSON
//! produit par la CLI du dépôt se lisait « toutes variables absentes » — les parseurs
//! renvoyaient des listes **vides sans erreur**. Les deux formes sont donc reconnues ici, à
//! l'unique endroit où les variables sont lues, ce qui vaut pour toute la famille `nie-data`.
//!
//! Source : `packages/inagle/src/core/config-parser.ts` (ConfigNode/ConfigVariable),
//! `packages/inagle/src/characters/types.ts` (CfgBinEntry/CfgBinVariable). Échantillon réel :
//! `/home/ubuntu/niers/data/common/text/fr/skill_text.cfg.bin.json`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::hash::HashId;

/// Une variable CfgBin, `value` réinterprétée à la demande.
///
/// Couvre les deux formes documentées en tête de module : `value` porte la chaîne de la forme
/// iecode (`""` quand la forme native n'en fournit pas), `num` porte le scalaire JSON de la
/// forme native. Les accesseurs privilégient `num` quand il est présent.
#[derive(Debug, Clone)]
pub struct Var<'a> {
    /// Type déclaré (`"Int"`, `"String"`, `"Float"`, `"Hash"`, …).
    pub ty: &'a str,
    /// Valeur brute textuelle (forme iecode) ; `""` si la forme native porte un scalaire.
    pub value: &'a str,
    /// Scalaire JSON de la forme native (`{"Int":13}`) ; `None` en forme iecode.
    pub(crate) num: Option<&'a Value>,
}

impl<'a> Var<'a> {
    /// Lit la variable comme entier signé (`Number.parseInt(value, 10)` côté TS).
    /// Tolère les flottants en tronquant (les `Float` arrivent en `"0.5"`).
    #[must_use]
    pub fn as_i64(&self) -> i64 {
        // Forme native : le scalaire est déjà typé, aucun aller-retour par le texte.
        if let Some(n) = self.num {
            if let Some(v) = n.as_i64() {
                return v;
            }
            if let Some(f) = n.as_f64() {
                return f as i64;
            }
        }
        if let Ok(v) = self.value.trim().parse::<i64>() {
            return v;
        }
        // Float éventuel : on tronque vers zéro (parseInt s'arrête au premier non-chiffre).
        if let Ok(f) = self.value.trim().replace(',', ".").parse::<f64>() {
            return f as i64;
        }
        0
    }

    /// Lit comme `HashId` (signé→non-signé, comme `toHex(parseInt(...))`).
    #[must_use]
    pub fn as_hash(&self) -> HashId {
        HashId::from_i64(self.as_i64())
    }

    /// Lit comme flottant (`parseFloat(value.replace(",","."))`).
    #[must_use]
    pub fn as_f64(&self) -> f64 {
        if let Some(f) = self.num.and_then(Value::as_f64) {
            return f;
        }
        self.value
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .unwrap_or(0.0)
    }
}

/// Un noeud CfgBin (`{name, variables, children}`), emprunté sur un `serde_json::Value`.
#[derive(Debug, Clone, Copy)]
pub struct Node<'a> {
    raw: &'a Value,
}

impl<'a> Node<'a> {
    /// Enveloppe un `Value` JSON comme noeud.
    #[must_use]
    pub fn new(raw: &'a Value) -> Self {
        Node { raw }
    }

    /// Nom du noeud (`""` si absent).
    #[must_use]
    pub fn name(&self) -> &'a str {
        self.raw.get("name").and_then(Value::as_str).unwrap_or("")
    }

    /// Nombre de variables.
    #[must_use]
    pub fn var_count(&self) -> usize {
        self.raw
            .get("variables")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    }

    /// Variable à l'index `i`, ou `None`.
    ///
    /// Reconnaît les deux formes de variable (cf. tête de module) : `{"type","value"}` (iecode)
    /// et `{"<Type>": scalaire}` (native serde de `nie_formats`).
    #[must_use]
    pub fn var(&self, i: usize) -> Option<Var<'a>> {
        let arr = self.raw.get("variables")?.as_array()?;
        let v = arr.get(i)?;

        // Forme iecode : la clé `value` est présente et porte une chaîne.
        if let Some(value) = v.get("value").and_then(Value::as_str) {
            let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
            return Some(Var {
                ty,
                value,
                num: None,
            });
        }

        // Forme native : objet à clé unique, la clé EST le nom du type (`{"Int":13}`).
        let obj = v.as_object()?;
        let (ty, raw) = obj.iter().next().filter(|_| obj.len() == 1)?;
        Some(Var {
            ty: ty.as_str(),
            value: raw.as_str().unwrap_or(""),
            num: Some(raw),
        })
    }

    /// Entier signé de la variable `i` (0 si absente).
    #[must_use]
    pub fn int(&self, i: usize) -> i64 {
        self.var(i).map_or(0, |v| v.as_i64())
    }

    /// Hash de la variable `i` (`HashId::ZERO` si absente).
    #[must_use]
    pub fn hash(&self, i: usize) -> HashId {
        self.var(i).map_or(HashId::ZERO, |v| v.as_hash())
    }

    /// Chaîne brute de la variable `i` (`""` si absente).
    #[must_use]
    pub fn string(&self, i: usize) -> &'a str {
        self.var(i).map_or("", |v| v.value)
    }

    /// Enfants directs (vide si absents).
    #[must_use]
    pub fn children(&self) -> Vec<Node<'a>> {
        self.raw
            .get("children")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(Node::new).collect())
            .unwrap_or_default()
    }
}

/// Visite récursive : appelle `f` sur chaque noeud (`entries` + descendants) dont le nom
/// commence par `prefix`. Parcours profondeur d'abord, comme le `traverse` d'inagle.
pub fn walk_named<'a, F: FnMut(Node<'a>)>(root: &'a Value, prefix: &str, mut f: F) {
    let mut stack: Vec<Node<'a>> = Vec::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for e in entries.iter().rev() {
            stack.push(Node::new(e));
        }
    }
    while let Some(node) = stack.pop() {
        if node.name().starts_with(prefix) {
            f(node);
        }
        let kids = node.children();
        for k in kids.into_iter().rev() {
            stack.push(k);
        }
    }
}

/// Renvoie la liste `lists[].values` (tableaux JSON) du `name` donné, pour les `*_config`.
#[must_use]
pub fn list_values<'a>(root: &'a Value, list_name: &str) -> Option<&'a Vec<Value>> {
    let lists = root.get("lists")?.as_array()?;
    for l in lists {
        if l.get("name").and_then(Value::as_str) == Some(list_name) {
            return l.get("values")?.as_array();
        }
    }
    None
}

/// Helper : lit un champ chaîne d'un objet `values[]` (les `*_config` mettent les hash en hex).
#[must_use]
pub fn field_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// Helper : lit un champ entier d'un objet `values[]`.
#[must_use]
pub fn field_i64(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_i64)
}

/// Helper : lit un champ flottant d'un objet `values[]`.
///
/// `serde_json::Value::as_f64` accepte les nombres JSON entiers **et** décimaux, donc ce
/// helper convient aux poids stockés en `float` (ex. `m_spiritDropDataList[].weight = 20.75`).
#[must_use]
pub fn field_f64(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

/// Helper : lit un champ booléen d'un objet `values[]`.
#[must_use]
pub fn field_bool(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}

/// Helper : lit un champ tableau `[offset, count]` d'un objet `values[]` (idiome `data`/`tableInfo`).
///
/// `None` si le champ n'est pas un tableau (port du `if (!Array.isArray) continue` d'inagle).
/// Les éléments manquants valent `0`.
#[must_use]
pub fn field_pair(v: &Value, key: &str) -> Option<[i64; 2]> {
    let arr = v.get(key)?.as_array()?;
    let offset = arr.first().and_then(Value::as_i64).unwrap_or(0);
    let count = arr.get(1).and_then(Value::as_i64).unwrap_or(0);
    Some([offset, count])
}

/// Helper : lit un champ hash (hex string `"0x..."` ou entier) d'un objet `values[]`.
#[must_use]
pub fn field_hash(v: &Value, key: &str) -> HashId {
    match v.get(key) {
        Some(Value::String(s)) => HashId::parse(s).unwrap_or(HashId::ZERO),
        Some(Value::Number(n)) => n.as_i64().map(HashId::from_i64).unwrap_or(HashId::ZERO),
        _ => HashId::ZERO,
    }
}

/// Représentation possédée d'une chaîne empruntée (utilitaire pour les structs `String`).
#[must_use]
pub fn owned(s: &str) -> String {
    String::from(s)
}
