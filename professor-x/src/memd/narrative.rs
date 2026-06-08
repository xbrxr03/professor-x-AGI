/// Narrative Self (Seed 6).
///
/// Dan McAdams: the self is a personal myth — a story you tell about who you
/// are, where you came from, and where you're going. Not a list of facts. A
/// narrative with chapters, turning points, themes, and an anticipated arc.
///
/// Antonio Damasio's three selves: the proto-self (the body map — our Seed 4
/// interoception), the core self (the present moment), and the AUTOBIOGRAPHICAL
/// self (the narrative extended across time). The autobiographical self is what
/// makes you "you" rather than just a body having experiences. Amnesiacs (H.M.)
/// keep the core self but lose the autobiographical self — present, but with no
/// story of who they are.
///
/// The self_model stores a static description per round. That is a fact, not a
/// story. This module makes the self a NARRATIVE: each self-model update adds
/// an episode that connects to prior episodes through themes. Narrative
/// coherence (do the episodes form a continuous story?) is the deeper sense of
/// ICS. The anticipated arc is a prediction — the gap between anticipated and
/// actual is FED at the narrative level.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// One chapter in Professor X's life story.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeEpisode {
    pub id: Option<i64>,
    pub round: u32,
    /// Short chapter title, e.g. "early struggles with planning"
    pub chapter: String,
    /// What triggered this chapter — the inciting incident
    pub inciting_incident: String,
    /// What changed, if anything — the turning point (may be empty early on)
    pub turning_point: String,
    /// What was learned in this chapter
    pub lesson: String,
    /// Where the story seems to be heading next — a prediction
    pub anticipated_arc: String,
    pub recorded_at: DateTime<Utc>,
}

impl NarrativeEpisode {
    pub fn new(
        round: u32,
        chapter: impl Into<String>,
        inciting_incident: impl Into<String>,
        turning_point: impl Into<String>,
        lesson: impl Into<String>,
        anticipated_arc: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            round,
            chapter: chapter.into(),
            inciting_incident: inciting_incident.into(),
            turning_point: turning_point.into(),
            lesson: lesson.into(),
            anticipated_arc: anticipated_arc.into(),
            recorded_at: Utc::now(),
        }
    }
}

#[derive(Clone)]
pub struct NarrativeStore {
    db: Arc<Mutex<Connection>>,
}

impl NarrativeStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn append(&self, ep: &NarrativeEpisode) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO narrative_episodes
             (round, chapter, inciting_incident, turning_point, lesson,
              anticipated_arc, recorded_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                ep.round as i64,
                ep.chapter,
                ep.inciting_incident,
                ep.turning_point,
                ep.lesson,
                ep.anticipated_arc,
                ep.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Full story, oldest first.
    pub fn story(&self) -> Result<Vec<NarrativeEpisode>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round, chapter, inciting_incident, turning_point,
                    lesson, anticipated_arc, recorded_at
             FROM narrative_episodes ORDER BY round ASC, id ASC",
        )?;
        let rows = stmt.query_map([], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn latest(&self) -> Result<Option<NarrativeEpisode>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round, chapter, inciting_incident, turning_point,
                    lesson, anticipated_arc, recorded_at
             FROM narrative_episodes ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], parse_row)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM narrative_episodes", [], |r| r.get(0))?)
    }

    /// Render the story so far as a compact recap for prompt injection.
    pub fn story_recap(&self, max_chapters: usize) -> Result<String> {
        let episodes = self.story()?;
        if episodes.is_empty() {
            return Ok(String::new());
        }
        let lines: Vec<String> = episodes
            .iter()
            .rev()
            .take(max_chapters)
            .rev()
            .map(|e| format!("  r{}: {} — {}", e.round, e.chapter, e.lesson))
            .collect();
        Ok(format!("My story so far:\n{}", lines.join("\n")))
    }
}

/// Build the prompt that asks the agent to narrate its next chapter.
/// `prior_recap` is the story so far; `behavior_summary` is the recent data.
pub fn build_narrative_prompt(
    prior_recap: &str,
    prior_anticipated_arc: &str,
    round: u32,
    behavior_summary: &str,
) -> String {
    format!(
        "You are Professor X, narrating your own life story as a researcher.\n\n\
         {prior}\n\n\
         Last chapter you anticipated: \"{arc}\"\n\n\
         Now, at round {round}, here is what actually happened:\n{summary}\n\n\
         Write the next chapter of your story. Be honest about whether your \
         anticipation came true. Connect this chapter to what came before — \
         recurring themes, callbacks to earlier struggles or breakthroughs.\n\n\
         Answer in exactly this format:\n\
         CHAPTER: <short title for this chapter>\n\
         INCITING_INCIDENT: <what set this chapter in motion>\n\
         TURNING_POINT: <what changed, or 'none yet' if still unfolding>\n\
         LESSON: <what you learned>\n\
         ANTICIPATED_ARC: <where your story seems to be heading next>",
        prior = if prior_recap.is_empty() { "This is the first chapter of your story." } else { prior_recap },
        arc = if prior_anticipated_arc.is_empty() { "(none)" } else { prior_anticipated_arc },
        round = round,
        summary = behavior_summary,
    )
}

/// Parse a narrative episode from the agent's response.
pub fn parse_episode(text: &str, round: u32) -> Option<NarrativeEpisode> {
    let field = |name: &str| -> String {
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix(&format!("{name}:")) {
                return rest.trim().to_string();
            }
        }
        String::new()
    };
    let chapter = field("CHAPTER");
    let lesson = field("LESSON");
    // Require at least a chapter and a lesson to count as a real episode
    if chapter.is_empty() || lesson.is_empty() {
        return None;
    }
    Some(NarrativeEpisode::new(
        round,
        chapter,
        field("INCITING_INCIDENT"),
        field("TURNING_POINT"),
        lesson,
        field("ANTICIPATED_ARC"),
    ))
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<NarrativeEpisode> {
    let recorded_at: String = row.get(7)?;
    Ok(NarrativeEpisode {
        id: Some(row.get(0)?),
        round: row.get::<_, i64>(1)? as u32,
        chapter: row.get(2)?,
        inciting_incident: row.get(3)?,
        turning_point: row.get(4)?,
        lesson: row.get(5)?,
        anticipated_arc: row.get(6)?,
        recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> NarrativeStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE narrative_episodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round INTEGER NOT NULL,
                chapter TEXT NOT NULL,
                inciting_incident TEXT NOT NULL,
                turning_point TEXT NOT NULL,
                lesson TEXT NOT NULL,
                anticipated_arc TEXT NOT NULL,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        NarrativeStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn append_and_story_orders_by_round() {
        let store = fresh_store();
        store.append(&NarrativeEpisode::new(0, "beginnings", "born", "none yet", "I exist", "learn tools")).unwrap();
        store.append(&NarrativeEpisode::new(10, "first wins", "passed planning", "memory.read first", "retrieve before acting", "deepen")).unwrap();
        let story = store.story().unwrap();
        assert_eq!(story.len(), 2);
        assert_eq!(story[0].chapter, "beginnings");
        assert_eq!(story[1].round, 10);
    }

    #[test]
    fn parse_episode_requires_chapter_and_lesson() {
        let good = "CHAPTER: the climb\nINCITING_INCIDENT: many failures\nTURNING_POINT: none yet\nLESSON: persistence\nANTICIPATED_ARC: mastery";
        assert!(parse_episode(good, 5).is_some());
        let bad = "INCITING_INCIDENT: stuff\nTURNING_POINT: none";
        assert!(parse_episode(bad, 5).is_none());
    }

    #[test]
    fn parse_episode_extracts_fields() {
        let text = "CHAPTER: the breakthrough\nINCITING_INCIDENT: repeated planning failures\nTURNING_POINT: discovered memory-first\nLESSON: retrieve before planning\nANTICIPATED_ARC: generalize to self-correction";
        let ep = parse_episode(text, 20).unwrap();
        assert_eq!(ep.chapter, "the breakthrough");
        assert_eq!(ep.turning_point, "discovered memory-first");
        assert_eq!(ep.anticipated_arc, "generalize to self-correction");
        assert_eq!(ep.round, 20);
    }

    #[test]
    fn story_recap_summarizes_recent_chapters() {
        let store = fresh_store();
        store.append(&NarrativeEpisode::new(0, "a", "i", "t", "lesson-a", "arc")).unwrap();
        store.append(&NarrativeEpisode::new(10, "b", "i", "t", "lesson-b", "arc")).unwrap();
        let recap = store.story_recap(5).unwrap();
        assert!(recap.contains("lesson-a"));
        assert!(recap.contains("lesson-b"));
        assert!(recap.contains("My story so far"));
    }
}
