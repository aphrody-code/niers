//! Frontière de la boucle RE niers, adossée à redis.
//!
//! La récursion sur le call-graph (résoudre une fonction → empiler ses callees/voisins
//! non résolus) est matérialisée par une file FIFO redis dédupliquée : un SET `seen`
//! empêche de re-traiter une adresse déjà vue, une LIST `frontier` porte l'ordre BFS.
//! Plusieurs workers peuvent dépiler en parallèle (`pop` = `LPOP` atomique).
#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

use redis::{Commands, RedisError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("redis: {0}")]
    Redis(#[from] RedisError),
}

pub type Result<T> = std::result::Result<T, QueueError>;

/// File de frontière BFS pour un binaire donné (préfixe de clés = `niers:<binary>`).
pub struct Frontier {
    conn: redis::Connection,
    seen_key: String,
    list_key: String,
}

impl Frontier {
    /// Se connecte à redis (`url` ex. `redis://127.0.0.1/`) et cible l'espace de clés
    /// du binaire `tag` (ex. son sha256 court).
    pub fn connect(url: &str, tag: &str) -> Result<Self> {
        let client = redis::Client::open(url)?;
        let conn = client.get_connection()?;
        Ok(Self {
            conn,
            seen_key: format!("niers:{tag}:seen"),
            list_key: format!("niers:{tag}:frontier"),
        })
    }

    /// Empile une adresse si elle n'a jamais été vue. Renvoie `true` si réellement ajoutée.
    pub fn push(&mut self, addr: i64) -> Result<bool> {
        let added: i64 = self.conn.sadd(&self.seen_key, addr)?;
        if added == 1 {
            let _: i64 = self.conn.rpush(&self.list_key, addr)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Empile un lot d'adresses ; renvoie le nombre réellement ajoutées (non déjà vues).
    pub fn push_many(&mut self, addrs: &[i64]) -> Result<usize> {
        let mut n = 0;
        for &a in addrs {
            if self.push(a)? {
                n += 1;
            }
        }
        Ok(n)
    }

    /// Dépile la prochaine adresse à traiter (FIFO), ou `None` si la frontière est vide.
    pub fn pop(&mut self) -> Result<Option<i64>> {
        let v: Option<i64> = self.conn.lpop(&self.list_key, None)?;
        Ok(v)
    }

    /// Taille courante de la frontière.
    pub fn len(&mut self) -> Result<usize> {
        let n: usize = self.conn.llen(&self.list_key)?;
        Ok(n)
    }

    pub fn is_empty(&mut self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Nombre total d'adresses déjà vues (résolues ou en attente).
    pub fn seen_count(&mut self) -> Result<usize> {
        let n: usize = self.conn.scard(&self.seen_key)?;
        Ok(n)
    }

    /// Réinitialise complètement la frontière de ce binaire.
    pub fn reset(&mut self) -> Result<()> {
        let _: i64 = self
            .conn
            .del(&[self.seen_key.clone(), self.list_key.clone()])?;
        Ok(())
    }
}
