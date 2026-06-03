/// Cross-Modal Resonance Binding.
///
/// The Binding Problem (Singer & Gray, 1989): the brain processes colour,
/// shape, motion, and sound in separate regions, yet experience is unified —
/// "a red ball bouncing with a thud," not four disconnected features. The
/// leading account is temporal synchrony: neurons representing features of the
/// SAME object fire together at gamma frequency. What makes features "belong"
/// is that they resonate together; attention selects which synchrony to amplify.
///
/// Professor X assembles each task's context from separate modalities —
/// episodic memory, semantic knowledge, cognition base, (later) vision. The
/// default is concatenation: put everything in the prompt and hope it coheres.
/// That is no binding at all, and it is where confabulation comes from — when
/// episodic memory suggests one thing and knowledge suggests another, a naive
/// blend produces a confident hallucination instead of surfacing the conflict.
///
/// This module binds by resonance: a feature is RETAINED and amplified only if
/// it is echoed across modalities (the same theme appears in episodic AND
/// cognition). Features coherent only within their own modality are suppressed.
/// The bound context is therefore multi-modally grounded — which both reduces
/// hallucination and raises integration (the IIT phi the system tracks).

use crate::embeddings::cosine_similarity;

/// A candidate context element from one modality, with its embedding.
#[derive(Debug, Clone)]
pub struct ModalityFeature {
    pub modality: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub base_relevance: f32,
}

/// A feature after binding: its cross-modal coherence and whether it was kept.
#[derive(Debug, Clone)]
pub struct BoundFeature {
    pub modality: String,
    pub content: String,
    /// Mean cosine similarity to the strongest matching feature in EACH other
    /// modality — how much this feature resonates across the system.
    pub coherence: f32,
    pub kept: bool,
}

/// Bind features by cross-modal resonance.
///
/// For each feature, coherence = mean over other modalities of the single
/// best cosine match in that modality. A feature echoed in 2+ modalities
/// scores high; a feature standing alone in its own modality scores low.
/// Features with coherence >= `threshold` are kept; the rest are suppressed.
///
/// Always retains at least the single highest-coherence feature so the context
/// is never empty when candidates exist.
pub fn bind(features: &[ModalityFeature], threshold: f32) -> Vec<BoundFeature> {
    if features.is_empty() {
        return Vec::new();
    }

    let mut bound: Vec<BoundFeature> = features
        .iter()
        .map(|f| {
            let coherence = cross_modal_coherence(f, features);
            BoundFeature {
                modality: f.modality.clone(),
                content: f.content.clone(),
                coherence,
                kept: coherence >= threshold,
            }
        })
        .collect();

    // Guarantee non-empty output: if nothing cleared the bar, keep the best.
    if !bound.iter().any(|b| b.kept) {
        if let Some(best) = bound
            .iter_mut()
            .max_by(|a, b| {
                a.coherence
                    .partial_cmp(&b.coherence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            best.kept = true;
        }
    }

    // Order kept features by coherence (strongest resonance first).
    bound.sort_by(|a, b| {
        b.coherence
            .partial_cmp(&a.coherence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    bound
}

/// Coherence of one feature = mean, over each OTHER modality present, of the
/// best cosine similarity to a feature in that modality. Weighted lightly by
/// the feature's own base relevance so a strongly-relevant-but-isolated item
/// isn't dropped purely for being unique.
fn cross_modal_coherence(feature: &ModalityFeature, all: &[ModalityFeature]) -> f32 {
    let other_modalities: std::collections::HashSet<&str> = all
        .iter()
        .filter(|f| f.modality != feature.modality)
        .map(|f| f.modality.as_str())
        .collect();

    if other_modalities.is_empty() {
        // Only one modality present — fall back to base relevance.
        return feature.base_relevance;
    }

    let mut sum = 0.0f32;
    for modality in &other_modalities {
        let best = all
            .iter()
            .filter(|f| &f.modality == modality)
            .map(|f| cosine_similarity(&feature.embedding, &f.embedding))
            .fold(0.0f32, f32::max);
        sum += best;
    }
    let cross = sum / other_modalities.len() as f32;
    // Blend: mostly cross-modal resonance, lightly anchored by own relevance.
    0.8 * cross + 0.2 * feature.base_relevance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feat(modality: &str, content: &str, emb: Vec<f32>, rel: f32) -> ModalityFeature {
        ModalityFeature {
            modality: modality.to_string(),
            content: content.to_string(),
            embedding: emb,
            base_relevance: rel,
        }
    }

    #[test]
    fn echoed_feature_scores_higher_than_isolated() {
        // episodic + cognition share a direction; a stray semantic is orthogonal.
        let features = vec![
            feat("episodic", "retrieval before planning works", vec![1.0, 0.0, 0.0], 0.6),
            feat("cognition", "memory-first improves planning", vec![0.95, 0.05, 0.0], 0.6),
            feat("semantic", "unrelated note about colour", vec![0.0, 0.0, 1.0], 0.6),
        ];
        let bound = bind(&features, 0.5);
        let echoed = bound.iter().find(|b| b.content.contains("retrieval before")).unwrap();
        let isolated = bound.iter().find(|b| b.content.contains("colour")).unwrap();
        assert!(echoed.coherence > isolated.coherence);
        assert!(echoed.kept);
    }

    #[test]
    fn incoherent_feature_is_suppressed() {
        let features = vec![
            feat("episodic", "a", vec![1.0, 0.0], 0.5),
            feat("cognition", "b", vec![0.99, 0.01], 0.5),
            feat("semantic", "c", vec![0.0, 1.0], 0.1),
        ];
        let bound = bind(&features, 0.6);
        let c = bound.iter().find(|b| b.content == "c").unwrap();
        assert!(!c.kept, "orthogonal low-relevance feature should be suppressed");
    }

    #[test]
    fn never_returns_empty_when_features_exist() {
        // All mutually orthogonal, high threshold — still keep the best one.
        let features = vec![
            feat("episodic", "a", vec![1.0, 0.0, 0.0], 0.2),
            feat("cognition", "b", vec![0.0, 1.0, 0.0], 0.2),
        ];
        let bound = bind(&features, 0.99);
        assert!(bound.iter().any(|b| b.kept));
    }

    #[test]
    fn single_modality_falls_back_to_relevance() {
        let features = vec![
            feat("episodic", "a", vec![1.0, 0.0], 0.9),
            feat("episodic", "b", vec![0.0, 1.0], 0.3),
        ];
        let bound = bind(&features, 0.5);
        let a = bound.iter().find(|b| b.content == "a").unwrap();
        assert!((a.coherence - 0.9).abs() < 1e-6);
    }
}
