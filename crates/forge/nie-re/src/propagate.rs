//! Propagation de labels semi-supervisée sur le call-graph (la couche « auto-ML »).
//!
//! Principe : un petit ensemble d'ancres étiquetées (fonctions nommées, classes RTTI,
//! fonctions référençant une string connue, formats reconnus) diffuse son label de
//! sous-système à travers le graphe d'appels. À chaque round, une fonction non verrouillée
//! adopte le label majoritaire de ses voisins (callers + callees), pondéré par la
//! confiance. On itère jusqu'à stabilisation. C'est du label-spreading (Zhou et al.),
//! ici en version discrète sur graphe non orienté dérivé du call-graph.
//!
//! Cette fonction est pure (graphe en entrée, labels en sortie) pour rester testable et
//! `no_std`-friendly ; le câblage vers la base sqlite vit dans la boucle CLI.

use hashbrown::HashMap;

/// Un nœud du graphe de propagation.
#[derive(Debug, Clone)]
pub struct Node {
    /// Label de sous-système courant (`None` = inconnu).
    pub label: Option<u32>,
    /// Confiance dans le label (0.0..=1.0).
    pub confidence: f32,
    /// Ancre : label figé, jamais réécrit.
    pub locked: bool,
}

impl Node {
    #[must_use]
    pub fn unknown() -> Self {
        Self {
            label: None,
            confidence: 0.0,
            locked: false,
        }
    }

    #[must_use]
    pub fn anchor(label: u32) -> Self {
        Self {
            label: Some(label),
            confidence: 1.0,
            locked: true,
        }
    }
}

/// Graphe de propagation : nœuds + arêtes non orientées **pondérées** (dérivées
/// des callees/callers, des vtables, etc.).
#[derive(Debug, Default, Clone)]
pub struct PropagationGraph {
    pub nodes: Vec<Node>,
    /// Adjacence pondérée : `(voisin, poids)`. Symétrique attendue.
    pub adj: Vec<Vec<(u32, f32)>>,
}

impl PropagationGraph {
    #[must_use]
    pub fn with_capacity(n: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(n),
            adj: vec![Vec::new(); n],
        }
    }

    /// Ajoute un nœud, renvoie son index.
    pub fn add_node(&mut self, node: Node) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push(node);
        if (self.adj.len() as u32) <= idx {
            self.adj.resize((idx + 1) as usize, Vec::new());
        }
        idx
    }

    /// Arête non orientée pondérée entre `a` et `b` (poids = force de l'indice :
    /// appel direct 1.0, cohésion de vtable < 1.0, etc.).
    pub fn add_edge(&mut self, a: u32, b: u32, weight: f32) {
        if a == b {
            return;
        }
        let max = a.max(b) as usize;
        if self.adj.len() <= max {
            self.adj.resize(max + 1, Vec::new());
        }
        self.adj[a as usize].push((b, weight));
        self.adj[b as usize].push((a, weight));
    }

    /// Atténuation par round : la confiance diffusée décroît avec la distance aux ancres.
    const DECAY: f32 = 0.85;

    /// Exécute jusqu'à `max_rounds` rounds (ou jusqu'à stabilisation). Renvoie le nombre de
    /// nœuds nouvellement étiquetés.
    pub fn run(&mut self, max_rounds: usize) -> usize {
        // Amortissement par degré (anti-hub) : un nœud très connecté (utilitaires
        // alloc/string appelés partout) diffuse une influence atténuée par
        // `1/ln(deg+2)`, pour ne pas noyer les labels spécifiques.
        let degree_damp: Vec<f32> = self
            .adj
            .iter()
            .map(|a| 1.0 / ((a.len() as f32) + 2.0).ln())
            .collect();

        let mut newly_labeled = 0usize;
        for _ in 0..max_rounds {
            let mut changed = false;
            // Snapshot des labels pour un update synchrone.
            let snapshot: Vec<Option<(u32, f32)>> = self
                .nodes
                .iter()
                .map(|n| n.label.map(|l| (l, n.confidence)))
                .collect();

            for i in 0..self.nodes.len() {
                if self.nodes[i].locked {
                    continue;
                }
                // Vote des voisins étiquetés, pondéré par le poids d'arête × l'amortissement
                // de degré du voisin : pour chaque label on suit la masse totale (→ pureté)
                // et la contribution max d'un parent (→ décroissance avec la distance/force).
                let mut votes: HashMap<u32, (f32, f32)> = HashMap::new();
                for &(nb, w) in &self.adj[i] {
                    if let Some((lbl, conf)) = snapshot[nb as usize] {
                        let contrib = conf * w * degree_damp[nb as usize];
                        let e = votes.entry(lbl).or_insert((0.0, 0.0));
                        e.0 += contrib;
                        e.1 = e.1.max(contrib);
                    }
                }
                let Some((&best_label, &(best_sum, best_max))) = votes.iter().max_by(|a, b| {
                    a.1.0
                        .partial_cmp(&b.1.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) else {
                    continue;
                };
                let total: f32 = votes.values().map(|v| v.0).sum();
                if total <= 0.0 {
                    continue;
                }
                // confiance = confiance du meilleur parent × atténuation × pureté du vote.
                let purity = best_sum / total;
                let new_conf = best_max * Self::DECAY * purity;
                let node = &mut self.nodes[i];
                let was_none = node.label.is_none();
                if node.label != Some(best_label) || (new_conf - node.confidence).abs() > 1e-3 {
                    node.label = Some(best_label);
                    node.confidence = new_conf;
                    changed = true;
                    if was_none {
                        newly_labeled += 1;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        newly_labeled
    }

    /// Nombre de nœuds étiquetés (ancres comprises).
    #[must_use]
    pub fn labeled_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.label.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_spreads_from_anchor_through_call_chain() {
        // Chaîne 0(ancre game=1) - 1 - 2 - 3(inconnus) : le label doit diffuser.
        let mut g = PropagationGraph::default();
        let a = g.add_node(Node::anchor(1));
        let b = g.add_node(Node::unknown());
        let c = g.add_node(Node::unknown());
        let d = g.add_node(Node::unknown());
        g.add_edge(a, b, 1.0);
        g.add_edge(b, c, 1.0);
        g.add_edge(c, d, 1.0);
        let newly = g.run(16);
        assert_eq!(newly, 3, "les 3 nœuds inconnus reçoivent un label");
        assert_eq!(g.nodes[b as usize].label, Some(1));
        assert_eq!(g.nodes[d as usize].label, Some(1));
        // La confiance décroît avec la distance à l'ancre.
        assert!(g.nodes[b as usize].confidence > g.nodes[d as usize].confidence);
        assert_eq!(g.labeled_count(), 4);
    }

    #[test]
    fn competing_anchors_split_by_majority() {
        // 0(game=1) - 2 - 1(menu=2), nœud 2 entre deux ancres : majorité/poids départage.
        let mut g = PropagationGraph::default();
        let g1 = g.add_node(Node::anchor(1));
        let m2 = g.add_node(Node::anchor(2));
        let mid = g.add_node(Node::unknown());
        g.add_edge(g1, mid, 1.0);
        g.add_edge(m2, mid, 1.0);
        g.run(8);
        // À égalité parfaite le label est l'un des deux ; il doit être étiqueté.
        assert!(g.nodes[mid as usize].label.is_some());
    }
}
