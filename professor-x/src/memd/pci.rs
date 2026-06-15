//! Complexity instruments for consciousness measurement — the DIFFERENTIATION
//! axis that phi (integration) is blind to.
//!
//! Consciousness, in every serious theory, requires activity that is BOTH
//! integrated AND differentiated (complex). A system that fires the same pattern
//! every step is maximally integrated but carries no information (a seizure);
//! one that fires independently is differentiated but not integrated. phi (total
//! correlation) sees only integration. This module adds the complexity axis:
//!
//! - `normalized_lzc` — Lempel-Ziv complexity of the module spatiotemporal
//!   activity, normalized so random→~1, stereotyped→~0. Schartner et al. 2015
//!   showed spontaneous-activity LZc tracks conscious level (down under
//!   anaesthesia, up under psychedelics). This is the resting-state measure.
//!
//! - `pci` — Perturbational Complexity Index: LZc of the RESPONSE to a
//!   perturbation, normalized. Casali et al. 2013 — the clinical gold standard
//!   that separates conscious from unconscious states even in unresponsive
//!   patients. Captured here as the complexity of how a perturbation propagates
//!   across the modules.
//!
//! Both reduce to a Lempel-Ziv production-complexity (LZ76) core.

/// Lempel-Ziv (1976) production complexity c(n): the number of distinct
/// substrings encountered when parsing the sequence left to right. This is the
/// canonical LZc used in the consciousness literature.
pub fn lz76(seq: &[bool]) -> usize {
    let n = seq.len();
    if n == 0 {
        return 0;
    }
    let mut complexity = 1usize;
    let mut prefix_len = 1usize; // length of the already-parsed prefix
    let mut substr_len = 1usize; // current candidate substring length
    let mut i = 0usize; // start of the current candidate within the prefix scan
    while prefix_len + substr_len <= n {
        // Does seq[prefix_len .. prefix_len+substr_len] appear starting at i
        // within the prefix? Compare element-wise.
        if seq[i + substr_len - 1] == seq[prefix_len + substr_len - 1] {
            substr_len += 1;
            if prefix_len + substr_len > n {
                // ran off the end while still matching → final incomplete word
                complexity += 1;
                break;
            }
        } else {
            i += 1;
            if i == prefix_len {
                // no match found in the prefix → new component
                complexity += 1;
                prefix_len += substr_len;
                i = 0;
                substr_len = 1;
            } else {
                substr_len = 1;
            }
        }
    }
    complexity
}

/// Normalize a raw LZ76 count to [0,1]-ish by the asymptotic value for a random
/// binary sequence of the same length: c_rand ≈ n / log2(n). Random→~1,
/// perfectly regular→~0. (Standard normalization, e.g. Schartner et al.)
pub fn normalize_lz(c: usize, n: usize) -> f32 {
    if n < 2 {
        return 0.0;
    }
    let norm = (n as f32) / (n as f32).log2();
    (c as f32 / norm).min(2.0)
}

/// Normalized LZ complexity of a binary spatiotemporal matrix (channels × time).
/// Channels are concatenated into one sequence (Schartner's construction), then
/// LZ76 + normalized. Captures DIFFERENTIATION: how non-stereotyped the joint
/// activity is across modules and time.
pub fn normalized_lzc(channels: &[Vec<bool>]) -> f32 {
    let flat: Vec<bool> = channels.iter().flat_map(|c| c.iter().copied()).collect();
    let n = flat.len();
    if n < 2 {
        return 0.0;
    }
    normalize_lz(lz76(&flat), n)
}

/// Perturbational Complexity Index of a response matrix (channels × time) that
/// followed a perturbation. Same complexity core as `normalized_lzc`; the
/// distinction is experimental (this is the response to a probe, not resting
/// activity). Returned in the same normalized units so it can be compared
/// against a control (decoupled / System-1) condition — the PCI contrast.
pub fn pci(response_channels: &[Vec<bool>]) -> f32 {
    normalized_lzc(response_channels)
}

/// Build a (channels × time) bool matrix from a time series of packed 7-bit
/// module-activation indices (as stored in phi_activations.activation_index).
/// MODULE_COUNT channels, one column per time step.
pub fn matrix_from_activation_indices(indices: &[usize], module_count: usize) -> Vec<Vec<bool>> {
    (0..module_count)
        .map(|b| indices.iter().map(|idx| (idx >> b) & 1 == 1).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz_constant_sequence_is_minimal() {
        // All identical → 2 components (the first bit, then the rest as one run).
        let c = lz76(&[false; 64]);
        assert!(
            c <= 3,
            "constant seq should be near-minimal complexity, got {c}"
        );
    }

    #[test]
    fn lz_alternating_is_low() {
        // 0101... is highly regular → low complexity relative to length.
        let seq: Vec<bool> = (0..64).map(|i| i % 2 == 0).collect();
        let norm = normalize_lz(lz76(&seq), seq.len());
        assert!(
            norm < 0.6,
            "alternating should be low-complexity, got {norm}"
        );
    }

    #[test]
    fn lz_structured_below_random() {
        // A pseudo-random-ish but deterministic varied sequence should exceed a
        // perfectly regular one.
        let regular: Vec<bool> = (0..128).map(|i| i % 4 == 0).collect();
        let varied: Vec<bool> = (0..128)
            .map(|i| ((i * 2654435761usize) >> 13) & 1 == 1)
            .collect();
        let cr = normalize_lz(lz76(&regular), regular.len());
        let cv = normalize_lz(lz76(&varied), varied.len());
        assert!(cv > cr, "varied ({cv}) should exceed regular ({cr})");
    }

    #[test]
    fn matrix_unpacks_bits() {
        // index 0b0000101 = episodic(bit0)=1, cognition(bit2)=1, rest 0
        let m = matrix_from_activation_indices(&[0b0000101, 0b0000000], 7);
        assert_eq!(m.len(), 7);
        assert!(m[0][0] && !m[0][1]); // episodic on at t0, off at t1
        assert!(m[2][0]); // cognition on at t0
        assert!(!m[1][0]); // semantic off
    }
}
