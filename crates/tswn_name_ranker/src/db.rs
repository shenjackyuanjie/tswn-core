use crate::abcp_calibration::Candidate;
use crate::model::{NameRow, ResultDetailRow, ResultPartnerRow, ResultRow, TargetRow};

pub const SPECIAL_ABCP_THRESHOLD: f64 = 4500.0;
pub const ABCP_OUTPUT_THRESHOLD: f64 = SPECIAL_ABCP_THRESHOLD;
use anyhow::Context;
use rusqlite::{Connection, params};
use std::{
    collections::HashMap,
    fmt::Write,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}
impl Db {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("打开数据库 {path}"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init()?;
        Ok(db)
    }
    fn init(&self) -> anyhow::Result<()> {
        self.conn.lock().unwrap().execute_batch(r#"
CREATE TABLE IF NOT EXISTS names(id INTEGER PRIMARY KEY, raw TEXT NOT NULL UNIQUE, diy TEXT NOT NULL, text_type TEXT NOT NULL, created_at TEXT DEFAULT CURRENT_TIMESTAMP);
CREATE TABLE IF NOT EXISTS targets(id INTEGER PRIMARY KEY, weight REAL NOT NULL, raw TEXT NOT NULL UNIQUE, left_name TEXT NOT NULL, right_name TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS pair_scores(name_a INTEGER NOT NULL, name_b INTEGER NOT NULL, score REAL NOT NULL, samples INTEGER NOT NULL, target_signature TEXT NOT NULL, updated_at TEXT DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY(name_a,name_b));
CREATE TABLE IF NOT EXISTS results(name_id INTEGER PRIMARY KEY, rank INTEGER NOT NULL, score REAL NOT NULL, coefficient REAL NOT NULL, strength REAL NOT NULL, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
CREATE TABLE IF NOT EXISTS abcp_scores(name_a TEXT NOT NULL, name_b TEXT NOT NULL, score REAL NOT NULL, updated_at TEXT DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY(name_a,name_b));
CREATE TABLE IF NOT EXISTS abcp_complete_names(raw TEXT PRIMARY KEY, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
CREATE TABLE IF NOT EXISTS name_ranker_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS expanded_names(name_id INTEGER PRIMARY KEY, added_at TEXT DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY(name_id) REFERENCES names(id));
CREATE TABLE IF NOT EXISTS archived_names(name_id INTEGER PRIMARY KEY, reason TEXT NOT NULL, score REAL NOT NULL, archived_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
INSERT OR IGNORE INTO abcp_complete_names(raw) SELECT name_a FROM abcp_scores;
INSERT OR IGNORE INTO abcp_complete_names(raw) SELECT name_b FROM abcp_scores;
"#)?;
        let stored_threshold = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM name_ranker_meta WHERE key='abcp_output_threshold'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok();
        let expected = format!("{ABCP_OUTPUT_THRESHOLD:.0}");
        if stored_threshold.as_deref() != Some(expected.as_str()) {
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM abcp_complete_names", [])?;
            tx.execute(
                "INSERT OR REPLACE INTO name_ranker_meta(key,value) VALUES('abcp_output_threshold',?1)",
                [&expected],
            )?;
            tx.commit()?;
        }
        Ok(())
    }
    pub fn add_name(&self, raw: &str, diy: &str, typ: &str) -> anyhow::Result<bool> {
        Ok(self.conn.lock().unwrap().execute(
            "INSERT OR IGNORE INTO names(raw,diy,text_type) VALUES(?1,?2,?3)",
            params![raw, diy, typ],
        )? > 0)
    }
    pub fn text_type_version(&self) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT value FROM name_ranker_meta WHERE key='text_type_version'", [], |r| {
                r.get(0)
            })
            .ok())
    }
    pub fn replace_text_types(&self, rows: &[(i64, String)], version: &str) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (id, text_type) in rows {
            tx.execute("UPDATE names SET text_type=?1 WHERE id=?2", params![text_type, id])?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO name_ranker_meta(key,value) VALUES('text_type_version',?1)",
            [version],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn expanded_names(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT n.raw FROM expanded_names e JOIN names n ON n.id=e.name_id ORDER BY n.raw")?;
        Ok(stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
    }
    pub fn set_names_expanded(&self, raws: &[String], expanded: bool) -> anyhow::Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut changed = 0;
        for raw in raws {
            let id = tx.query_row("SELECT id FROM names WHERE raw=?1", [raw], |r| r.get::<_, i64>(0))?;
            changed += if expanded {
                tx.execute("INSERT OR IGNORE INTO expanded_names(name_id) VALUES(?1)", [id])?
            } else {
                tx.execute("DELETE FROM expanded_names WHERE name_id=?1", [id])?
            };
        }
        tx.commit()?;
        Ok(changed)
    }
    pub fn replace_targets(&self, rows: &[(f64, String, String, String)]) -> anyhow::Result<()> {
        let mut c = self.conn.lock().unwrap();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM targets", [])?;
        for (w, r, l, rr) in rows {
            tx.execute(
                "INSERT INTO targets(weight,raw,left_name,right_name) VALUES(?1,?2,?3,?4)",
                params![w, r, l, rr],
            )?;
        }
        tx.execute("DELETE FROM pair_scores", [])?;
        tx.execute("DELETE FROM results", [])?;
        tx.commit()?;
        Ok(())
    }
    pub fn names(&self) -> anyhow::Result<Vec<(NameRow, String)>> {
        let c = self.conn.lock().unwrap();
        let mut s = c.prepare("SELECT id,raw,text_type,diy FROM names ORDER BY id")?;
        let it = s.query_map([], |r| {
            Ok((
                NameRow {
                    id: r.get(0)?,
                    raw: r.get(1)?,
                    text_type: r.get(2)?,
                },
                r.get(3)?,
            ))
        })?;
        Ok(it.collect::<Result<_, _>>()?)
    }
    pub fn names_for_run(&self, skip_archived: bool) -> anyhow::Result<Vec<(NameRow, String)>> {
        let c = self.conn.lock().unwrap();
        let sql = if skip_archived {
            "SELECT n.id,n.raw,n.text_type,n.diy FROM names n LEFT JOIN archived_names a ON a.name_id=n.id WHERE a.name_id IS NULL ORDER BY n.id"
        } else {
            "SELECT id,raw,text_type,diy FROM names ORDER BY id"
        };
        let mut s = c.prepare(sql)?;
        let it = s.query_map([], |r| {
            Ok((
                NameRow {
                    id: r.get(0)?,
                    raw: r.get(1)?,
                    text_type: r.get(2)?,
                },
                r.get(3)?,
            ))
        })?;
        Ok(it.collect::<Result<_, _>>()?)
    }
    pub fn archived_name_ids(&self) -> anyhow::Result<std::collections::HashSet<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name_id FROM archived_names")?;
        Ok(stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
    }
    pub fn archive_names(&self, rows: &[(i64, String, f64)]) -> anyhow::Result<usize> {
        let mut c = self.conn.lock().unwrap();
        let tx = c.transaction()?;
        let mut added = 0;
        for (id, reason, score) in rows {
            added += tx.execute(
                "INSERT INTO archived_names(name_id,reason,score) VALUES(?1,?2,?3) ON CONFLICT(name_id) DO UPDATE SET reason=excluded.reason,score=excluded.score,updated_at=CURRENT_TIMESTAMP",
                params![id, reason, score],
            )?;
        }
        tx.commit()?;
        Ok(added)
    }
    pub fn replace_archived_names(&self, rows: &[(i64, String, f64)]) -> anyhow::Result<usize> {
        let mut c = self.conn.lock().unwrap();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM archived_names", [])?;
        for (id, reason, score) in rows {
            tx.execute(
                "INSERT INTO archived_names(name_id,reason,score) VALUES(?1,?2,?3)",
                params![id, reason, score],
            )?;
        }
        tx.commit()?;
        Ok(rows.len())
    }
    pub fn targets(&self) -> anyhow::Result<Vec<TargetRow>> {
        let c = self.conn.lock().unwrap();
        let mut s = c.prepare("SELECT weight,raw,left_name,right_name FROM targets ORDER BY id")?;
        let it = s.query_map([], |r| {
            Ok(TargetRow {
                weight: r.get(0)?,
                raw: r.get(1)?,
                left_name: r.get(2)?,
                right_name: r.get(3)?,
            })
        })?;
        Ok(it.collect::<Result<_, _>>()?)
    }
    pub fn signature(&self) -> anyhow::Result<String> {
        let ts = self.targets()?;
        Ok(ts.iter().map(|t| format!("{}:{:.12}", t.raw, t.weight)).collect::<Vec<_>>().join("\n"))
    }
    pub fn score(&self, a: i64, b: i64, sig: &str, samples: usize) -> anyhow::Result<Option<f64>> {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let c = self.conn.lock().unwrap();
        let mut s =
            c.prepare("SELECT score FROM pair_scores WHERE name_a=?1 AND name_b=?2 AND target_signature=?3 AND samples=?4")?;
        let mut q = s.query(params![a, b, sig, samples as i64])?;
        Ok(q.next()?.map(|r| r.get(0)).transpose()?)
    }
    pub fn save_scores_bulk(&self, rows: &[(i64, i64, f64)], samples: usize, sig: &str) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for &(a, b, score) in rows {
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            tx.execute("INSERT OR REPLACE INTO pair_scores(name_a,name_b,score,samples,target_signature,updated_at) VALUES(?1,?2,?3,?4,?5,CURRENT_TIMESTAMP)",params![a,b,score,samples as i64,sig])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn replace_results(&self, rows: &[(i64, usize, f64, f64, f64)]) -> anyhow::Result<()> {
        let mut c = self.conn.lock().unwrap();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM results", [])?;
        for (id, rank, score, coef, strength) in rows {
            tx.execute(
                "INSERT INTO results(name_id,rank,score,coefficient,strength) VALUES(?1,?2,?3,?4,?5)",
                params![id, *rank as i64, score, coef, strength],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn result_details(&self, legal: &std::collections::HashSet<(String, String)>) -> anyhow::Result<Vec<ResultDetailRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT n.id,r.rank,r.score,r.coefficient,r.strength,n.text_type,n.raw \
             FROM results r JOIN names n ON n.id=r.name_id ORDER BY r.rank",
        )?;
        let nodes = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)? as usize,
                    r.get::<_, f64>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let index = nodes.iter().enumerate().map(|(i, n)| (n.0, i)).collect::<HashMap<_, _>>();
        let mut choices = vec![Vec::<(f64, f64, usize)>::new(); nodes.len()];
        let mut stmt = conn.prepare(
            "SELECT p.name_a,p.name_b,p.score FROM pair_scores p \
             JOIN results ra ON ra.name_id=p.name_a \
             JOIN results rb ON rb.name_id=p.name_b WHERE p.samples=10000",
        )?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, f64>(2)?)))? {
            let (a, b, score) = row?;
            let i = index[&a];
            let j = index[&b];
            let key = if nodes[i].6 <= nodes[j].6 {
                (nodes[i].6.clone(), nodes[j].6.clone())
            } else {
                (nodes[j].6.clone(), nodes[i].6.clone())
            };
            if !legal.contains(&key) {
                continue;
            }
            choices[i].push((score * nodes[j].3, score, j));
            if i != j {
                choices[j].push((score * nodes[i].3, score, i));
            }
        }
        Ok(nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                choices[i].sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.2.cmp(&b.2)));
                ResultDetailRow {
                    result: ResultRow {
                        rank: node.1,
                        score: node.2,
                        text_type: node.5.clone(),
                        name: node.6.clone(),
                        coefficient: node.3,
                        strength: node.4,
                    },
                    partners: choices[i]
                        .iter()
                        .take(crate::ranker::TOP_PARTNERS)
                        .map(|&(_, win_rate, j)| ResultPartnerRow {
                            rank: nodes[j].1,
                            win_rate,
                            text_type: nodes[j].5.clone(),
                            name: nodes[j].6.clone(),
                        })
                        .collect(),
                }
            })
            .collect())
    }
    pub fn export_results(&self, legal: &std::collections::HashSet<(String, String)>) -> anyhow::Result<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT n.id,r.rank,r.score,r.coefficient,n.text_type,n.raw FROM results r JOIN names n ON n.id=r.name_id ORDER BY r.rank")?;
        let nodes = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)? as usize,
                    r.get::<_, f64>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let index = nodes.iter().enumerate().map(|(i, n)| (n.0, i)).collect::<HashMap<_, _>>();
        let mut choices = vec![Vec::<(f64, f64, usize)>::new(); nodes.len()];
        let mut stmt=conn.prepare("SELECT p.name_a,p.name_b,p.score FROM pair_scores p JOIN results ra ON ra.name_id=p.name_a JOIN results rb ON rb.name_id=p.name_b WHERE p.samples=10000")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, f64>(2)?)))? {
            let (a, b, score) = row?;
            let i = index[&a];
            let j = index[&b];
            let key = if nodes[i].5 <= nodes[j].5 {
                (nodes[i].5.clone(), nodes[j].5.clone())
            } else {
                (nodes[j].5.clone(), nodes[i].5.clone())
            };
            if !legal.contains(&key) {
                continue;
            }
            choices[i].push((score * nodes[j].3, score, j));
            if i != j {
                choices[j].push((score * nodes[i].3, score, i));
            }
        }
        let mut out = String::new();
        for (i, node) in nodes.iter().enumerate() {
            choices[i].sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.2.cmp(&b.2)));
            writeln!(out, "{}\t{:.6}\t{}\t{}", node.1, node.2, node.4, node.5)?;
            for &(_, win_rate, j) in choices[i].iter().take(crate::ranker::TOP_PARTNERS) {
                let partner = &nodes[j];
                writeln!(out, "    {}\t{:.6}\t{}\t{}", partner.1, win_rate, partner.4, partner.5)?;
            }
            writeln!(out)?;
        }
        Ok(out)
    }
    pub fn abcp_candidates(&self) -> anyhow::Result<Vec<Candidate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT x.name_a,x.name_b,x.score,ea.name_id IS NOT NULL OR eb.name_id IS NOT NULL,COALESCE(ra.coefficient,1.0),COALESCE(rb.coefficient,1.0) FROM abcp_scores x JOIN names a ON a.raw=x.name_a JOIN names b ON b.raw=x.name_b LEFT JOIN expanded_names ea ON ea.name_id=a.id LEFT JOIN expanded_names eb ON eb.name_id=b.id LEFT JOIN results ra ON ra.name_id=a.id LEFT JOIN results rb ON rb.name_id=b.id WHERE x.score>?1",
        )?;
        Ok(stmt
            .query_map([SPECIAL_ABCP_THRESHOLD], |r| {
                Ok(Candidate {
                    a: r.get(0)?,
                    b: r.get(1)?,
                    raw: r.get(2)?,
                    expanded: r.get(3)?,
                    coefficient_a: r.get(4)?,
                    coefficient_b: r.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    pub fn pending_abcp_names(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT n.raw FROM names n LEFT JOIN abcp_complete_names c ON c.raw=n.raw WHERE c.raw IS NULL ORDER BY n.raw",
        )?;
        Ok(stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
    }

    pub fn save_abcp_results(
        &self,
        pairs: &[(String, String)],
        scored: &[(String, String, f64)],
        completed: &[String],
    ) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (a, b) in pairs {
            tx.execute(
                "INSERT OR REPLACE INTO abcp_scores(name_a,name_b,score,updated_at)VALUES(?1,?2,0,CURRENT_TIMESTAMP)",
                params![a, b],
            )?;
        }
        for (a, b, score) in scored {
            tx.execute(
                "INSERT OR REPLACE INTO abcp_scores(name_a,name_b,score,updated_at)VALUES(?1,?2,?3,CURRENT_TIMESTAMP)",
                params![a, b, score],
            )?;
        }
        for raw in completed {
            tx.execute(
                "INSERT OR REPLACE INTO abcp_complete_names(raw,updated_at)VALUES(?1,CURRENT_TIMESTAMP)",
                [raw],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
